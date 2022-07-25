use crate::driver::{
    Core, CoreSkel, Driver, DriverFactory, DriverLifecycleCall, DriverSkel, DriverStatus,
};
use crate::star::{LayerInjectionRouter, StarSkel};
use crate::Platform;
use cosmic_api::command::request::create::PointFactoryU64;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, ToPoint, ToPort};
use cosmic_api::id::TraversalInjection;
use cosmic_api::sys::{Assign, InterchangeKind};
use cosmic_api::wave::Agent::Anonymous;
use cosmic_api::wave::RecipientSelector;
use cosmic_api::wave::{
    Agent, CoreBounce, DirectedHandler, InCtx, ProtoTransmitter, ProtoTransmitterBuilder,
    RootInCtx, Signal, UltraWave, Wave,
};
use cosmic_api::wave::{DirectedHandlerSelector, SetStrategy, TxRouter};
use cosmic_api::State;
use cosmic_hyperlane::{
    AnonHyperAuthenticator, AnonHyperAuthenticatorAssignEndPoint, HyperGate, Hyperway, HyperwayExt,
    HyperwayInterchange, InterchangeGate,
};
use dashmap::DashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};

pub struct ControlDriverFactory<P>
where
    P: Platform,
{
    pub star_skel: StarSkel<P>,
}

impl<P> DriverFactory<P> for ControlDriverFactory<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    fn create(&self, skel: DriverSkel<P>) -> Box<dyn Driver> {
        Box::new(ControlDriver::new(skel))
    }
}

#[derive(DirectedHandler)]
pub struct ControlDriver<P>
where
    P: Platform,
{
    states: Arc<DashMap<Point, Arc<RwLock<ControlState>>>>,
    skel: DriverSkel<P>,
    runner_tx: mpsc::Sender<ControlCall<P>>,
}

#[routes]
impl<P> ControlDriver<P>
where
    P: Platform,
{
    fn new(skel: DriverSkel<P>) -> Self {
        let states = Arc::new(DashMap::new());
        let runner_tx = ControlDriverRunner::new(skel.clone(), states.clone());
        Self {
            skel,
            states,
            runner_tx,
        }
    }
}

#[async_trait]
impl<P> Driver for ControlDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Control
    }

    async fn status(&self) -> DriverStatus {
        let (rtn, rtn_rx) = oneshot::channel();
        self.runner_tx.send(ControlCall::GetStatus(rtn)).await;
        rtn_rx.await.unwrap_or(DriverStatus::Unknown)
    }

    async fn lifecycle(&mut self, event: DriverLifecycleCall) -> Result<DriverStatus, MsgErr> {
        self.runner_tx.send(ControlCall::Lifecycle(event)).await;
        Ok(self.status().await)
    }

    async fn ex(&self, point: &Point) -> Result<Box<dyn Core>, MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.runner_tx
            .send(ControlCall::GetCore {
                point: point.clone(),
                rtn,
            })
            .await;
        let core = rtn_rx.await??;
        let core = Box::new(core);
        Ok(core)
    }

    async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.runner_tx
            .send(ControlCall::Assign { assign, rtn })
            .await;
        rtn_rx.await??;
        Ok(())
    }
}

pub enum ControlCall<P>
where
    P: Platform,
{
    Lifecycle(DriverLifecycleCall),
    GetCore {
        point: Point,
        rtn: oneshot::Sender<Result<ControlCore<P>, MsgErr>>,
    },
    FromExternal(UltraWave),
    Assign {
        assign: Assign,
        rtn: oneshot::Sender<Result<(), MsgErr>>,
    },
GetStatus(oneshot::Sender<DriverStatus>)
}

pub struct ControlDriverRunner<P>
where
    P: Platform,
{
    pub skel: DriverSkel<P>,
    pub states: Arc<DashMap<Point, Arc<RwLock<ControlState>>>>,
    pub status: DriverStatus,
    pub external_router: Option<Arc<TxRouter>>,
    pub runner_tx: mpsc::Sender<ControlCall<P>>,
    pub runner_rx: mpsc::Receiver<ControlCall<P>>,
}

impl<P> ControlDriverRunner<P>
where
    P: Platform,
{
    fn new(
        skel: DriverSkel<P>,
        states: Arc<DashMap<Point, Arc<RwLock<ControlState>>>>,
    ) -> mpsc::Sender<ControlCall<P>> {
        let (tx, rx) = mpsc::channel(1024);
        let status = DriverStatus::Pending;
        let mut runner = Self {
            states,
            skel,
            status,
            external_router: None,
            runner_tx: tx.clone(),
            runner_rx: rx,
        };
        runner.start();

        tx
    }

    fn log<O, E: ToString>(&self, result: Result<O, E>) -> Result<O, E> {
        match result {
            Ok(o) => Ok(o),
            Err(err) => {
                self.skel.logger.error(err.to_string());
                Err(err)
            }
        }
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.runner_rx.recv().await {
                match call {
                    ControlCall::Lifecycle(call)=> {
                        match call {
                            DriverLifecycleCall::Init => {
                                self.init().await;
                            }
                            DriverLifecycleCall::Shutdown => {}
                        }
                    }
                    ControlCall::GetCore { point, rtn } => {
                        rtn.send(self.log(self.core(&point)));
                    }
                    ControlCall::FromExternal(wave) => {
                        self.log(self.route(wave));
                    }
                    ControlCall::Assign { assign, rtn } => {
                        rtn.send(self.log(self.assign(assign)));
                    }
                    ControlCall::GetStatus(rtn) => {
                        rtn.send( self.status.clone() );
                    }
                }
            }
        });
    }

    async fn init(&mut self) -> Result<(), MsgErr> {
        self.status = DriverStatus::Initializing;
        let point = self.skel.star_skel.point.push("controls").unwrap();
        let logger = self.skel.star_skel.logger.point(point.clone());
        let remote_point_factory =
            Arc::new(PointFactoryU64::new(point.clone(), "control-".to_string()));
        let auth = AnonHyperAuthenticatorAssignEndPoint::new(remote_point_factory);
        let interchange = Arc::new(HyperwayInterchange::new(logger.clone()));
        let hyperway = Hyperway::new(Point::from_str("REMOTE::control")?, Agent::HyperUser);
        let mut hyperway_ext = hyperway.mount().await;
        interchange.internal(hyperway);
        let external_router = Arc::new(TxRouter::new(hyperway_ext.tx.clone()));
        self.external_router = Some(external_router.clone());

        let gate = Arc::new(InterchangeGate::new(auth, interchange, logger));
        {
            let runner_tx = self.runner_tx.clone();
            tokio::spawn(async move {
                while let Some(wave) = hyperway_ext.rx.recv().await {
                    runner_tx.send(ControlCall::FromExternal(wave)).await;
                }
            });
        }

        match self
            .skel
            .star_skel
            .machine
            .api
            .add_interchange(InterchangeKind::Control, gate)
            .await
        {
            Ok(_) => {}
            Err(_) => {}
        }
        self.status = DriverStatus::Ready;
        Ok(())
    }

    fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        if self.states.contains_key(&assign.details.stub.point) {
            return Err("already assigned to this star".into());
        }
        self.states.insert(
            assign.details.stub.point.clone(),
            Arc::new(RwLock::new(ControlState::new())),
        );
        Ok(())
    }

    fn route(&self, hop: UltraWave) -> Result<(), MsgErr> {
        let agent = hop.agent().clone();
        let hop = hop.to_signal()?;
        let transport = hop.unwrap_from_hop()?;
        if transport.agent != agent {
            return Err("control transport agent mismatch".into());
        }
        let core = self.core(&transport.to.point)?;
        let wave = transport.unwrap_from_transport()?;
        if *wave.agent() != agent {
            return Err("control transport agent mismatch".into());
        }
        core.route(wave);
        Ok(())
    }

    fn core(&self, point: &Point) -> Result<ControlCore<P>, MsgErr> {
        let state = self
            .states
            .get(point)
            .ok_or::<MsgErr>("could not find state for control core".into())?
            .value()
            .clone();
        let router = LayerInjectionRouter::new(
            self.skel.star_skel.clone(),
            point.clone().to_port().with_layer(Layer::Core),
        );
        let mut transmitter =
            ProtoTransmitterBuilder::new(Arc::new(router), self.skel.star_skel.exchanger.clone());
        transmitter.from = SetStrategy::Override(point.clone().to_port().with_layer(Layer::Core));

        let skel = CoreSkel::new(point.clone(), transmitter.build());
        Ok(ControlCore::new(skel, state))
    }
}

impl<P> ControlDriver<P> where P: Platform {}

#[derive(DirectedHandler)]
pub struct ControlCore<P>
where
    P: Platform,
{
    skel: CoreSkel<P>,
    state: Arc<RwLock<ControlState>>,
}

impl<P> ControlCore<P>
where
    P: Platform,
{
    pub fn new(skel: CoreSkel<P>, state: Arc<RwLock<ControlState>>) -> Self {
        Self { skel, state }
    }

    pub async fn route(&self, wave: UltraWave) {
        self.skel.transmitter.route(wave).await;
    }
}

#[routes]
impl<P> Core for ControlCore<P> where P: Platform {}

pub struct ControlState {}

impl ControlState {
    pub fn new() -> Self {
        Self {}
    }
}
