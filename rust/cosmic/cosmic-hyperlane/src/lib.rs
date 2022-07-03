#![allow(warnings)]

use dashmap::DashMap;
use futures::future::select_all;
use futures::FutureExt;
use cosmic_api::error::MsgErr;
use cosmic_api::command::request::create::{
    PointFactory, PointFactoryU128, PointSegTemplate,
};
use cosmic_api::frame::frame::PrimitiveFrame;
use cosmic_api::id::id::{Point, Port, ToPoint, ToPort, Version};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::substance::substance::{
    Errors, Substance, SubstanceKind, Token,
};
use cosmic_api::sys::{EntryReq, InterchangeKind, Sys};
use cosmic_api::util::uuid;
use cosmic_api::wave::{Agent, HyperWave, Method, Ping, Reflectable, Pong, SysMethod, Wave, UltraWave, Router};
use cosmic_api::VERSION;
use std::collections::HashMap;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

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
    pub logger: PointLogger,
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
    pub fn new(router: Box<dyn HyperRouter>, logger: PointLogger) -> Self {
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
                            router.route(wave).await;
                        }
                        Some(HyperwayCall::Out(wave)) => {
                            let point = match wave.to().single_or() {
                                Ok(port) => {
                                    match hyperways.get(&port.point) {
                                        None => {
                                            logger.error(format!("attempt to send wave from '{}' to hyperway '{}' which is not present in this HyperwayInterchange", wave.from().to_string(), wave.to().unwrap_single().to_string()) );
                                        }
                                        Some(hyperway) => {
                                            hyperway.value().outbound(wave).await;
                                        }
                                    }
                                }
                                Err(err) => {
                                    logger.error( err.to_string() );
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
        Box::new( OutboundRouter::new(self.call_tx.clone()))
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
        self.call_tx.send( HyperwayCall::Out(wave)).await;
    }
}

#[async_trait]
pub trait HyperRouter: Send + Sync {
    async fn route(&self, wave: HyperWave);
}

pub struct OutboundRouter {
    pub call_tx: mpsc::Sender<HyperwayCall>
}

impl OutboundRouter {
    pub fn new( call_tx: mpsc::Sender<HyperwayCall>) -> Self {
        Self {
            call_tx
        }
    }
}

#[async_trait]
impl Router for OutboundRouter {
    async fn route(&self, wave: UltraWave ) {
        self.call_tx.send( HyperwayCall::Out(wave)).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        self.call_tx.try_send( HyperwayCall::Out(wave));
    }
}

#[async_trait]
pub trait HyperAuthenticator: Send + Sync {
    async fn auth(&self, req: EntryReq) -> Result<HyperwayStub, MsgErr>;
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

#[async_trait]
impl HyperAuthenticator for AnonHyperAuthenticator {
    async fn auth(&self, req: EntryReq) -> Result<HyperwayStub, MsgErr> {
        todo!()
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
    async fn auth(&self, req: EntryReq) -> Result<HyperwayStub, MsgErr> {
        todo!()
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
    async fn auth(&self, auth_req: EntryReq) -> Result<HyperwayStub, MsgErr> {
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
    pub end_point_factory: Box<dyn PointFactory>,
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
        let interchange = HyperwayInterchange::new(router, logger.clone());
        Self {
            agent,
            tokens,
            logger,
            lane_point_factory,
            end_point_factory,
            interchange,
        }
    }

    pub fn dispense(&mut self) -> Result<(Token, HyperwayStub), MsgErr> {
        let token = Token::new_uuid();
        let end_point = self.end_point_factory.create()?;
        let lane_point = self.lane_point_factory.create()?;
        let logger = self.logger.point(lane_point);
        let stub = HyperwayStub {
            agent: self.agent.clone(),
            logger,
            remote: end_point,
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
    map: HashMap<InterchangeKind, HyperGate>,
}

impl InterchangeEntryRouter {
    pub fn new(map: HashMap<InterchangeKind, HyperGate>) -> Self {
        Self { map }
    }

    pub async fn enter(
        &self,
        req: EntryReq,
    ) -> Result<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>), MsgErr> {
        if let Some(gate) = self.map.get(&req.interchange) {
            gate.enter(req).await
        } else {
            Err(MsgErr::from(
                format!("interchange not available: {}", req.interchange.to_string()).as_str(),
            ))
        }
    }
}

#[derive(Clone)]
pub struct HyperGate {
    pub logger: PointLogger,
    pub auth: Arc<Mutex<Box<dyn HyperAuthenticator>>>,
    pub interchange: Arc<HyperwayInterchange>,
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

        let hyperway = Hyperway {
            agent: stub.agent,
            remote: stub.remote,
            logger: stub.logger,
            outbound,
            inbound,
        };

        self.interchange.add(hyperway);

        Ok((tx, rx))
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
