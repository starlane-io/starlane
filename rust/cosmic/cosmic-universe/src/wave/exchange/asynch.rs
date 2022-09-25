use std::sync::Arc;
use tokio::sync::mpsc;
use crate::{Agent, ReflectedCore, Surface, UniErr};
use crate::wave::exchange::{BroadTxRouter, Exchanger, InCtxDef, ProtoTransmitterBuilderDef, ProtoTransmitterDef, RootInCtxDef, SetStrategy};
use crate::wave::{BounceBacks, DirectedProto, FromReflectedAggregate, Handling, Pong, RecipientSelector, ReflectedAggregate, ReflectedProto, Scope, UltraWave, Wave};
use crate::wave::core::cmd::CmdMethod;
use crate::wave::core::CoreBounce;

#[async_trait]
impl Router for TxRouter {
    async fn route(&self, wave: UltraWave) {
        self.tx.send(wave).await;
    }
}

#[async_trait]
impl Router for BroadTxRouter {
    async fn route(&self, wave: UltraWave) {
        self.tx.send(wave);
    }
}

#[async_trait]
pub trait Router: Send + Sync {
    async fn route(&self, wave: UltraWave);
}

#[derive(Clone)]
pub struct AsyncRouter {
    pub router: Arc<dyn Router>
}

impl AsyncRouter {
    pub fn new( router: Arc<dyn Router>) -> Self {
        Self {
            router
        }
    }
}

#[async_trait]
impl Router for AsyncRouter {
    async fn route(&self, wave: UltraWave) {
        self.router.route(wave).await
    }
}

pub type ProtoTransmitter = ProtoTransmitterDef<AsyncRouter>;

impl ProtoTransmitter {
    pub fn new(router: Arc<dyn Router>, exchanger: Exchanger) -> ProtoTransmitter {
        let router = AsyncRouter::new(router);
        Self {
            from: SetStrategy::None,
            to: SetStrategy::None,
            agent: SetStrategy::Fill(Agent::Anonymous),
            scope: SetStrategy::Fill(Scope::None),
            handling: SetStrategy::Fill(Handling::default()),
            router,
            exchanger,
        }
    }

    pub async fn direct<D, W>(&self, wave: D) -> Result<W, UniErr>
    where
        W: FromReflectedAggregate,
        D: Into<DirectedProto>,
    {
        let mut wave: DirectedProto = wave.into();

        self.prep_direct(&mut wave);

        let directed = wave.build()?;

        match directed.bounce_backs() {
            BounceBacks::None => {
                self.router.route(directed.to_ultra()).await;
                FromReflectedAggregate::from_reflected_aggregate(ReflectedAggregate::None)
            }
            _ => {
                let reflected_rx = self.exchanger.exchange(&directed).await;
                self.router.route(directed.to_ultra()).await;
                let reflected_agg = reflected_rx.await?;
                FromReflectedAggregate::from_reflected_aggregate(reflected_agg)
            }
        }
    }

    pub async fn bounce_from(&self, to: &Surface, from: &Surface) -> bool {
        let mut directed = DirectedProto::ping();
        directed.from(from.clone());
        directed.to(to.clone());
        directed.method(CmdMethod::Bounce);
        match self.direct(directed).await {
            Ok(pong) => {
                let pong: Wave<Pong> = pong;
                pong.is_ok()
            }
            Err(_) => false,
        }
    }

    pub async fn bounce(&self, to: &Surface) -> bool {
        let mut directed = DirectedProto::ping();
        directed.to(to.clone());
        directed.method(CmdMethod::Bounce);
        match self.direct(directed).await {
            Ok(pong) => {
                let pong: Wave<Pong> = pong;
                pong.is_ok()
            }
            Err(_) => false,
        }
    }

    pub async fn route(&self, wave: UltraWave) {
        self.router.route(wave).await
    }

    pub async fn reflect<W>(&self, wave: W) -> Result<(), UniErr>
    where
        W: Into<ReflectedProto>,
    {
        let mut wave: ReflectedProto = wave.into();

        self.prep_reflect(&mut wave);

        let wave = wave.build()?;
        let wave = wave.to_ultra();
        self.router.route(wave).await;

        Ok(())
    }
}

pub type ProtoTransmitterBuilder = ProtoTransmitterBuilderDef<AsyncRouter>;

impl ProtoTransmitterBuilder {
    pub fn new(router: Arc<dyn Router>, exchanger: Exchanger) -> ProtoTransmitterBuilder {
        let router = AsyncRouter::new(router);
        Self {
            from: SetStrategy::None,
            to: SetStrategy::None,
            agent: SetStrategy::Fill(Agent::Anonymous),
            scope: SetStrategy::Fill(Scope::None),
            handling: SetStrategy::Fill(Handling::default()),
            router,
            exchanger,
        }
    }
}

pub type RootInCtx=RootInCtxDef<ProtoTransmitter>;

pub type InCtx<'a,I> = InCtxDef<'a,I,ProtoTransmitter>;

impl <'a,I> InCtx<'a,I> {

        pub fn push_from(self, from: Surface) -> InCtx<'a, I> {
        let mut transmitter = self.transmitter.clone();
        transmitter.to_mut().from = SetStrategy::Override(from);
        InCtx{
            root: self.root,
            input: self.input,
            logger: self.logger.clone(),
            transmitter,
        }
    }

}

#[async_trait]
pub trait DirectedHandlerSelector {
    fn select<'a>(&self, select: &'a RecipientSelector<'a>) -> Result<&dyn DirectedHandler, ()>;
}

#[async_trait]
pub trait DirectedHandler: Send + Sync {
    async fn handle(&self, ctx: RootInCtx) -> CoreBounce;

    async fn bounce(&self, ctx: RootInCtx) -> CoreBounce {
        CoreBounce::Reflected(ReflectedCore::ok())
    }
}

#[derive(Clone)]
pub struct TxRouter {
    pub tx: mpsc::Sender<UltraWave>,
}

impl TxRouter {
    pub fn new(tx: mpsc::Sender<UltraWave>) -> Self {
        Self { tx }
    }
}
