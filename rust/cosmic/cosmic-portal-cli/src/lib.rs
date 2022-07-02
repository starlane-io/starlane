#![allow(warnings)]


#[macro_use]
extern crate cosmic_macros;

#[macro_use]
extern crate async_trait;

use std::sync::Arc;
use cosmic_api::cli::{CommandTemplate, RawCommand, Transfer};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Port, Topic};
use cosmic_api::msg::MsgMethod;
use cosmic_api::wave::{Agent, Exchanger, Handling, PingProto, Pong, ProtoTransmitter, Router, Scope, SetStrategy, ToRecipients, Wave};

pub struct Cli {
    cli_session_factory: Port,
    tx: ProtoTransmitter,
}

impl Cli {
    pub fn new(
        router: Arc<dyn Router>,
        cli_session_factory: Port,
        mut from: Port,
        exchanger: Exchanger
    ) -> Self {
        let mut tx = ProtoTransmitter::new(router, exchanger);
        from = from.with_topic(Topic::Cli);
        tx.from = SetStrategy::Override(from);

        Self {
            cli_session_factory,
            tx,
        }
    }

    pub fn set_agent(&mut self, agent: Agent) {
        self.tx.agent = SetStrategy::Override(agent);
    }

    pub fn set_handling(&mut self, handling: Handling) {
        self.tx.handling = SetStrategy::Override(handling);
    }

    pub fn set_scope(&mut self, scope: Scope) {
        self.tx.scope = SetStrategy::Override(scope);
    }

    pub async fn session(&self) -> Result<CliSession<'_>, MsgErr> {
        let mut ping = PingProto::new();
        ping.to(self.cli_session_factory.clone());
        ping.method(MsgMethod::new("NewCliSession").unwrap());

        let pong: Wave<Pong> = self.tx.direct(ping).await?;

        if pong.core.is_ok() {
            let session: Port = pong.core.body.clone().try_into()?;
            let mut tx = self.tx.clone();
            tx.to = SetStrategy::Override(session.to_recipients());
            tx.from_topic(Topic::Cli)?;
            Ok(CliSession::new(self, tx))
        } else {
            Err("could not create cli".into())
        }
    }
}

#[derive(Clone)]
pub struct CliSession<'a> {
    pub cli: &'a Cli,
    pub tx: ProtoTransmitter,
}

impl<'a> CliSession<'a> {
    pub fn new(cli: &'a Cli, transmitter: ProtoTransmitter) -> Self {
        Self {
            cli,
            tx: transmitter,
        }
    }

    pub async fn exec<R: ToString>(&self, raw: R) -> Result<Wave<Pong>, MsgErr> {
        self.exec_with_transfers(raw, vec![]).await
    }

    pub async fn exec_with_transfers<R>(
        &self,
        raw: R,
        transfers: Vec<Transfer>,
    ) -> Result<Wave<Pong>, MsgErr>
    where
        R: ToString,
    {
        let raw = RawCommand {
            line: raw.to_string(),
            transfers,
        };
        let mut req: PingProto = PingProto::new();
        req.core(raw.into())?;
        self.tx.direct(req.clone()).await
    }

    pub fn template<R: ToString>(&self, raw: R) -> Result<CommandTemplate, MsgErr> {
        unimplemented!()
    }
}

impl<'a> Drop for CliSession<'a> {
    fn drop(&mut self) {
        match self.tx.to.clone().unwrap() {
            Ok(to) => {
                let ping = PingProto::msg(to, MsgMethod::new("DropSession").unwrap());
                let tx = self.tx.clone();
                match ping.build() {
                    Ok(ping) => {
                        tx.route_sync(ping.to_ultra() );
                    }
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }
    }
}

#[cfg(test)]
pub mod test {
    use mesh_portal::error::MsgErr;
    use mesh_portal::version::latest::entity::request::ReqCore;
    use mesh_portal::version::latest::entity::response::RespCore;
    use mesh_portal::version::latest::messaging::{ReqShell, RootRequestCtx};
    use mesh_portal::version::latest::payload::Substance;
    use cosmic_api::wave::{
        DirectedHandler, InCtx, DirectedHandler, RequestHandlerRelay,
    };
    use std::marker::PhantomData;
    use std::sync::{Arc, RwLock};

    #[test]
    pub fn test() {
        //let mut obj: Obj = Obj::new();
        //        router.pipelines.push(IntPipeline)
    }
}
