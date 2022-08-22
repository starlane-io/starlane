use std::marker::PhantomData;
use std::str::FromStr;
use crate::{DriverFactory, PlatErr, Platform, Registry};
use cosmic_api::cli::RawCommand;
use cosmic_api::command::command::common::StateSrc;
use cosmic_api::command::request::create::{Create, PointSegTemplate, Strategy};
use cosmic_api::command::Command;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, Port, ToPort, GLOBAL_EXEC, ToPoint};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::parse::{bind_config, command_line};
use cosmic_api::parse::error::result;
use cosmic_api::particle::particle::{Details, Status};
use cosmic_api::util::{log, ToResolved};
use cosmic_api::wave::{
    Agent, DirectedHandlerShell, DirectedProto, Exchanger, Handling, InCtx, Pong, ProtoTransmitter,
    ProtoTransmitterBuilder, ReflectedCore, Router, Scope, SetStrategy, SysMethod, Wave,
};
use cosmic_api::{Registration, HYPERUSER, ArtRef};
use cosmic_nom::new_span;
use std::sync::Arc;

/*
#[derive(DirectedHandler,Clone)]
pub struct Global<P> where P: Platform {
    pub logger: PointLogger,
    pub registry: Registry<P>,
}

 */

use crate::driver::{Driver, DriverCtx, DriverSkel, DriverStatus, HyperDriverFactory, Item, ItemDirectedHandler, ItemHandler};
use crate::star::StarSkel;
use cosmic_api::config::config::bind::{BindConfig, RouteSelector};
use cosmic_api::parse::route_attribute;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, AssignmentKind, Sys};
use cosmic_api::wave::CoreBounce;
use cosmic_api::wave::DirectedHandler;
use cosmic_api::wave::DirectedHandlerSelector;
use cosmic_api::wave::RecipientSelector;
use cosmic_api::wave::RootInCtx;

lazy_static! {
    static ref GLOBAL_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(global_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/global.bind").unwrap()
    );
}


fn global_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route<Cmd<RawCommand>> -> (());
       Route<Cmd<Command>> -> (()) => &;
    }
    "#,
    ))
        .unwrap()

}

pub struct GlobalDriverFactory<P>
where
    P: Platform,
{
    phantom: PhantomData<P>
}

#[async_trait]
impl<P> HyperDriverFactory<P> for GlobalDriverFactory<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Global
    }

    async fn create(
        &self,
        star: StarSkel<P>,
        driver: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(GlobalDriver::new(star)))
    }
}

impl<P> GlobalDriverFactory<P>
where
    P: Platform,
{
    pub fn new() -> Self {
        Self {
            phantom: Default::default()
        }
    }
}

#[derive(DirectedHandler)]
pub struct GlobalDriver<P>
where
    P: Platform,
{
    pub skel: StarSkel<P>,
}

#[async_trait]
impl<P> Driver<P> for GlobalDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Global
    }

    async fn init(&mut self, skel: DriverSkel<P>, ctx: DriverCtx) -> Result<(), P::Err> {
        self.skel
            .logger
            .result(skel.status_tx.send(DriverStatus::Init).await)
            .unwrap_or_default();

        let point = self.skel.machine.global.clone().point;
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Global,
            registry: Default::default(),
            properties: Default::default(),
            owner: HYPERUSER.clone(),
            strategy: Strategy::Override,
            status: Status::Ready
        };

        self.skel.api.create_states(point.clone()).await?;
        self.skel.registry.register(&registration).await?;
        self.skel.registry.assign(&point).send(self.skel.point.clone());
        self.skel
            .logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();

        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemHandler<P>, P::Err> {
        if *point == self.skel.machine.global.point {
            Ok(ItemHandler::Handler(Box::new(GlobalCore::restore(self.skel.clone(), (), ()))))
        } else {
            Err(MsgErr::not_found().into())
        }
    }

    async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        Err(MsgErr::forbidden().into())
    }
}

#[routes]
impl<P> GlobalDriver<P>
where
    P: Platform,
{
    pub fn new(skel: StarSkel<P>) -> Self {
        Self { skel }
    }
}

#[derive(DirectedHandler)]
pub struct GlobalCore<P>
where
    P: Platform,
{
    skel: StarSkel<P>,
}

#[async_trait]
impl <P> ItemDirectedHandler<P> for GlobalCore<P> where P: Platform {
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
          <GlobalCore<P> as Item<P>>::bind(self).await
    }
}


#[async_trait]
impl<P> Item<P> for GlobalCore<P>
where
    P: Platform,
{
    type Skel = StarSkel<P>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        GlobalCore { skel }
    }

    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(GLOBAL_BIND_CONFIG.clone())
    }
}

#[routes]
impl<P> GlobalCore<P>
where
    P: Platform,
{
    #[route("Cmd<RawCommand>")]
    pub async fn raw(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, P::Err> {
        let line = ctx.input.line.clone();
        let span = new_span(line.as_str());
        let command = log(result(command_line(span)))?;
        let command = command.collapse()?;
        let ctx = ctx.push_input_ref(&command);
        self.command(ctx).await
    }

    #[route("Cmd<Command>")]
    pub async fn command(&self, ctx: InCtx<'_, Command>) -> Result<ReflectedCore, P::Err> {
        let global = Global::new( self.skel.clone(), self.skel.logger.clone() );
        let agent = ctx.wave().agent().clone();
        match ctx.input {
            Command::Create(create) => {
                Ok(ctx.ok_body(self.skel.logger.result(global.create(create,&agent).await)?.into()))
            },
            _ => Err(P::Err::new("not implemented")),
        }
    }
}

pub struct Global<P>
where
    P: Platform,
{
    pub skel: StarSkel<P>,
    pub logger: PointLogger,
}

impl<P> Global<P>
where
    P: Platform,
{
    pub fn new( skel: StarSkel<P>, logger: PointLogger ) -> Self {
        Self {
            skel,
            logger
        }
    }

    pub async fn create(&self, create: &Create, agent: &Agent) -> Result<Details, P::Err> {
        let child_kind = self
            .skel
            .machine
            .platform
            .select_kind(&create.template.kind)?;
        let details = match &create.template.point.child_segment_template {
            PointSegTemplate::Exact(child_segment) => {
                let point = create.template.point.parent.push(child_segment.clone());
                match &point {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("RC CREATE error: {}", err.to_string());
                    }
                }
                let point = point?;

                let properties = self
                    .skel
                    .machine
                    .platform
                    .properties_config(&child_kind)
                    .fill_create_defaults(&create.properties)?;
                self.skel
                    .machine
                    .platform
                    .properties_config(&child_kind)
                    .check_create(&properties)?;

                let registration = Registration {
                    point: point.clone(),
                    kind: child_kind.clone(),
                    registry: Default::default(),
                    properties,
                    owner: agent.clone().to_point(),
                    strategy: create.strategy.clone(),
                    status: Status::Ready
                };
                let mut result = self.skel.registry.register(&registration).await;
                result?
            }
            PointSegTemplate::Pattern(pattern) => {
                if !pattern.contains("%") {
                    return Err(P::Err::status_msg(500u16, "AddressSegmentTemplate::Pattern must have at least one '%' char for substitution"));
                }
                    let index = self
                        .skel
                        .registry
                        .sequence(&create.template.point.parent)
                        .await?;
                    let child_segment = pattern.replace("%", index.to_string().as_str());
                    let point = create.template.point.parent.push(child_segment.clone())?;
                    let registration = Registration {
                        point: point.clone(),
                        kind: child_kind.clone(),
                        registry: Default::default(),
                        properties: create.properties.clone(),
                        owner: Point::root(),
                        strategy: create.strategy.clone(),
                        status: Status::Ready
                    };

                    self.skel.registry.register(&registration).await?
            }
        };

        Ok(details)
    }
}
