use crate::hyperspace::err::HyperErr;
use crate::hyperspace::platform::Platform;
use crate::hyperspace::reg::Registration;
use crate::hyperspace::star::{HyperStarSkel, SmartLocator};
use once_cell::sync::Lazy;
use starlane_parse::new_span;
use starlane::space::artifact::ArtRef;
use starlane::space::command::direct::create::{Create, PointSegTemplate};
use starlane::space::command::Command;
use starlane::space::command::RawCommand;
use starlane::space::config::bind::BindConfig;
use starlane::space::loc::{ToPoint, ToSurface};
use starlane::space::log::PointLogger;
use starlane::space::parse::error::result;
use starlane::space::parse::{bind_config, command_line};
use starlane::space::particle::{Details, Status};
use starlane::space::point::Point;
use starlane::space::substance::Substance;
use starlane::space::util::{log, ToResolved};
use starlane::space::wave::core::cmd::CmdMethod;
use starlane::space::wave::core::ReflectedCore;
use starlane::space::wave::exchange::asynch::{DirectedHandler, InCtx};
use starlane::space::wave::{Agent, DirectedProto};
use std::str::FromStr;
use std::sync::Arc;
/*
#[derive(DirectedHandler,Clone)]
pub struct Global<P> where P: Platform {
    pub logger: PointLogger,
    pub registry: Registry<P>,
}

 */

static GLOBAL_BIND_CONFIG: Lazy<ArtRef<BindConfig>> = Lazy::new(|| {
    ArtRef::new(
        Arc::new(global_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/global.bind").unwrap(),
    )
});

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
    P: Platform,
{
    skel: HyperStarSkel<P>,
}

impl<P> GlobalCommandExecutionHandler<P>
where
    P: Platform,
{
    pub fn new(skel: HyperStarSkel<P>) -> Self {
        Self { skel }
    }
}

#[handler]
impl<P> GlobalCommandExecutionHandler<P>
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
        let global = GlobalExecutionChamber::new(self.skel.clone());
        let agent = ctx.wave().agent().clone();
        match ctx.input {
            Command::Create(create) => {
                let details = self
                    .skel
                    .logger
                    .result(global.create(create, &agent).await)?;
                Ok(ReflectedCore::ok_body(details.into()))
            }
            Command::Select(select) => {
                let mut select = select.clone();
                let substance: Substance = self.skel.registry.select(&mut select).await?.into();
                Ok(ReflectedCore::ok_body(substance))
            }
            Command::Delete(delete) => {
                let substance: Substance = self.skel.registry.delete(delete).await?.into();
                Ok(ReflectedCore::ok_body(substance))
            }
            Command::Set(set) => {
                self.skel
                    .registry
                    .set_properties(&set.point, &set.properties)
                    .await?;
                Ok(ReflectedCore::ok())
            }
            Command::Read(read) => {
                println!("\tread cmd : {}", read.point.to_string());
                // proxy the read command
                let mut proto = DirectedProto::ping();
                proto.method(CmdMethod::Read);
                proto.agent(ctx.wave().agent().clone());
                proto.to(read.point.to_surface());
                let pong = ctx.transmitter.ping(proto).await?;
                Ok(pong.variant.core)
            }
            _ => Err(P::Err::new("not implemented")),
        }
    }
}

pub struct GlobalExecutionChamber<P>
where
    P: Platform,
{
    pub skel: HyperStarSkel<P>,
    pub logger: PointLogger,
}

impl<P> GlobalExecutionChamber<P>
where
    P: Platform,
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
            .cosmos
            .select_kind(&create.template.kind)
            .map_err(|err| {
                P::Err::new(format!(
                    "Kind {} is not available on this Platform",
                    create.template.kind.to_string()
                ))
            })?;
        let point = match &create.template.point.child_segment_template {
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
                    .cosmos
                    .properties_config(&child_kind)
                    .fill_create_defaults(&create.properties)?;
                self.skel
                    .machine
                    .cosmos
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
                result?;
                point
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

                self.skel.registry.register(&registration).await?;
                point
            }
            PointSegTemplate::Root => Point::root(),
        };

        if create.state.has_substance() || child_kind.is_auto_provision() {
            println!("\tprovisioning: {}", point.to_string());
            let provisioner = SmartLocator::new(self.skel.clone());
            //tokio::spawn(async move {
            provisioner.provision(&point, create.state.clone()).await?;
            //});
        }

        let record = self.skel.registry.record(&point).await?;

        Ok(record.details)
    }
}
