#![allow(warnings)]

use std::cell::{Cell, RefCell};
use cosmic_api::command::request::create::{PointFactory, PointFactoryU64, PointSegTemplate};
use cosmic_api::error::MsgErr;
use cosmic_api::frame::frame::PrimitiveFrame;
use cosmic_api::id::id::{Point, Port, ToPoint, ToPort, Version};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::substance::substance::{Errors, Substance, SubstanceKind, Token};
use cosmic_api::sys::{Knock, InterchangeKind, Sys};
use cosmic_api::util::uuid;
use cosmic_api::wave::{Agent, HyperWave, Method, Ping, Pong, Reflectable, Router, SysMethod, TxRouter, UltraWave, Wave};
use cosmic_api::VERSION;
use dashmap::DashMap;
use futures::future::select_all;
use futures::FutureExt;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::select;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError, TrySendError};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{broadcast, mpsc, Mutex, oneshot, RwLock};
use tokio::sync::oneshot::Sender;
use cosmic_api::particle::particle::Status;

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate lazy_static;

pub enum HyperwayKind {
  Mount,
  Ephemeral
}

pub struct Hyperway {
    pub remote: Point,
    pub agent: Agent,
    outbound: Hyperlane,
    inbound: Hyperlane,
}

impl Hyperway {
    pub fn new(
        remote: Point,
        agent: Agent,
    ) -> Self {

        Self {
            remote,
            agent,
            outbound: Hyperlane::new(),
            inbound: Hyperlane::new(),
        }
    }

    pub async fn mount(&self) -> HyperwayExt {
        let drop_tx = None;

        HyperwayExt {
            tx: self.outbound.tx(),
            rx: self.inbound.rx().await,
            drop_tx,
        }
    }

    pub async fn ephemeral(&self, drop_tx: oneshot::Sender<()> ) -> HyperwayExt {
        let drop_tx = Some(drop_tx);

        HyperwayExt {
            tx: self.outbound.tx(),
            rx: self.inbound.rx().await,
            drop_tx,
        }
    }


}

pub struct HyperwayExt {
    drop_tx: Option<oneshot::Sender<()>>,
    pub tx: mpsc::Sender<UltraWave>,
    pub rx: mpsc::Receiver<UltraWave>
}

impl HyperwayExt {
    pub fn new(tx: mpsc::Sender<UltraWave>, rx: mpsc::Receiver<UltraWave>) -> Self {
        let drop_tx = None;
        Self {
            tx,
            rx,
            drop_tx
        }
    }

    pub fn new_with_drop(tx: mpsc::Sender<UltraWave>, rx: mpsc::Receiver<UltraWave>, drop_tx: oneshot::Sender<()>) -> Self {
        let drop_tx = Some(drop_tx);
        Self {
            tx,
            rx,
            drop_tx
        }
    }

    pub fn add_drop_tx( &mut self, drop_tx: oneshot::Sender<()>) {
        self.drop_tx.replace(drop_tx);
    }
}

impl Drop for HyperwayExt {
    fn drop(&mut self) {
       match self.drop_tx.take()  {
           None => {}
           Some(drop_tx) => {
               drop_tx.send(());
           }
       }
    }
}

#[derive(Clone)]
pub struct HyperwayStub {
    pub agent: Agent,
    pub remote: Point,
}

impl HyperwayStub {
    pub fn from_point( remote: Point ) -> Self {
        Self {
            agent: remote.to_agent(),
            remote
        }
    }

    pub fn new( remote: Point, agent: Agent ) -> Self {
        Self {
            agent,
            remote
        }
    }
}

pub enum HyperwayInterchangeCall {
    Wave(UltraWave),
    Add(Hyperway),
    Remove(Point),
    Mount { stub: HyperwayStub, tx: oneshot::Sender<HyperwayExt> }
}

pub enum HyperlaneCall {
    Drain,
    Ext(mpsc::Sender<UltraWave>),
    ResetExt,
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
                        HyperlaneCall::Ext(new) => {
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
                        HyperlaneCall::ResetExt => {
                            ext = None;
                        }
                    }

                    if let Some(ext_tx) = ext.as_mut() {
                        for wave in queue.drain(..) {
                            match ext_tx.send(wave).await {
                                Ok(_) => {}
                                Err(err) => {
                                    tx.send(HyperlaneCall::ResetExt).await;
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
        self.tx.send(HyperlaneCall::Ext(tx)).await;
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
                        HyperwayInterchangeCall::Mount {  stub, tx } => {
                            match hyperways.get(&stub.remote) {
                                None => {}
                                Some(hyperway) => {
                                    tx.send(hyperway.mount().await);
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

    pub async fn mount(&self, stub: HyperwayStub) -> Result<HyperwayExt, MsgErr> {
        let call_tx = self.call_tx.clone();
        let (tx,rx) = oneshot::channel();
        tokio::spawn(async move {
            call_tx.send(HyperwayInterchangeCall::Mount { stub,tx}).await;
        });
        Ok(rx.await?)
    }

    pub fn add(&self, hyperway: Hyperway) {
        let call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            call_tx.send(HyperwayInterchangeCall::Add(hyperway)).await;
        });
    }

    pub fn remove(&self, hyperway: Point) {
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
pub trait HyperAuthenticator: Send + Sync + Clone + Sized {
    async fn auth(&self, knock: Knock) -> Result<HyperwayStub, MsgErr>;
}

#[derive(Clone)]
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
    async fn auth(&self, req: Knock) -> Result<HyperwayStub, MsgErr> {
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

#[derive(Clone)]
pub struct AnonHyperAuthenticator {
    pub logger: RootLogger,
    pub lane_point_factory: Arc<dyn PointFactory>,
}

impl AnonHyperAuthenticator {
    pub fn new(lane_point_factory: Arc<dyn PointFactory>, logger: RootLogger) -> Self {
        Self {
            logger,
            lane_point_factory,
        }
    }
}

#[derive(Clone)]
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
    async fn auth(&self, req: Knock) -> Result<HyperwayStub, MsgErr> {
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
    async fn auth(&self, req: Knock) -> Result<HyperwayStub, MsgErr> {
        let remote = req
            .remote
            .ok_or::<MsgErr>("required remote point request".into())?;

        Ok(HyperwayStub {
            agent: Agent::Anonymous,
            remote,
        })
    }
}

#[derive(Clone)]
pub struct AnonHyperAuthenticatorAssignEndPoint {
    pub logger: RootLogger,
    pub lane_point_factory: Arc<dyn PointFactory>,
    pub remote_point_factory: Arc<dyn PointFactory>,
}

impl AnonHyperAuthenticatorAssignEndPoint {
    pub fn new(
        lane_point_factory: Arc<dyn PointFactory>,
        remote_point_factory: Arc<dyn PointFactory>,
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
    async fn auth(&self, knock: Knock) -> Result<HyperwayStub, MsgErr> {
        let remote = self.remote_point_factory.create().await?;

        Ok(HyperwayStub {
            agent: Agent::Anonymous,
            remote,
        })
    }
}

#[derive(Clone)]
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
    async fn auth(&self, auth_req: Knock) -> Result<HyperwayStub, MsgErr> {
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

    pub async fn dispense(&self) -> Result<(Token, HyperwayStub), MsgErr> {
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
    router: HyperGateSelector,
}

impl VersionGate {
    pub async fn unlock(&self, version: semver::Version) -> Result<HyperGateSelector, String> {
        if version == *VERSION {
            Ok(self.router.clone())
        } else {
            Err("version mismatch".to_string())
        }
    }
}

#[async_trait]
pub trait HyperGate: Send+Sync {
    async fn knock(
        &self,
        knock: Knock,
    ) -> Result<HyperwayExt, MsgErr>;

    async fn jump(&self, kind: InterchangeKind, stub: HyperwayStub ) -> Result<HyperwayExt,MsgErr>;
}

#[derive(Clone)]
pub struct HyperGateSelector {
    map: Arc<DashMap<InterchangeKind,Box<dyn HyperGate>>>,
}

impl HyperGateSelector {
    pub fn new(map: Arc<DashMap<InterchangeKind, Box<dyn HyperGate>>>) -> Self {
        Self { map }
    }
}

#[async_trait]
impl HyperGate for HyperGateSelector {

    async fn knock(
        &self,
        req: Knock,
    ) -> Result<HyperwayExt, MsgErr> {
        if let Some(gate) = self.map.get(&req.kind) {
            gate.value().knock(req).await
        } else {
            Err(MsgErr::from(
                format!("interchange not available: {}", req.kind.to_string()).as_str(),
            ))
        }
    }

    async fn jump(&self, kind: InterchangeKind, stub: HyperwayStub ) -> Result<HyperwayExt,MsgErr>
    {
        self.map
            .get(&kind)
            .ok_or("expected kind to be available")?
            .value()
            .jump(kind, stub).await
    }
}

#[derive(Clone)]
pub struct InterchangeGate<A> where A: HyperAuthenticator {
    logger: PointLogger,
    auth: A,
    interchange: Arc<HyperwayInterchange>,
}
impl <A> InterchangeGate<A> where A: HyperAuthenticator {
    fn new(
        auth: A,
        interchange: Arc<HyperwayInterchange>,
        logger: PointLogger,
    ) -> Self {
        Self {
            auth,
            interchange,
            logger,
        }
    }
}

impl <A> InterchangeGate<A> where A: HyperAuthenticator {
    async fn enter(&self, stub: HyperwayStub ) -> Result<HyperwayExt, MsgErr> {
        let hyperway= Hyperway::new( stub.remote.clone(), stub.agent.clone() );

        let (drop_tx,drop_rx) = oneshot::channel();
        let ext = hyperway.ephemeral(drop_tx).await;
        self.interchange.add(hyperway);

        let interchange = self.interchange.clone();
        tokio::spawn( async move {
            drop_rx.await;
            interchange.remove(stub.remote)
        });

        Ok(ext)
    }

}

#[async_trait]
impl <A> HyperGate for InterchangeGate<A> where A:HyperAuthenticator {
    async fn knock(
        &self,
        knock: Knock,
    ) -> Result<HyperwayExt, MsgErr> {
        let stub = self.auth.auth(knock).await?;
        self.enter(stub).await
    }

    async fn jump(&self, _kind: InterchangeKind, stub: HyperwayStub) -> Result<HyperwayExt, MsgErr> {
        self.enter(stub).await
    }
}

#[derive(Clone)]
pub struct MountInterchangeGate<A> where A: HyperAuthenticator{
    logger: PointLogger,
    auth: A,
    interchange: Arc<HyperwayInterchange>,
}

impl <A> MountInterchangeGate<A> where A: HyperAuthenticator{
    pub fn new(
        auth: A,
        interchange: Arc<HyperwayInterchange>,
        logger: PointLogger,
    ) -> Self {
        Self {
            auth,
            interchange,
            logger,
        }
    }
}

#[async_trait]
impl <A> HyperGate for MountInterchangeGate<A> where A: HyperAuthenticator  {
    async fn knock(
        &self,
        knock: Knock,
    ) -> Result<HyperwayExt, MsgErr> {

        let stub = self.auth.auth(knock).await?;

        let ext = self.interchange.mount(stub).await?;

        Ok(ext)
    }

    async fn jump(&self, _kind: InterchangeKind, stub: HyperwayStub) -> Result<HyperwayExt, MsgErr> {
        Ok(self.interchange.mount(stub).await?)
    }
}




pub struct HyperClient {
    pub stub: HyperwayStub,
    tx: mpsc::Sender<UltraWave>,
    status_tx: broadcast::Sender<HyperClientStatus>
}

impl HyperClient {
    pub fn new(
        stub: HyperwayStub,
        factory: Box<dyn HyperwayExtFactory>,
        to_client_listener_tx: mpsc::Sender<UltraWave>
    ) -> Result<HyperClient, MsgErr> {

        let (to_hyperway_tx, from_client_rx) = mpsc::channel(1024);
        let (status_tx,mut status_rx) = broadcast::channel(1);

        let mut from_runner_rx = HyperClientRunner::new(factory, from_client_rx, status_tx.clone());

        let mut client = Self {
            stub,
            tx: to_hyperway_tx,
            status_tx:status_tx.clone()
        };

        tokio::spawn( async move {
           async fn relay( mut from_runner_rx: mpsc::Receiver<UltraWave>, to_client_listener_tx: mpsc::Sender<UltraWave>) -> Result<(),MsgErr> {
               while let Some(wave) = from_runner_rx.recv().await {
                   to_client_listener_tx.send(wave).await?;
               }
               Ok(())
           }
           relay(from_runner_rx, to_client_listener_tx).await;
        });

        Ok(client)
    }

    pub fn router(&self) -> TxRouter {
        TxRouter::new(self.tx.clone())
    }
}


#[derive(Clone)]
pub enum HyperClientStatus {
    Unknown,
    Connecting,
    Ready,
    Panic
}

pub enum HyperClientCall {
    Wave(UltraWave)
}

pub struct HyperClientRunner {
    ext: Option<HyperwayExt>,
    factory: Box<dyn HyperwayExtFactory>,
    status_tx: broadcast::Sender<HyperClientStatus>,
    to_client_tx: mpsc::Sender<UltraWave>,
    from_client_rx: mpsc::Receiver<UltraWave>
}

impl HyperClientRunner {
    pub fn new(factory: Box<dyn HyperwayExtFactory>, from_client_rx: mpsc::Receiver<UltraWave>, status_tx: broadcast::Sender<HyperClientStatus>) -> mpsc::Receiver<UltraWave>{
        let (to_client_tx,from_runner_rx) = mpsc::channel(1024);
        let runner =Self {
            ext:None,
            factory,
            to_client_tx,
            from_client_rx,
            status_tx,
        };
        runner.start();
        from_runner_rx
    }

    async fn start(mut self) {
        self.status_tx.send(HyperClientStatus::Unknown );
        loop {
            async fn connect(runner: &mut HyperClientRunner ) -> Result<(),MsgErr> {
                runner.status_tx.send(HyperClientStatus::Connecting);
                loop {
                    match tokio::time::timeout( Duration::from_secs(30),runner.factory.create()).await {
                        Ok(Ok(ext)) => {
                            runner.ext.replace(ext);
                            runner.status_tx.send(HyperClientStatus::Ready);
                            return Ok(())
                        }
                        _ => {
                            // we eventually need to know WHY it failed... if it was that
                            // credentials were rejected, then this client must halt and
                            // not attempt to reconnect... if it was a timeout we need to keep retrying
                        }
                    }
                    // wait a little while before attempting to reconnect
                    // maybe add exponential backoff later
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }

            async fn relay(runner: &mut HyperClientRunner) -> Result<(),MsgErr> {
                let ext = runner.ext.as_mut().ok_or::<MsgErr>("must reconnect".into() )?;
                while let (Some(wave),index,_) = futures::future::select_all(vec![runner.from_client_rx.recv().boxed(), ext.rx.recv().boxed()]).await {
                    match index {
                        0 => {
                            // message comes from client, therefore it should go towards ext
                            match ext.tx.send(wave).await {
                                Ok(_) => {}
                                Err(err) => {
                                    // wave gets lost... need to requeue it somehow...
//                                    runner.to_client_tx.try_send(err.0);
                                    return Err(MsgErr::from_500("ext failure"));
                                }
                            }
                        },
                        1 => {
                            // message comes from ext therefor it should go to client
                            runner.to_client_tx.send(wave).await;
                        }
                        _ => {
                            // error!
                        }
                    }

                }
                Ok(())
            }

            loop {
                match connect(& mut self).await {
                    Ok(_) => {}
                    Err(_) => {
                        self.status_tx.send(HyperClientStatus::Panic);
                        return;
                    }
                }

                match relay(& mut self).await {
                    Ok(_) => {
                        // natural end... this runner is ready to be dropped
                        break;
                    }
                    Err(_) => {
                        // some error occurred when relaying therefore we need to reconnect
                        self.ext = None;
                    }
                }
            }
        }
    }
}

#[async_trait]
pub trait HyperwayExtFactory: Send + Sync {
    async fn create(&self)
                    -> Result<HyperwayExt, MsgErr>;
}

pub struct LocalHyperwayGateUnlocker {
    pub knock: Knock,
    pub gate: Arc<dyn HyperGate>,
}

impl LocalHyperwayGateUnlocker {
    pub fn new(knock: Knock, gate: Arc<dyn HyperGate>) -> Self {
        Self {
            knock: knock,
            gate,
        }
    }
}

#[async_trait]
impl HyperwayExtFactory for LocalHyperwayGateUnlocker {
    async fn create(
        &self,
    ) -> Result<HyperwayExt, MsgErr> {
        self.gate.knock(self.knock.clone()).await
    }
}

pub struct DirectInterchangeMountHyperwayExtFactory {
    pub stub: HyperwayStub,
    pub interchange: Arc<HyperwayInterchange>
}

impl DirectInterchangeMountHyperwayExtFactory {
    pub fn new( stub: HyperwayStub, interchange: Arc<HyperwayInterchange>) -> Self {
        Self {
            stub,
            interchange
        }
    }
}

#[async_trait]
impl HyperwayExtFactory for DirectInterchangeMountHyperwayExtFactory {
    async fn create(
        &self,
    ) -> Result<HyperwayExt, MsgErr> {
        self.interchange.mount(self.stub.clone()).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{AnonHyperAuthenticator, HyperGate, HyperGateSelector, HyperRouter, HyperwayInterchange, InterchangeGate};
    use chrono::{DateTime, Utc};
    use cosmic_api::command::request::create::PointFactoryU64;
    use cosmic_api::id::id::Point;
    use cosmic_api::log::RootLogger;
    use cosmic_api::substance::substance::Substance;
    use cosmic_api::sys::{Knock, InterchangeKind};
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

    /*
    #[tokio::test]
    async fn hyper_test() {
        let point = Point::from_str("test").unwrap();
        let logger = RootLogger::default().point(point.clone());
        let interchange = Arc::new(HyperwayInterchange::new(
            logger.push("interchange").unwrap(),
        ));

        let point_factory =
            PointFactoryU64::new(point.push("portals").unwrap(), "portal-".to_string());
        let auth = AnonHyperAuthenticator::new(
            Arc::new(point_factory),
            logger.logger.clone(),
        );

        let gate = InterchangeGate::new(auth, interchange, logger.push("gate").unwrap());

        let mut map = Arc::new(DashMap::new());
        map.insert(InterchangeKind::Cli, Box::new(gate));

        let entry_router = HyperGateSelector::new(map);

        let knock = Knock {
            kind: InterchangeKind::Cli,
            auth: Box::new(Substance::Empty),
            remote: Some(point.push("cli").unwrap()),
        };

        entry_router.knock(knock).await.unwrap();
    }

     */
}

pub mod test {
    use crate::{AnonHyperAuthenticator, AnonHyperAuthenticatorAssignEndPoint, DirectInterchangeMountHyperwayExtFactory, HyperClient, HyperGate, HyperGateSelector, HyperRouter, Hyperway, HyperwayInterchange, HyperwayStub, InterchangeGate, LocalHyperwayGateUnlocker, MountInterchangeGate, TokenAuthenticatorWithRemoteWhitelist};
    use cosmic_api::command::request::create::PointFactoryU64;
    use cosmic_api::error::MsgErr;
    use cosmic_api::id::id::{Point, ToPort};
    use cosmic_api::log::RootLogger;
    use cosmic_api::msg::MsgMethod;
    use cosmic_api::substance::substance::{Substance, Token};
    use cosmic_api::sys::{Knock, InterchangeKind};
    use cosmic_api::wave::{
        Agent, DirectedKind, DirectedProto, Exchanger, HyperWave, Pong, ProtoTransmitter,
        ReflectedKind, ReflectedProto, ReflectedWave, Router, TxRouter, Wave,
    };
    use std::collections::{HashMap, HashSet};
    use std::str::FromStr;
    use std::sync::{Arc};
    use dashmap::DashMap;
    use lazy_static::lazy_static;
    use tokio::sync::mpsc;

    lazy_static! {
    pub static ref LESS: Point = Point::from_str("space:users:less").expect("point");
    pub static ref FAE: Point = Point::from_str("space:users:fae").expect("point");
}


    pub struct TestRouter {}

    #[async_trait]
    impl HyperRouter for TestRouter {
        async fn route(&self, wave: HyperWave) {
            println!("Test Router routing!");
            //    todo!()
        }
    }

    #[tokio::test]
    pub async fn test() {
        let root_logger = RootLogger::default();
        let logger = root_logger.point(Point::from_str("point").unwrap());
        let interchange = Arc::new(HyperwayInterchange::new(
            logger.push("interchange").unwrap(),
        ));

        interchange.add(Hyperway::new(LESS.clone(), LESS.to_agent() ));
        interchange.add(Hyperway::new(FAE.clone(), FAE.to_agent() ));

        let lane_point_factory = Arc::new(PointFactoryU64::new(
            Point::from_str("point:lanes").unwrap(),
            "lane-".to_string(),
        ));

        let auth = AnonHyperAuthenticator::new(lane_point_factory, root_logger.clone());
        let gate = MountInterchangeGate::new(auth, interchange.clone(), logger.push("gate").unwrap());
        let mut gates : Arc<DashMap<InterchangeKind,Box<dyn HyperGate>>>= Arc::new(DashMap::new());
        gates.insert( InterchangeKind::Cli, Box::new(gate) );
        let gate = Arc::new(HyperGateSelector::new( gates ));

        let less_stub = HyperwayStub::from_point(LESS.clone());
        let fae_stub = HyperwayStub::from_point(FAE.clone());

        let less_factory = LocalHyperwayGateUnlocker::new(Knock::new(InterchangeKind::Cli, LESS.clone(), Substance::Empty), gate.clone() );
        let fae_factory = DirectInterchangeMountHyperwayExtFactory::new(fae_stub.clone(), interchange.clone() );

        let (less_client_listener_tx,mut less_rx) = mpsc::channel(1024);
        let (fae_client_listener_tx,mut fae_rx) = mpsc::channel(1024);

        let less_client = HyperClient::new(less_stub.clone(), Box::new(less_factory), less_client_listener_tx).unwrap();
        let fae_client = HyperClient::new(fae_stub.clone(), Box::new(fae_factory), fae_client_listener_tx).unwrap();

        let less_router = less_client.router();
        let less_exchanger = Exchanger::new(LESS.clone().to_port(), Default::default());
        let less_transmitter = ProtoTransmitter::new(Arc::new(less_router), less_exchanger.clone());

        let fae_router = fae_client.router();
        let fae_exchanger = Exchanger::new(FAE.clone().to_port(), Default::default());
        let fae_transmitter = ProtoTransmitter::new(Arc::new(fae_router), fae_exchanger);

        {
            let fae = FAE.clone();
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
        hello.to(FAE.clone().to_port());
        hello.from(LESS.clone().to_port());
        hello.method(MsgMethod::new("Hello").unwrap());
        hello.body(Substance::Empty);
        let pong: Wave<Pong> = less_transmitter.direct(hello).await.unwrap();
        assert_eq!(pong.core.status.as_u16(), 200u16);
    }
}
