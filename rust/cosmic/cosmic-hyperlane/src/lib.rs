#![allow(warnings)]

use cosmic_api::command::request::create::{PointFactory, PointFactoryU128, PointSegTemplate};
use cosmic_api::error::MsgErr;
use cosmic_api::frame::frame::PrimitiveFrame;
use cosmic_api::id::id::{Point, Port, ToPoint, ToPort, Version};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::substance::substance::{Errors, Substance, SubstanceKind, Token};
use cosmic_api::sys::{EntryReq, InterchangeKind, Sys};
use cosmic_api::util::uuid;
use cosmic_api::wave::{
    Agent, HyperWave, Method, Ping, Pong, Reflectable, Router, SysMethod, UltraWave, Wave,
};
use cosmic_api::VERSION;
use dashmap::DashMap;
use futures::future::select_all;
use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError, TrySendError};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, Mutex, oneshot};

#[macro_use]
extern crate async_trait;

pub struct Hyperway {
    pub agent: Option<Agent>,
    pub remote: Point,
    outbound: Hyperlane,
    inbound: Hyperlane,
}

impl Hyperway {
    pub fn new(
        remote: Point,
    ) -> Self {

        Self {
            agent: None,
            remote,
            outbound: Hyperlane::new(),
            inbound: Hyperlane::new(),
        }
    }

    pub fn ext(&self) -> HyperwayExt {
        HyperwayExt {
            tx: self.outbound.tx(),
            rx: self.inbound.rx(),
        }
    }

}

pub struct HyperwayExt {
    pub tx: mpsc::Sender<UltraWave>,
    pub rx: mpsc::Receiver<UltraWave>
}

impl HyperwayExt {}

#[derive(Clone)]
pub struct HyperwayStub {
    pub agent: Agent,
    pub remote: Point,
}
pub enum HyperwayInterchangeCall {
    Wave(UltraWave),
    Add(Hyperway),
    Remove(Point),
    Ext{ point: Point, tx: oneshot::Sender<HyperwayExt> }
}

pub enum HyperlaneCall {
    Drain,
    LaneExt(mpsc::Sender<UltraWave>),
    Wave(UltraWave),
}

pub struct Hyperlane {
    tx: mpsc::Sender<HyperlaneCall>,
}

impl Hyperlane {
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::channel(1024);
        {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut ext = None;
                let mut queue = vec![];
                while let Some(call) = rx.recv().await {
                    match call {
                        HyperlaneCall::LaneExt(new) => {
                            ext.replace(new);
                        }
                        HyperlaneCall::Wave(wave) => {
                            while queue.len() > 1024 {
                                // start dropping the oldest messages
                                queue.remove(0);
                            }
                            queue.push(wave);
                        }
                        HyperlaneCall::Drain => {
                            // just drains the queue later if there is a listener
                        }
                    }

                    if let Some(ext_tx) = ext.as_mut() {
                        for wave in queue {
                            match ext_tx.send(wave).await {
                                Ok(_) => {}
                                Err(err) => {
                                    ext = None;
                                    tx.try_send(HyperlaneCall::Wave(err.0));
                                }
                            }
                        }
                    }
                }
            });
        }

        Self { tx }
    }

    pub async fn send(&self, wave: UltraWave ) -> Result<(),MsgErr>{
        Ok(self.tx.send_timeout( HyperlaneCall::Wave(wave), Duration::from_secs(5)).await?)
    }

    pub fn tx(&self) -> mpsc::Sender<UltraWave> {
        let (tx,mut rx) = mpsc::channel(1024);
        let call_tx = self.tx.clone();
        tokio::spawn(async move {
            while let Some(wave) = rx.recv().await {
                call_tx.send(HyperlaneCall::Wave(wave)).await;
            }
        });
        tx
    }

    pub async fn rx(&self) -> mpsc::Receiver<UltraWave> {
        let (tx,rx) = mpsc::channel(1024);
        self.tx.send(HyperlaneCall::LaneExt(tx)).await;
        rx
    }
}



pub struct HyperwayInterchange {
    call_tx: mpsc::Sender<HyperwayInterchangeCall>,
    logger: PointLogger,
}

impl HyperwayInterchange {
    pub fn new(logger: PointLogger) -> Self {
        let (call_tx, mut call_rx) = mpsc::channel(1024);

        {
            let call_tx = call_tx.clone();
            let logger = logger.clone();
            tokio::spawn(async move {
                let mut hyperways = HashMap::new();
                while let Some(call) = call_rx.recv().await {
                    match call {
                        HyperwayInterchangeCall::Add(hyperway) => {
                            let mut rx = hyperway.inbound.rx().await;
                            hyperways.insert( hyperway.remote.clone(), hyperway );
                            let call_tx = call_tx.clone();
                            tokio::spawn( async move {
                                while let Some(wave) = rx.recv().await {
                                    call_tx.send_timeout(HyperwayInterchangeCall::Wave(wave), Duration::from_secs(60u64)).await;
                                }
                            });
                        }
                        HyperwayInterchangeCall::Remove(point) => {
                            hyperways.remove(&point);
                        }
                        HyperwayInterchangeCall::Wave(wave) => {
                            match wave.to().single_or() {
                                Ok(to) => {
                                    match hyperways.get(&to.point) {
                                        None => {}
                                        Some(hyperway) => {
                                            hyperway.outbound.send(wave).await;
                                        }
                                    }
                                }
                                Err(_) => {
                                    logger.warn("Hyperway Interchange cannot route Ripples, instead wrap in a Hop or Transport");
                                }
                            }
                        }
                        HyperwayInterchangeCall::Ext{point,tx} => {
                            match hyperways.get(&point) {
                                None => {}
                                Some(hyperway) => {
                                    tx.send(hyperway.ext())
                                }
                            }
                        }
                    }

                }
            });
        }

        Self {
            call_tx,
            logger,
        }
    }

    pub fn router(&self) -> Box<dyn Router> {
        Box::new(OutboundRouter::new(self.call_tx.clone()))
    }

    pub fn point(&self) -> &Point {
        &self.logger.point
    }

    pub fn add(&self, hyperway: Hyperway) {
        let call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            call_tx.send(HyperwayInterchangeCall::Add(hyperway)).await;
        });
    }

    pub fn remove(&mut self, hyperway: Point) {
        let call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            call_tx.send(HyperwayInterchangeCall::Remove(hyperway)).await;
        });
    }

    pub async fn route(&self, wave: UltraWave) {
        self.call_tx.send(HyperwayInterchangeCall::Wave(wave)).await;
    }
}

#[async_trait]
pub trait HyperRouter: Send + Sync {
    async fn route(&self, wave: HyperWave);
}

pub struct OutboundRouter {
    pub call_tx: mpsc::Sender<HyperwayInterchangeCall>,
}

impl OutboundRouter {
    pub fn new(call_tx: mpsc::Sender<HyperwayInterchangeCall>) -> Self {
        Self { call_tx }
    }
}

#[async_trait]
impl Router for OutboundRouter {
    async fn route(&self, wave: UltraWave) {
        self.call_tx.send(HyperwayInterchangeCall::Wave(wave)).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        self.call_tx.try_send(HyperwayInterchangeCall::Wave(wave));
    }
}

#[async_trait]
pub trait HyperAuthenticator: Send + Sync {
    async fn auth(&mut self, req: EntryReq) -> Result<HyperwayStub, MsgErr>;
}

pub struct TokenAuthenticator {
    pub token: Token,
    pub agent: Agent,
}

impl TokenAuthenticator {
    pub fn new(agent: Agent, token: Token) -> Self {
        Self { agent, token }
    }
}

#[async_trait]
impl HyperAuthenticator for TokenAuthenticator {
    async fn auth(&mut self, req: EntryReq) -> Result<HyperwayStub, MsgErr> {
        if let Substance::Token(token) = &*req.auth {
            if *token == self.token {
                Ok(HyperwayStub {
                    agent: self.agent.clone(),
                    remote: req
                        .remote
                        .ok_or::<MsgErr>("expected a remote entry selection".into())?,
                })
            } else {
                Err(MsgErr::forbidden())
            }
        } else {
            Err(MsgErr::forbidden())
        }
    }
}

pub struct AnonHyperAuthenticator {
    pub logger: RootLogger,
    pub lane_point_factory: Box<dyn PointFactory>,
}

impl AnonHyperAuthenticator {
    pub fn new(lane_point_factory: Box<dyn PointFactory>, logger: RootLogger) -> Self {
        Self {
            logger,
            lane_point_factory,
        }
    }
}

pub struct TokenAuthenticatorWithRemoteWhitelist {
    pub token: Token,
    pub agent: Agent,
    pub whitelist: HashSet<Point>,
}

impl TokenAuthenticatorWithRemoteWhitelist {
    pub fn new(agent: Agent, token: Token, whitelist: HashSet<Point>) -> Self {
        Self {
            agent,
            token,
            whitelist,
        }
    }
}

#[async_trait]
impl HyperAuthenticator for TokenAuthenticatorWithRemoteWhitelist {
    async fn auth(&mut self, req: EntryReq) -> Result<HyperwayStub, MsgErr> {
        if let Substance::Token(token) = &*req.auth {
            if *token == self.token {
                let remote = req
                    .remote
                    .ok_or::<MsgErr>("expected a remote entry selection".into())?;
                if self.whitelist.contains(&remote) {
                    Ok(HyperwayStub {
                        agent: self.agent.clone(),
                        remote,
                    })
                } else {
                    Err(MsgErr::forbidden_msg("remote is not part of the whitelist"))
                }
            } else {
                Err(MsgErr::forbidden())
            }
        } else {
            Err(MsgErr::forbidden())
        }
    }
}

#[async_trait]
impl HyperAuthenticator for AnonHyperAuthenticator {
    async fn auth(&mut self, req: EntryReq) -> Result<HyperwayStub, MsgErr> {
        let remote = req
            .remote
            .ok_or::<MsgErr>("required remote point request".into())?;

        Ok(HyperwayStub {
            agent: Agent::Anonymous,
            remote,
        })
    }
}

pub struct AnonHyperAuthenticatorAssignEndPoint {
    pub logger: RootLogger,
    pub lane_point_factory: Box<dyn PointFactory>,
    pub remote_point_factory: Box<dyn PointFactory>,
}

impl AnonHyperAuthenticatorAssignEndPoint {
    pub fn new(
        lane_point_factory: Box<dyn PointFactory>,
        remote_point_factory: Box<dyn PointFactory>,
        logger: RootLogger,
    ) -> Self {
        Self {
            logger,
            lane_point_factory,
            remote_point_factory,
        }
    }
}

#[async_trait]
impl HyperAuthenticator for AnonHyperAuthenticatorAssignEndPoint {
    async fn auth(&mut self, req: EntryReq) -> Result<HyperwayStub, MsgErr> {
        let remote = self.remote_point_factory.create().await?;

        Ok(HyperwayStub {
            agent: Agent::Anonymous,
            remote,
        })
    }
}

pub struct TokensFromHeavenHyperAuthenticatorAssignEndPoint {
    pub logger: RootLogger,
    pub tokens: Arc<DashMap<Token, HyperwayStub>>,
}

impl TokensFromHeavenHyperAuthenticatorAssignEndPoint {
    pub fn new(tokens: Arc<DashMap<Token, HyperwayStub>>, logger: RootLogger) -> Self {
        Self { logger, tokens }
    }
}

#[async_trait]
impl HyperAuthenticator for TokensFromHeavenHyperAuthenticatorAssignEndPoint {
    async fn auth(&mut self, auth_req: EntryReq) -> Result<HyperwayStub, MsgErr> {
        match &*auth_req.auth {
            Substance::Token(token) => {
                if let Some((_, stub)) = self.tokens.remove(token) {
                    return Ok(stub);
                } else {
                    return Err(MsgErr::forbidden());
                }
            }
            _ => {
                return Err(MsgErr::bad_request());
            }
        }
    }
}

pub struct FixedHyperwayInterchangeBuilder {
    pub hyperways: Vec<HyperwayStub>,
}

impl FixedHyperwayInterchangeBuilder {
    pub fn add(&mut self, stub: HyperwayStub) {
        self.hyperways.push(stub)
    }

    pub fn build(self) -> FixedHyperwayInterchange {}
}

pub struct TokenDispensingHyperwayInterchange {
    pub agent: Agent,
    pub logger: PointLogger,
    pub tokens: Arc<DashMap<Token, HyperwayStub>>,
    pub lane_point_factory: Box<dyn PointFactory>,
    pub remote_point_factory: Box<dyn PointFactory>,
    pub interchange: HyperwayInterchange,
}

impl TokenDispensingHyperwayInterchange {
    pub fn new(
        agent: Agent,
        router: Box<dyn HyperRouter>,
        lane_point_factory: Box<dyn PointFactory>,
        end_point_factory: Box<dyn PointFactory>,
        logger: PointLogger,
    ) -> Self {
        let tokens = Arc::new(DashMap::new());
        let authenticator = Box::new(TokensFromHeavenHyperAuthenticatorAssignEndPoint::new(
            tokens.clone(),
            logger.logger.clone(),
        ));
        let interchange = HyperwayInterchange::new(logger.clone());
        Self {
            agent,
            tokens,
            logger,
            lane_point_factory,
            remote_point_factory: end_point_factory,
            interchange,
        }
    }

    pub async fn dispense(&mut self) -> Result<(Token, HyperwayStub), MsgErr> {
        let token = Token::new_uuid();
        let remote_point = self.remote_point_factory.create().await?;
        let lane_point = self.lane_point_factory.create().await?;
        let logger = self.logger.point(lane_point);
        let stub = HyperwayStub {
            agent: self.agent.clone(),
            remote: remote_point,
        };
        self.tokens.insert(token.clone(), stub.clone());
        Ok((token, stub))
    }
}

impl Deref for TokenDispensingHyperwayInterchange {
    type Target = HyperwayInterchange;

    fn deref(&self) -> &Self::Target {
        &self.interchange
    }
}

impl DerefMut for TokenDispensingHyperwayInterchange {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.interchange
    }
}

pub struct VersionGate {
    router: HyperGate,
}

impl VersionGate {
    pub async fn unlock(&self, version: semver::Version) -> Result<HyperGate, String> {
        if version == *VERSION {
            Ok(self.router.clone())
        } else {
            Err("version mismatch".to_string())
        }
    }
}

#[derive(Clone)]
pub struct HyperGate {
    map: Arc<DashMap<InterchangeKind, InterchangeGate>>,
}

impl HyperGate {
    pub fn new(map: Arc<DashMap<InterchangeKind, InterchangeGate>>) -> Self {
        Self { map }
    }

    pub async fn enter(
        &self,
        req: EntryReq,
    ) -> Result<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>), MsgErr> {
        if let Some(gate) = self.map.get(&req.kind) {
            gate.enter(req).await
        } else {
            Err(MsgErr::from(
                format!("interchange not available: {}", req.kind.to_string()).as_str(),
            ))
        }
    }

    pub async fn add(&self, kind: InterchangeKind, hyperway: Hyperway) -> Result<(), MsgErr> {
        self.map
            .get(&kind)
            .ok_or("expected kind to be available")?
            .value()
            .jump_the_gate(hyperway);
        Ok(())
    }
}

#[derive(Clone)]
pub struct InterchangeGate {
    logger: PointLogger,
    auth: Arc<Mutex<Box<dyn HyperAuthenticator>>>,
    interchange: Arc<HyperwayInterchange>,
}

impl InterchangeGate {
    pub fn new(
        auth: Box<dyn HyperAuthenticator>,
        interchange: Arc<HyperwayInterchange>,
        logger: PointLogger,
    ) -> Self {
        let auth = Arc::new(Mutex::new(auth));

        Self {
            auth,
            interchange,
            logger,
        }
    }

    pub async fn enter(
        &self,
        req: EntryReq,
    ) -> Result<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>), MsgErr> {
        let stub = {
            let mut auth = self.auth.lock().await;
            auth.auth(req).await?
        };

        let (inbound, tx) = InboundLanes::new();
        let (outbound, rx) = Hyperlane::new();

        let hyperway = Hyperway {
            agent: stub.agent,
            remote: stub.remote,
            outbound,
            inbound,
        };

        self.interchange.add(hyperway);

        Ok((tx, rx))
    }

    pub fn jump_the_gate(&self, hyperway: Hyperway) {
        self.interchange.add(hyperway);
    }
}

pub struct HyperClient {
    pub agent: Agent,
    pub point: Point,
    pub factory: Box<dyn HyperClientConnectionFactory>,
    pub receiver_tx: mpsc::Sender<UltraWave>,
    pub sender_rx: mpsc::Receiver<UltraWave>,
}

impl HyperClient {
    pub fn new(
        agent: Agent,
        point: Point,
        factory: Box<dyn HyperClientConnectionFactory>,
        logger: PointLogger,
    ) -> Result<Hyperway, MsgErr> {
        let (sender_tx, sender_rx) = mpsc::channel(32 * 1024);
        let (receiver_tx, receiver_rx) = mpsc::channel(32 * 1024);

        let outbound = Hyperlane { tx: sender_tx };
        let inbound = InboundLanes { rx: receiver_rx };

        let hyperway = Hyperway {
            inbound,
            outbound,
            agent: agent.clone(),
            remote: point.clone(),
            logger,
        };

        let mut client = Self {
            agent,
            point,
            factory,
            sender_rx,
            receiver_tx,
        };

        client.start();

        Ok(hyperway)
    }

    pub fn start(mut self) {
        tokio::spawn(async move {
            loop {
                if let Ok((sender_tx, mut receiver_rx)) = self.factory.connect().await {
                    while let (Some(wave), index, _) = select_all(vec![
                        self.sender_rx.recv().boxed(),
                        receiver_rx.recv().boxed(),
                    ])
                    .await
                    {
                        if index == 0 {
                            sender_tx.send(wave).await;
                        } else {
                            self.receiver_tx.send(wave).await;
                        }
                    }
                } else {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        });
    }
}

pub trait HyperConnection {}

#[async_trait]
pub trait HyperClientConnectionFactory: Send + Sync {
    async fn connect(&self)
        -> Result<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>), MsgErr>;
}

pub struct LocalClientConnectionFactory {
    pub entry_req: EntryReq,
    pub entry_router: HyperGate,
}

impl LocalClientConnectionFactory {
    pub fn new(entry_req: EntryReq, entry_router: HyperGate) -> Self {
        Self {
            entry_req,
            entry_router,
        }
    }
}

#[async_trait]
impl HyperClientConnectionFactory for LocalClientConnectionFactory {
    async fn connect(
        &self,
    ) -> Result<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>), MsgErr> {
        self.entry_router.enter(self.entry_req.clone()).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AnonHyperAuthenticator, HyperGate, HyperRouter, HyperwayInterchange, InterchangeGate,
    };
    use chrono::{DateTime, Utc};
    use cosmic_api::command::request::create::PointFactoryU128;
    use cosmic_api::id::id::Point;
    use cosmic_api::log::RootLogger;
    use cosmic_api::substance::substance::Substance;
    use cosmic_api::sys::{EntryReq, InterchangeKind};
    use cosmic_api::wave::HyperWave;
    use dashmap::DashMap;
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Arc;

    #[no_mangle]
    pub(crate) extern "C" fn cosmic_uuid() -> String {
        "Uuid".to_string()
    }

    #[no_mangle]
    pub(crate) extern "C" fn cosmic_timestamp() -> DateTime<Utc> {
        Utc::now()
    }

    pub struct DummyRouter {}

    #[async_trait]
    impl HyperRouter for DummyRouter {
        async fn route(&self, wave: HyperWave) {
            println!("received hyperwave!");
        }
    }

    #[tokio::test]
    async fn hyper_test() {
        let point = Point::from_str("test").unwrap();
        let logger = RootLogger::default().point(point.clone());
        let interchange = Arc::new(HyperwayInterchange::new(
            logger.push("interchange").unwrap(),
        ));

        let point_factory =
            PointFactoryU128::new(point.push("portals").unwrap(), "portal-".to_string());
        let auth = Box::new(AnonHyperAuthenticator::new(
            Box::new(point_factory),
            logger.logger.clone(),
        ));

        let gate = InterchangeGate::new(auth, interchange, logger.push("gate").unwrap());

        let mut map = Arc::new(DashMap::new());
        map.insert(InterchangeKind::Cli, gate);

        let entry_router = HyperGate::new(map);

        let entry = EntryReq {
            kind: InterchangeKind::Cli,
            auth: Box::new(Substance::Empty),
            remote: Some(point.push("portal").unwrap()),
        };

        entry_router.enter(entry).await.unwrap();
    }
}

pub mod test {
    use crate::{
        AnonHyperAuthenticator, AnonHyperAuthenticatorAssignEndPoint, HyperRouter,
        HyperwayInterchange, InterchangeGate, TokenAuthenticatorWithRemoteWhitelist,
    };
    use cosmic_api::command::request::create::PointFactoryU128;
    use cosmic_api::error::MsgErr;
    use cosmic_api::id::id::{Point, ToPort};
    use cosmic_api::log::RootLogger;
    use cosmic_api::msg::MsgMethod;
    use cosmic_api::substance::substance::{Substance, Token};
    use cosmic_api::sys::{EntryReq, InterchangeKind};
    use cosmic_api::wave::{
        Agent, DirectedKind, DirectedProto, Exchanger, HyperWave, Pong, ProtoTransmitter,
        ReflectedKind, ReflectedProto, ReflectedWave, Router, TxRouter, Wave,
    };
    use std::collections::HashSet;
    use std::str::FromStr;
    use std::sync::Arc;

    pub struct TestRouter {}

    #[async_trait]
    impl HyperRouter for TestRouter {
        async fn route(&self, wave: HyperWave) {
            println!("Test Router routing!");
            //    todo!()
        }
    }

    #[tokio::test]
    pub async fn test_interchange() {
        let root_logger = RootLogger::default();
        let logger = root_logger.point(Point::from_str("point").unwrap());
        let interchange = Arc::new(HyperwayInterchange::new(
            logger.push("interchange").unwrap(),
        ));

        let lane_point_factory = Box::new(PointFactoryU128::new(
            Point::from_str("point:lanes").unwrap(),
            "lane-".to_string(),
        ));

        let auth = AnonHyperAuthenticator::new(lane_point_factory, root_logger.clone());

        let gate = InterchangeGate::new(Box::new(auth), interchange, logger.push("gate").unwrap());

        let less = Point::from_str("less").unwrap();
        let fae = Point::from_str("fae").unwrap();

        let (less_tx, mut less_rx) = gate
            .enter(EntryReq::new(
                InterchangeKind::Cli,
                less.clone(),
                Substance::Empty,
            ))
            .await
            .unwrap();
        let (fae_tx, mut fae_rx) = gate
            .enter(EntryReq::new(
                InterchangeKind::Cli,
                fae.clone(),
                Substance::Empty,
            ))
            .await
            .unwrap();

        let less_router = TxRouter::new(less_tx);
        let less_exchanger = Exchanger::new(less.clone().to_port(), Default::default());
        let less_transmitter = ProtoTransmitter::new(Arc::new(less_router), less_exchanger.clone());

        let fae_router = TxRouter::new(fae_tx);
        let fae_exchanger = Exchanger::new(fae.clone().to_port(), Default::default());
        let fae_transmitter = ProtoTransmitter::new(Arc::new(fae_router), fae_exchanger);

        {
            let fae = fae.clone();
            tokio::spawn(async move {
                let wave = fae_rx.recv().await.unwrap();
                let mut reflected = ReflectedProto::new();
                reflected.kind(ReflectedKind::Pong);
                reflected.status(200u16);
                reflected.to(wave.from().clone());
                reflected.from(fae.to_port());
                reflected.intended(wave.to());
                reflected.reflection_of(wave.id());
                let wave = reflected.build().unwrap();
                let wave = wave.to_ultra();
                fae_transmitter.route(wave).await;
            });
        }

        {
            let less_exchanger = less_exchanger.clone();
            tokio::spawn(async move {
                let wave = less_rx.recv().await.unwrap();
                if !wave.is_directed() {
                    less_exchanger.reflected(wave.to_reflected().unwrap()).await;
                }
            });
        }

        let mut hello = DirectedProto::new();
        hello.kind(DirectedKind::Ping);
        hello.to(fae.clone());
        hello.from(less.clone());
        hello.method(MsgMethod::new("Hello").unwrap());
        hello.body(Substance::Empty);
        let pong: Wave<Pong> = less_transmitter.direct(hello).await.unwrap();
        assert_eq!(pong.core.status.as_u16(), 200u16);
    }

    #[tokio::test]
    pub async fn test_connections() {
        let root_logger = RootLogger::default();
        let logger = root_logger.point(Point::from_str("point").unwrap());
        let interchange = Arc::new(HyperwayInterchange::new(
            logger.push("interchange").unwrap(),
        ));

        let lane_point_factory = Box::new(PointFactoryU128::new(
            Point::from_str("point:lanes").unwrap(),
            "lane-".to_string(),
        ));

        let auth = AnonHyperAuthenticator::new(lane_point_factory, root_logger.clone());

        let gate = InterchangeGate::new(Box::new(auth), interchange, logger.push("gate").unwrap());

        let less = Point::from_str("less").unwrap();
        let fae = Point::from_str("fae").unwrap();

        let (less_tx, mut less_rx) = gate
            .enter(EntryReq::new(
                InterchangeKind::Cli,
                less.clone(),
                Substance::Empty,
            ))
            .await
            .unwrap();
        let (fae_tx, mut fae_rx) = gate
            .enter(EntryReq::new(
                InterchangeKind::Cli,
                fae.clone(),
                Substance::Empty,
            ))
            .await
            .unwrap();

        let less_router = TxRouter::new(less_tx);
        let less_exchanger = Exchanger::new(less.clone().to_port(), Default::default());
        let less_transmitter = ProtoTransmitter::new(Arc::new(less_router), less_exchanger.clone());

        let fae_router = TxRouter::new(fae_tx);
        let fae_exchanger = Exchanger::new(fae.clone().to_port(), Default::default());
        let fae_transmitter = ProtoTransmitter::new(Arc::new(fae_router), fae_exchanger);

        {
            let fae = fae.clone();
            tokio::spawn(async move {
                let wave = fae_rx.recv().await.unwrap();
                let mut reflected = ReflectedProto::new();
                reflected.kind(ReflectedKind::Pong);
                reflected.status(200u16);
                reflected.to(wave.from().clone());
                reflected.from(fae.to_port());
                reflected.intended(wave.to());
                reflected.reflection_of(wave.id());
                let wave = reflected.build().unwrap();
                let wave = wave.to_ultra();
                fae_transmitter.route(wave).await;
            });
        }

        {
            let less_exchanger = less_exchanger.clone();
            tokio::spawn(async move {
                let wave = less_rx.recv().await.unwrap();
                if !wave.is_directed() {
                    less_exchanger.reflected(wave.to_reflected().unwrap()).await;
                }
            });
        }

        let mut hello = DirectedProto::new();
        hello.kind(DirectedKind::Ping);
        hello.to(fae.clone());
        hello.from(less.clone());
        hello.method(MsgMethod::new("Hello").unwrap());
        hello.body(Substance::Empty);
        let pong: Wave<Pong> = less_transmitter.direct(hello).await.unwrap();
        assert_eq!(pong.core.status.as_u16(), 200u16);
    }
}
