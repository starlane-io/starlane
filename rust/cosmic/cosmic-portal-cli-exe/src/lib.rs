#![allow(warnings)]

mod scratch;

use core::option::Option;
use core::option::Option::None;
use core::result::Result::{Err, Ok};
use cosmic_universe::command::Command;
use cosmic_universe::id::id::{ToPoint, ToPort};
use cosmic_universe::msg::MsgMethod;
use cosmic_universe::parse::error::result;
use cosmic_universe::parse::model::MethodScopeSelector;
use cosmic_universe::parse::{command, command_line, Env};
use cosmic_universe::util::{ToResolved, ValuePattern};
use cosmic_universe::wave::{
    AsyncRequestHandlerRelay, AsyncRouter, DirectedHandler, DirectedHandler, InternalPipeline,
    PointRequestHandler, RequestHandlerRelay, SyncTransmitRelay, SyncTransmitter, Transmitter,
};
use cosmic_nom::new_span;
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::cli::{RawCommand, Transfer};
use mesh_portal::version::latest::config::bind::RouteSelector;
use mesh_portal::version::latest::entity::request::create::{
    CreateOp, Fulfillment, KindTemplate, Set,
};
use mesh_portal::version::latest::entity::request::get::Get;
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::entity::request::{Method, Rc, ReqCore};
use mesh_portal::version::latest::entity::response::RespCore;
use mesh_portal::version::latest::id::Port;
use mesh_portal::version::latest::id::{Point, TargetLayer, Topic, Uuid};
use mesh_portal::version::latest::messaging::{
    ReqCtx, ReqProto, ReqShell, RespShell, RootRequestCtx,
};
use mesh_portal::version::latest::particle::Stub;
use mesh_portal::version::latest::payload::{PayloadType, Substance};
use mesh_portal::version::latest::util::uuid;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

#[macro_use]
extern crate cosmic_macros;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;

#[derive(AsyncRequestHandler)]
pub struct CliRelay {
    pub port: Port,
    pub messenger: AsyncTransmitterWithAgent,
    pub handlers: RwLock<PointRequestHandler<AsyncRequestHandlerRelay>>,
}

#[routes_async(self.handlers.read().await)]
impl CliRelay {
    pub fn new(port: Port, messenger: AsyncTransmitterWithAgent) -> Self {
        let handlers = RwLock::new(PointRequestHandler::new());

        let rtn = Self {
            port,
            messenger,
            handlers,
        };

        rtn
    }

    fn filter(&self, request: &ReqShell) -> bool {
        request.from.layer == TargetLayer::Core
    }

    #[route("Msg<NewSession>")]
    pub async fn new_session(&self, ctx: ReqCtx<'_, ReqShell>) -> Result<Port, MsgErr> {
        if !self.filter(ctx.wave()) {
            return Err(MsgErr::forbidden());
        }

        let mut session_port = self.port.clone().with_topic(Topic::uuid());
        let mut source = ctx.wave().from.clone();

        let messenger = self.messenger.clone().with_from(session_port.clone());

        let session = CliSession {
            port: session_port.clone(),
            relay: self.port.clone(),
            env: Env::new(self.port.clone().to_point()),
            source,
            tx: messenger,
        };

        let selector = RouteSelector::any().with_topic(session_port.topic.clone());
        {
            let mut write = self.handlers.write().await;
            write.add(selector, AsyncRequestHandlerRelay::new(Arc::new(session)));
        }

        Ok(session_port)
    }

    #[route("Msg<EndSession>")]
    pub async fn end_session(&self, ctx: ReqCtx<'_, ReqShell>) -> Result<RespCore, MsgErr> {
        if !self.filter(ctx.wave()) {
            return Err(MsgErr::new(403, "forbidden"));
        }

        let mut write = self.handlers.write().await;
        write.remove_topic(Some(ValuePattern::Pattern(ctx.to.topic.clone())));
        Ok(RespCore::ok(Substance::Empty))
    }
}

#[derive(AsyncRequestHandler)]
pub struct CliSession {
    pub relay: Port,
    pub port: Port,
    pub env: Env,
    pub tx: AsyncTransmitterWithAgent,
    // will only handle requests from THIS port
    pub source: Port,
}

#[routes_async]
impl CliSession {
    pub fn new(
        port: Port,
        relay: Port,
        messenger: AsyncTransmitterWithAgent,
        source: Port,
    ) -> CliSession {
        let messenger = messenger.with_from(port.clone());
        let env = Env::new(port.clone().to_point());
        Self {
            port,
            relay,
            env,
            tx: messenger,
            source,
        }
    }

    pub fn filter(&self, request: &ReqShell) -> bool {
        request.from == self.source
    }

    #[route("Msg<ExecCommand>")]
    pub async fn exec(&self, ctx: ReqCtx<'_, RawCommand>) -> Result<RespCore, MsgErr> {
        if !self.filter(ctx.wave()) {
            return Err(MsgErr::forbidden());
        }

        let exec_topic = Topic::uuid();
        let exec_port = self.port.clone().with_topic(exec_topic.clone());
        let tx = self.tx.clone().with_topic(exec_topic);
        let mut exec = CommandExecutor::new(exec_port, self.source.clone(), tx, self.env.clone());

        let result = exec.execute(ctx).await;

        result
    }
}

#[derive(AsyncRequestHandler)]
pub struct CommandExecutor {
    tx: AsyncTransmitterWithAgent,
    port: Port,
    source: Port,
    env: Env,
}

#[routes_async]
impl CommandExecutor {
    pub fn new(port: Port, source: Port, tx: AsyncTransmitterWithAgent, env: Env) -> Self {
        Self {
            tx,
            port,
            source,
            env,
        }
    }

    pub async fn execute(&self, raw: ReqCtx<'_, RawCommand>) -> Result<RespCore, MsgErr> {
        let command = result(command_line(new_span(raw.line.as_str())))?;
        let mut env = self.env.clone();
        for transfer in &raw.transfers {
            env.set_file(transfer.id.clone(), transfer.content.clone())
        }
        let command: Command = command.to_resolved(&self.env)?;
        let request: ReqCore = command.into();
        let request = ReqProto::from_core(request);

        RespShell::core(self.tx.send(request).await)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
