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
use futures::future::select_all;
use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use dashmap::DashMap;
use tokio::sync::{mpsc, Mutex};
use tokio::sync::mpsc::Receiver;

#[macro_use]
extern crate async_trait;

pub struct Hyperway {
    pub agent: Agent,
    pub logger: PointLogger,
    pub remote: Point,
    outbound: OutboundLanes,
    inbound: InboundLanes,
}

impl Hyperway {
    pub fn new(
        agent: Agent,
        logger: PointLogger,
        remote: Point,
        outbound: OutboundLanes,
        inbound: InboundLanes,
    ) -> Self {
        Self {
            agent,
            logger,
            remote,
            outbound,
            inbound,
        }
    }
}

#[derive(Clone)]
pub struct HyperwayStub {
    pub agent: Agent,
    pub remote: Point,
}

pub struct HyperwayOut {
    pub agent: Agent,
    pub remote: Point,
    outbound: OutboundLanes,
    logger: PointLogger,
}

pub struct HyperwayIn {
    pub agent: Agent,
    pub remote: Point,
    inbound: InboundLanes,
    logger: PointLogger,
}

impl HyperwayOut {
    pub async fn outbound(&self, wave: UltraWave) {
        self.outbound.send(wave).await;
    }
}

impl HyperwayIn {
    pub async fn inbound(&mut self) -> Option<UltraWave> {
        self.inbound.receive().await
    }

    pub async fn inbound_into_call(&mut self) -> Option<HyperwayCall> {
        let wave = self.inbound().await;
        match wave {
            None => None,
            Some(wave) => {
                let hyperwave = HyperWave {
                    from: self.remote.clone(),
                    wave,
                };
                Some(HyperwayCall::Wave(hyperwave))
            }
        }
    }
}

impl Hyperway {
    pub fn split(self) -> (HyperwayIn, HyperwayOut) {
        (
            HyperwayIn {
                remote: self.remote.clone(),
                inbound: self.inbound,
                logger: self.logger.push("inbound").unwrap(),
                agent: self.agent.clone(),
            },
            HyperwayOut {
                remote: self.remote.clone(),
                outbound: self.outbound,
                logger: self.logger.push("outbound").unwrap(),
                agent: self.agent.clone(),
            },
        )
    }
}

pub enum HyperwayCall {
    Out(UltraWave),
    Wave(HyperWave),
    Add(HyperwayIn),
    Remove(Point),
}

/// doesn't do much now, but the eventual idea is to have it handle multiple lanes
/// and send to them based on priority
pub struct OutboundLanes {
    pub tx: mpsc::Sender<UltraWave>,
}

impl OutboundLanes {
    pub fn new() -> (Self, mpsc::Receiver<UltraWave>) {
        let (tx, rx) = mpsc::channel(1024);
        (Self { tx }, rx)
    }
}

impl OutboundLanes {
    async fn send(&self, wave: UltraWave) {
        self.tx.send(wave).await;
    }
}

/// doesn't do much now, but the eventual idea is to have it handle multiple lanes
/// and draw from them based on priority
pub struct InboundLanes {
    pub rx: mpsc::Receiver<UltraWave>,
}

impl InboundLanes {
    pub fn new() -> (Self, mpsc::Sender<UltraWave>) {
        let (tx, rx) = mpsc::channel(1024);
        (Self { rx }, tx)
    }
}

impl InboundLanes {
    async fn receive(&mut self) -> Option<UltraWave> {
        self.rx.recv().await
    }
}

pub struct HyperwayInterchange {
    hyperways: Arc<DashMap<Point, HyperwayOut>>,
    call_tx: mpsc::Sender<HyperwayCall>,
    logger: PointLogger,
}

impl HyperwayInterchange {
    pub fn new(logger: PointLogger) -> Self {
        let (call_tx, mut call_rx) = mpsc::channel(1024);
        let hyperways: Arc<DashMap<Point, HyperwayOut>> = Arc::new(DashMap::new());

        {
            let logger = logger.clone();
            let hyperways = hyperways.clone();
            let hyperway_outs = hyperways.clone();
            tokio::spawn(async move {
                let mut hyperway_ins: HashMap<Point, HyperwayIn> = HashMap::new();
                loop {
                    let mut rx = vec![];
                    let mut index_to_point = HashMap::new();

                    for (index, (point, hyperway)) in hyperway_ins.iter_mut().enumerate() {
                        index_to_point.insert(index, point.clone());
                        rx.push(hyperway.inbound_into_call().boxed())
                    }

                    rx.push(call_rx.recv().boxed());

                    let (result, index, _) = select_all(rx).await;
                    match result {
                        Some(HyperwayCall::Add(hyperway_in)) => {
                            hyperway_ins.insert(hyperway_in.remote.clone(), hyperway_in);
                        }
                        Some(HyperwayCall::Remove(hyperway)) => {
                            hyperway_ins.remove(&hyperway);
                            hyperway_outs.remove(&hyperway);
                        }
                        Some(HyperwayCall::Wave(wave)) => {
                            match wave.to().single_or() {
                                Ok(to) => {
                                    if to.point != wave.from {
                                        match hyperway_outs.get(&to.clone().to_point()) {
                                            None => {
                                                logger.warn(format!("hyperway not found in interchange: {}", to.point.to_string()));
                                            }
                                            Some(hyperway_out) => {
                                                hyperway_out.value().outbound(wave.wave).await;
                                            }
                                        }
                                    } else {
                                        logger.warn("illegal attempt to route a wave back to it's origin (cannot have same 'to' and 'from' points)");
                                    }
                                }
                                Err(_) => {
                                    logger.warn("interchange can only route to single recipients (no ripples)");
                                }
                            }
                        }
                        Some(HyperwayCall::Out(wave)) => {
println!("Hyperway sending Out... ");
                            match wave.to().single_or() {
                                Ok(port) => {
                                    match hyperways.get(&port.point) {
                                        None => {
                                            logger.warn(format!("hyperway not found in interchange: {}", port.point.to_string()));
                                        }
                                        Some(hyperway) => {
println!("wave matched to hyperway..." );
                                            hyperway.value().outbound(wave).await;
                                        }
                                    }
                                },
                                Err(err) => {
                                    logger.warn("interchange can only route to single recipients (no ripples)");
                                }
                            };
                        }
                        None => {
                            match index_to_point.get(&index) {
                                Some(hyperway) => {
                                    hyperway_ins.remove(hyperway);
                                    hyperway_outs.remove(hyperway);
                                }
                                None => {
                                    // this means call_rx returned None... we are done here.
                                    break;
                                }
                            }
                        }
                    }
                }
            });
        }

        Self {
            hyperways,
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
        let (hyperway_in, hyperway_out) = hyperway.split();
        self.hyperways
            .insert(hyperway_out.remote.clone(), hyperway_out);
        let call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            call_tx.send(HyperwayCall::Add(hyperway_in)).await;
        });
    }

    pub fn remove(&mut self, hyperway: Point) {
        self.hyperways.remove(&hyperway);
        let call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            call_tx.send(HyperwayCall::Remove(hyperway)).await;
        });
    }

    pub async fn outbound(&self, wave: UltraWave) {
        self.call_tx.send(HyperwayCall::Out(wave)).await;
    }
}

#[async_trait]
pub trait HyperRouter: Send + Sync {
    async fn route(&self, wave: HyperWave);
}

pub struct OutboundRouter {
    pub call_tx: mpsc::Sender<HyperwayCall>,
}

impl OutboundRouter {
    pub fn new(call_tx: mpsc::Sender<HyperwayCall>) -> Self {
        Self { call_tx }
    }
}

#[async_trait]
impl Router for OutboundRouter {
    async fn route(&self, wave: UltraWave) {
        self.call_tx.send(HyperwayCall::Out(wave)).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        self.call_tx.try_send(HyperwayCall::Out(wave));
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
        let interchange = HyperwayInterchange::new( logger.clone());
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
    router: InterchangeEntryRouter,
}

impl VersionGate {
    pub async fn unlock(&self, version: semver::Version) -> Result<InterchangeEntryRouter, String> {
        if version == *VERSION {
            Ok(self.router.clone())
        } else {
            Err("version mismatch".to_string())
        }
    }
}

#[derive(Clone)]
pub struct InterchangeEntryRouter {
    map: Arc<DashMap<InterchangeKind, HyperGate>>,
}

impl InterchangeEntryRouter {
    pub fn new(map: Arc<DashMap<InterchangeKind, HyperGate>>) -> Self {
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

    pub async fn add(&self, kind: InterchangeKind, hyperway: Hyperway ) -> Result<(),MsgErr> {
        self.map.get(&kind).ok_or("expected kind to be available")?.value().jump_the_gate(hyperway);
        Ok(())
    }
}

#[derive(Clone)]
pub struct HyperGate {
    logger: PointLogger,
    auth: Arc<Mutex<Box<dyn HyperAuthenticator>>>,
    interchange: Arc<HyperwayInterchange>,
}

impl HyperGate {
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
        let (outbound, rx) = OutboundLanes::new();

        let logger = self.logger.push("lane")?;

        let hyperway = Hyperway {
            agent: stub.agent,
            remote: stub.remote,
            logger,
            outbound,
            inbound,
        };

        self.interchange.add(hyperway);

        Ok((tx, rx))
    }

    pub fn jump_the_gate(&self, hyperway: Hyperway )  {
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
    pub fn new(agent: Agent, point: Point, factory: Box<dyn HyperClientConnectionFactory>, logger: PointLogger ) -> Result<Hyperway,MsgErr> {
        let (sender_tx,sender_rx) = mpsc::channel(32*1024);
        let (receiver_tx,receiver_rx) = mpsc::channel(32*1024);


        let outbound = OutboundLanes{ tx: sender_tx };
        let inbound = InboundLanes{ rx: receiver_rx };

        let hyperway = Hyperway {
            inbound,
            outbound,
            agent: agent.clone(),
            remote: point.clone(),
            logger
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
                if let Ok((sender_tx,mut receiver_rx)) = self.factory.connect().await {
                    while let (Some(wave),index,_)= select_all( vec![self.sender_rx.recv().boxed(),receiver_rx.recv().boxed()] ).await
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


#[async_trait]
pub trait HyperClientConnectionFactory: Send+Sync {
  async fn connect(&self) -> Result<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>),MsgErr>;
}

pub struct LocalClientConnectionFactory {
    pub entry_req: EntryReq,
    pub entry_router: InterchangeEntryRouter
}

impl LocalClientConnectionFactory {
    pub fn new( entry_req: EntryReq, entry_router: InterchangeEntryRouter ) -> Self {
        Self {
            entry_req,
            entry_router
        }
    }
}

#[async_trait]
impl HyperClientConnectionFactory for LocalClientConnectionFactory {
    async fn connect(&self) -> Result<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>), MsgErr> {
        self.entry_router.enter( self.entry_req.clone() ).await
    }
}


#[cfg(test)]
mod tests {
    use crate::{
        AnonHyperAuthenticator, HyperGate, HyperRouter, HyperwayInterchange, InterchangeEntryRouter,
    };
    use chrono::{DateTime, Utc};
    use cosmic_api::command::request::create::PointFactoryU128;
    use cosmic_api::id::id::Point;
    use cosmic_api::log::RootLogger;
    use cosmic_api::substance::substance::Substance;
    use cosmic_api::sys::{EntryReq, InterchangeKind};
    use cosmic_api::wave::HyperWave;
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Arc;
    use dashmap::DashMap;

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

        let gate = HyperGate::new(auth, interchange, logger.push("gate").unwrap());

        let mut map = Arc::new(DashMap::new());
        map.insert(InterchangeKind::Cli, gate);

        let entry_router = InterchangeEntryRouter::new(map);

        let entry = EntryReq {
            kind: InterchangeKind::Cli,
            auth: Box::new(Substance::Empty),
            remote: Some(point.push("portal").unwrap()),
        };

        entry_router.enter(entry).await.unwrap();
    }
}


pub mod test {
    use std::collections::HashSet;
    use std::str::FromStr;
    use std::sync::Arc;
    use cosmic_api::command::request::create::PointFactoryU128;
    use cosmic_api::error::MsgErr;
    use cosmic_api::id::id::{Point, ToPort};
    use cosmic_api::log::RootLogger;
    use cosmic_api::msg::MsgMethod;
    use cosmic_api::substance::substance::{Substance, Token};
    use cosmic_api::sys::{EntryReq, InterchangeKind};
    use cosmic_api::wave::{Agent, DirectedKind, DirectedProto, Exchanger, HyperWave, Pong, ProtoTransmitter, ReflectedKind, ReflectedProto, ReflectedWave, Router, TxRouter, Wave};
    use crate::{AnonHyperAuthenticator, AnonHyperAuthenticatorAssignEndPoint, HyperGate, HyperRouter, HyperwayInterchange, TokenAuthenticatorWithRemoteWhitelist};

    pub struct TestRouter {

    }

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
        let logger = root_logger.point( Point::from_str("point").unwrap());
        let interchange = Arc::new(HyperwayInterchange::new(
            logger.push("interchange").unwrap(),
        ));

        let lane_point_factory = Box::new(PointFactoryU128::new(Point::from_str("point:lanes").unwrap(), "lane-".to_string() ));

        let auth = AnonHyperAuthenticator::new(
            lane_point_factory,
            root_logger.clone(),
        );

        let gate = HyperGate::new(
            Box::new(auth),
            interchange,
            logger.push("gate").unwrap(),
        );

        let less = Point::from_str("less").unwrap();
        let fae  = Point::from_str("fae").unwrap();

        let (less_tx, mut less_rx) = gate.enter(EntryReq::new(InterchangeKind::Cli, less.clone(), Substance::Empty )).await.unwrap();
        let (fae_tx, mut fae_rx) = gate.enter(EntryReq::new(InterchangeKind::Cli, fae.clone(), Substance::Empty )).await.unwrap();

        let less_router = TxRouter::new(less_tx);
        let less_exchanger = Exchanger::new( less.clone().to_port(), Default::default() );
        let less_transmitter = ProtoTransmitter::new( Arc::new(less_router), less_exchanger.clone() );

        let fae_router = TxRouter::new(fae_tx);
        let fae_exchanger = Exchanger::new( fae.clone().to_port(), Default::default() );
        let fae_transmitter = ProtoTransmitter::new( Arc::new(fae_router), fae_exchanger );


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
        hello.to( fae.clone() );
        hello.from(less.clone());
        hello.method(MsgMethod::new("Hello").unwrap());
        hello.body(Substance::Empty);
        let pong: Wave<Pong> = less_transmitter.direct(hello).await.unwrap();
        assert_eq!(pong.core.status.as_u16(), 200u16);

    }
}