use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;

use cosmic_nom::new_span;
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::command::common::StateSrc;
use cosmic_universe::command::direct::create::{Create, PointSegTemplate, Strategy};
use cosmic_universe::command::Command;
use cosmic_universe::command::RawCommand;
use cosmic_universe::config::bind::{BindConfig, RouteSelector};
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::{Assign, AssignmentKind, HyperSubstance};
use cosmic_universe::kind::Kind;
use cosmic_universe::loc::{Layer, Point, Surface, ToPoint, ToSurface};
use cosmic_universe::log::{PointLogger, RootLogger};
use cosmic_universe::parse::error::result;
use cosmic_universe::parse::route_attribute;
use cosmic_universe::parse::{bind_config, command_line};
use cosmic_universe::particle::{Details, Status};
use cosmic_universe::substance::Substance;
use cosmic_universe::util::{log, ToResolved};
use cosmic_universe::wave::core::hyp::HypMethod;
use cosmic_universe::wave::core::CoreBounce;
use cosmic_universe::wave::core::ReflectedCore;
use cosmic_universe::wave::exchange::DirectedHandlerShell;
use cosmic_universe::wave::exchange::RootInCtx;
use cosmic_universe::wave::exchange::{DirectedHandler, Exchanger, InCtx};
use cosmic_universe::wave::exchange::{
    DirectedHandlerSelector, ProtoTransmitter, ProtoTransmitterBuilder, SetStrategy,
};
use cosmic_universe::wave::RecipientSelector;
use cosmic_universe::wave::{Agent, DirectedProto, Handling, Pong, Scope, Wave};
use cosmic_universe::HYPERUSER;
use cosmic_universe::wave::exchange::asynch::Router;

use crate::driver::{
    Driver, DriverCtx, DriverSkel, DriverStatus, HyperDriverFactory, Item, ItemHandler, ItemSphere,
};
use crate::star::{HyperStarSkel, SmartLocator};
use crate::Registration;
use crate::{Cosmos, DriverFactory, HyperErr, Registry};

/*
#[derive(DirectedHandler,Clone)]
pub struct Global<P> where P: Platform {
    pub logger: PointLogger,
    pub registry: Registry<P>,
}

 */

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

#[derive(Clone, DirectedHandler)]
pub struct GlobalCommandExecutionHandler<P>
where
    P: Cosmos,
{
    skel: HyperStarSkel<P>,
}

impl<P> GlobalCommandExecutionHandler<P>
where
    P: Cosmos,
{
    pub fn new(skel: HyperStarSkel<P>) -> Self {
        Self { skel }
    }
}

#[handler]
impl<P> GlobalCommandExecutionHandler<P>
where
    P: Cosmos,
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
        let global = GlobalExecutionChamber::new(self.skel.clone());
        let agent = ctx.wave().agent().clone();
        match ctx.input {
            Command::Create(create) => Ok(ctx.ok_body(
                self.skel
                    .logger
                    .result(global.create(create, &agent).await)?
                    .into(),
            )),
            _ => Err(P::Err::new("not implemented")),
        }
    }
}

pub struct GlobalExecutionChamber<P>
where
    P: Cosmos,
{
    pub skel: HyperStarSkel<P>,
    pub logger: PointLogger,
}

impl<P> GlobalExecutionChamber<P>
where
    P: Cosmos,
{
    pub fn new(skel: HyperStarSkel<P>) -> Self {
        let logger = skel.logger.push_point("global").unwrap();
        Self { skel, logger }
    }

    #[track_caller]
    pub async fn create(&self, create: &Create, agent: &Agent) -> Result<Details, P::Err> {
        let child_kind = self
            .skel
            .machine
            .hyperverse
            .select_kind(&create.template.kind)
            .map_err(|err| {
                P::Err::new(format!(
                    "Kind {} is not available on this Platform",
                    create.template.kind.to_string()
                ))
            })?;
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
                    .hyperverse
                    .properties_config(&child_kind)
                    .fill_create_defaults(&create.properties)?;
                self.skel
                    .machine
                    .hyperverse
                    .properties_config(&child_kind)
                    .check_create(&properties)?;

                let registration = Registration {
                    point: point.clone(),
                    kind: child_kind.clone(),
                    registry: Default::default(),
                    properties,
                    owner: agent.clone().to_point(),
                    strategy: create.strategy.clone(),
                    status: Status::Ready,
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
                    status: Status::Ready,
                };

                self.skel.registry.register(&registration).await?
            }
        };

        if create.state.has_substance() || details.stub.kind.is_auto_provision() {
            // spawning a task is a hack, but without it this process will freeze
            // need to come up with a better solution so that
            {
                let details = details.clone();
                let provisioner = SmartLocator::new(self.skel.clone());
                let state = create.state.clone();
                tokio::spawn(async move {
                    provisioner
                        .provision(&details.stub.point, state)
                        .await
                        .unwrap();
                });
            }
        }

        Ok(details)
    }
}
