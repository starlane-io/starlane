use crate::{DriverFactory, PlatErr, Platform, Registry};
use cosmic_api::cli::RawCommand;
use cosmic_api::command::command::common::StateSrc;
use cosmic_api::command::request::create::{Create, PointSegTemplate, Strategy};
use cosmic_api::command::Command;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, Port, ToPort, GLOBAL_EXEC};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::parse::command_line;
use cosmic_api::parse::error::result;
use cosmic_api::particle::particle::{Details, Status};
use cosmic_api::util::{log, ToResolved};
use cosmic_api::wave::{
    Agent, DirectedHandlerShell, DirectedProto, Exchanger, Handling, InCtx, Pong, ProtoTransmitter,
    ProtoTransmitterBuilder, ReflectedCore, Router, Scope, SetStrategy, SysMethod, Wave,
};
use cosmic_api::{Registration, HYPERUSER};
use cosmic_nom::new_span;
use std::sync::Arc;

/*
#[derive(DirectedHandler,Clone)]
pub struct Global<P> where P: Platform {
    pub logger: PointLogger,
    pub registry: Registry<P>,
}

 */

use crate::driver::{Driver, DriverInitCtx, DriverSkel, DriverStatus, Item, ItemHandler};
use crate::star::StarSkel;
use cosmic_api::config::config::bind::RouteSelector;
use cosmic_api::parse::route_attribute;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, AssignmentKind, Sys};
use cosmic_api::wave::CoreBounce;
use cosmic_api::wave::DirectedHandler;
use cosmic_api::wave::DirectedHandlerSelector;
use cosmic_api::wave::RecipientSelector;
use cosmic_api::wave::RootInCtx;

pub struct GlobalDriverFactory<P>
where
    P: Platform,
{
    pub skel: StarSkel<P>,
}

#[async_trait]
impl<P> DriverFactory<P> for GlobalDriverFactory<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Global
    }

    async fn init(
        &self,
        skel: DriverSkel<P>,
        ctx: &DriverInitCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(GlobalDriver::new(self.skel.clone())))
    }
}

impl<P> GlobalDriverFactory<P>
where
    P: Platform,
{
    pub fn new(skel: StarSkel<P>) -> Self {
        Self { skel }
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

    async fn init(&self, skel: DriverSkel<P>, ctx: DriverInitCtx) -> Result<(), P::Err> {
        let point = self.skel.machine.global.clone().point;
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Global,
            registry: Default::default(),
            properties: Default::default(),
            owner: HYPERUSER.clone(),
            strategy: Strategy::Override,
        };

        self.skel.registry.register(&registration).await?;
        self.skel.registry.assign(&point, &self.skel.point).await?;
        self.skel.api.create_states(point.clone()).await?;
        self.skel
            .registry
            .set_status(&point, &Status::Ready)
            .await?;
        self.skel
            .logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();
        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<Box<dyn ItemHandler<P>>, P::Err> {
        if *point == self.skel.machine.global {
            Ok(Box::new(GlobalCore::restore(self.skel.clone(), (), ())))
        } else {
            Err(MsgErr::not_found().into())
        }
    }

    async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        Err(MsgErr::forbidden())
    }
}

#[routes]
impl<P> GlobalDriver<P>
where
    P: Platform,
{
    pub fn new(skel: StarSkel<P>) -> Self {
        let core_point = skel.point.push("global").unwrap();
        Self { skel, core_point }
    }
}

#[derive(DirectedHandler)]
pub struct GlobalCore<P>
where
    P: Platform,
{
    skel: StarSkel<P>,
}

impl<P> ItemHandler<P> for GlobalCore<P> where P: Platform {}

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
        match ctx.input {
            Command::Create(create) => Ok(ctx.ok_body(self.create(&create).await?.stub.into())),
            _ => Err(P::Err::new("not implemented")),
        }
    }
}

pub struct Global<P>
where
    P: Platform,
{
    pub skel: StarSkel<P>,
    pub logger: Logger,
}

impl<P> Global<P>
where
    P: Platform,
{
    pub async fn create(&self, create: &Create) -> Result<Details, P::Err> {
        let child_kind = self
            .skel
            .machine
            .platform
            .default_implementation(&create.template.kind)?;
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
                    registry: create.registry.clone(),
                    properties,
                    owner: Point::root(),
                    strategy: create.strategy.clone(),
                };
                let mut result = self.skel.registry.register(&registration).await;
                result?
            }
            PointSegTemplate::Pattern(pattern) => {
                if !pattern.contains("%") {
                    return Err(P::Err::status_msg(500u16, "AddressSegmentTemplate::Pattern must have at least one '%' char for substitution"));
                }
                loop {
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
                        registry: create.registry.clone(),
                        properties: create.properties.clone(),
                        owner: Point::root(),
                        strategy: create.strategy.clone(),
                    };

                    self.skel.registry.register(&registration).await?;
                }
            }
        };

        let parent = self
            .skel
            .registry
            .locate(&create.template.point.parent)
            .await?;
        let assign = Assign::new(AssignmentKind::Create, details.clone(), StateSrc::None);

        let mut wave = DirectedProto::ping();
        wave.method(SysMethod::Assign);
        wave.body(Sys::Assign(assign).into());
        wave.from(self.skel.point.clone().to_port().with_layer(Layer::Core));
        wave.to(parent.location);

        let pong: Wave<Pong> = self.skel.gravity_transmitter.direct(wave).await?;

        if pong.core.status.as_u16() == 200 {
            if let Substance::Point(location) = &pong.core.body {
                self.skel
                    .registry
                    .assign(&details.stub.point, &location)
                    .await?;
            } else {
                return self
                    .logger
                    .result(Err("Assign result expected Substance Point".into()));
            }
        } else {
            self.logger
                .result(
                    self.skel
                        .registry
                        .set_status(&details.stub.point, &Status::Panic)
                        .await,
                )
                .unwrap_or_default();
        }

        Ok(details)
    }
}
