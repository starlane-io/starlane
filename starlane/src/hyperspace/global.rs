use crate::hyperspace::reg::Registration;
use crate::hyperspace::registry::err::RegErr;
use crate::hyperspace::star::{HyperStarSkel, SmartLocator, StarErr};
use once_cell::sync::Lazy;
use starlane_macros::{handler, route, DirectedHandler};
use starlane_primitive_macros::push_mark;
use crate::space::artifact::ArtRef;
use crate::space::command::direct::create::{Create, PointSegTemplate};
use crate::space::command::Command;
use crate::space::command::RawCommand;
use crate::space::config::bind::BindConfig;
use crate::space::err::{CoreReflector, SpaceErr};
use crate::space::loc::{ToPoint, ToSurface};
use crate::space::log::Logger;
use crate::space::parse::util::new_span;
use crate::space::parse::util::result;
use crate::space::parse::{bind_config, command_line};
use crate::space::particle::{Details, Status};
use crate::space::point::Point;
use crate::space::substance::Substance;
use crate::space::util::{log, ToResolved};
use crate::space::wave::core::cmd::CmdMethod;
use crate::space::wave::core::http2::StatusCode;
use crate::space::wave::core::ReflectedCore;
use crate::space::wave::exchange::asynch::{DirectedHandler, InCtx};
use crate::space::wave::{Agent, DirectedProto};
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use thiserror_context::impl_context;
/*
#[derive(DirectedHandler,Clone)]
pub struct Global where P: Platform {
    pub logger: PointLogger,
    pub registry: Registry,
}

 */

#[derive(Clone, DirectedHandler)]
pub struct GlobalCommandExecutionHandler {
    skel: HyperStarSkel,
}

impl GlobalCommandExecutionHandler {
    pub fn new(skel: HyperStarSkel) -> Self {
        Self { skel }
    }
}

#[handler]
impl GlobalCommandExecutionHandler {
    #[route("Cmd<RawCommand>")]
    pub async fn raw(&self, ctx: InCtx<'_, RawCommand>) -> Result<ReflectedCore, StarErr> {
        let line = ctx.input.line.clone();
        let span = new_span(line.as_str());
        let command = log(result(command_line(span)))?;
        let command = command.collapse()?;
        let ctx = ctx.push_input_ref(&command);
        self.command(ctx).await
    }

    #[route("Cmd<Command>")]
    pub async fn command(&self, ctx: InCtx<'_, Command>) -> Result<ReflectedCore, StarErr> {
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
                // proxy the read command
                let mut proto = DirectedProto::ping();
                proto.method(CmdMethod::Read);
                proto.agent(ctx.wave().agent().clone());
                proto.to(read.point.to_surface());
                let pong = ctx.transmitter.ping(proto).await?;
                Ok(pong.variant.core)
            }
            c => Err(SpaceErr::unimplemented(format!("command not recognized")))?,
        }
    }
}

pub struct GlobalExecutionChamber {
    pub skel: HyperStarSkel,
    pub logger: Logger,
}

impl GlobalExecutionChamber {
    pub fn new(skel: HyperStarSkel) -> Self {
        let logger = push_mark!(skel.logger);
        Self { skel, logger }
    }

    #[track_caller]
    pub async fn create(&self, create: &Create, agent: &Agent) -> Result<Details, StarErr> {
        let child_kind = self
            .skel
            .machine_api
            .select_kind(&create.template.kind)
            .await?;
        let point = match &create.template.point.child_segment_template {
            PointSegTemplate::Exact(child_segment) => {
                let point = create.template.point.parent.push(child_segment.clone())?;

                let properties = self
                    .skel
                    .machine_api
                    .properties_config(&child_kind)
                    .await?
                    .fill_create_defaults(&create.properties)?;
                self.skel
                    .machine_api
                    .properties_config(&child_kind)
                    .await?
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
                    Err(SpaceErr::ExpectingWildcardInPointTemplate(
                        pattern.to_string(),
                    ))?;
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
            let provisioner = SmartLocator::new(self.skel.clone());
            //tokio::spawn(async move {
            provisioner.provision(&point, create.state.clone()).await?;
            //});
        }

        let record = self.skel.registry.record(&point).await?;

        Ok(record.details)
    }
}
