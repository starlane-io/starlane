use crate::machine::MachineSkel;
use crate::star::StarCall::LayerTraversalInjection;
use crate::star::{LayerInjectionRouter, StarSkel, StarState, StateApi, StateCall};
use crate::{PlatErr, Platform, RegistryApi};
use cosmic_api::command::command::common::StateSrc;
use cosmic_api::config::config::bind::RouteSelector;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{
    BaseKind, Kind, Layer, Point, Port, ToBaseKind, ToPoint, ToPort, TraversalLayer, Uuid,
};
use cosmic_api::id::{BaseSubKind, StarKey, Traversal, TraversalInjection};
use cosmic_api::log::{PointLogger, Tracker};
use cosmic_api::parse::model::Subst;
use cosmic_api::parse::route_attribute;
use cosmic_api::particle::particle::{Details, Status, Stub};
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, AssignmentKind, Sys};
use cosmic_api::util::{log, ValuePattern};
use cosmic_api::wave::{
    Bounce, CoreBounce, DirectedCore, DirectedHandler, DirectedHandlerSelector, DirectedKind,
    DirectedProto, DirectedWave, Exchanger, InCtx, Ping, Pong, ProtoTransmitter,
    ProtoTransmitterBuilder, RecipientSelector, ReflectedCore, ReflectedWave, RootInCtx, Router,
    SetStrategy, SysMethod, UltraWave, Wave, WaveKind,
};
use cosmic_api::{Registration, State, HYPERUSER};
use dashmap::DashMap;
use futures::future::select_all;
use futures::FutureExt;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::Receiver;
use tokio::sync::watch::Ref;
use tokio::sync::{broadcast, mpsc, oneshot, watch, RwLock};

#[derive(Clone,Eq,PartialEq)]
pub enum BootPhase {
    Star,
    DriverDriver,
    Final,
    Cross,
}

impl BootPhase {
    pub fn init(&self, kind: &Kind) -> bool {
        match self {
            BootPhase::Star => kind.to_base() == BaseKind::Star,
            BootPhase::DriverDriver => kind.to_base() == BaseKind::Driver,
            BootPhase::Final => {
                kind.to_base() != BaseKind::Star && kind.to_base() != BaseKind::Driver
            }
            BootPhase::Cross => false,
        }
    }
}

pub enum BootPhaseStatus {
    Pending,
    Initializing,
    Ready,
    Retry,
    Fatal,
}

pub enum DriversCall<P>
where
    P: Platform,
{
    Init {
        phase: BootPhase,
        watch: watch::Sender<BootPhaseStatus>,
    },
    AddDriver {
        kind: Kind,
        driver: DriverApi,
    },
    Visit(Traversal<UltraWave>),
    Kinds(oneshot::Sender<Vec<Kind>>),
    Assign {
        assign: Assign,
        rtn: oneshot::Sender<Result<(), MsgErr>>,
    },
    Drivers(oneshot::Sender<HashMap<Kind, DriverApi>>),
    Status {
        kind: Kind,
        rtn: oneshot::Sender<DriverStatus>,
    },
}

#[derive(Clone)]
pub struct DriversApi<P>
where
    P: Platform,
{
    tx: mpsc::Sender<DriversCall<P>>,
}

impl<P> DriversApi<P>
where
    P: Platform,
{
    pub fn new(tx: mpsc::Sender<DriversCall<P>>) -> Self {
        Self { tx }
    }

    pub async fn visit(&self, traversal: Traversal<UltraWave>) {
        self.tx.send(DriversCall::Visit(traversal)).await;
    }

    pub async fn kinds(&self) -> Result<Vec<Kind>, MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.tx.send(DriversCall::Kinds(rtn)).await;
        Ok(rtn_rx.await?)
    }

    pub async fn drivers(&self) -> Result<HashMap<Kind, DriverApi>, MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.tx.send(DriversCall::Drivers(rtn)).await;
        Ok(rtn_rx.await?)
    }

    pub async fn init(&self, phase: BootPhase) -> watch::Receiver<BootPhaseStatus> {
        let (watch, watch_rx) = watch::channel(BootPhaseStatus::Pending);
        self.tx.send(DriversCall::Init { phase, watch }).await;
        watch_rx
    }

    pub async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        println!("DriversApi ENTERING ASSIGN");
        let (rtn, rtn_rx) = oneshot::channel();
        self.tx.send(DriversCall::Assign { assign, rtn }).await;
        let rtn = Ok(rtn_rx.await??);
        println!("DriversApi RETURNING FROM ASSIGN");
        rtn
    }
}

#[derive(DirectedHandler)]
pub struct Drivers<P>
where
    P: Platform + 'static,
{
    port: Port,
    skel: StarSkel<P>,
    factories: HashMap<Kind, Box<dyn DriverFactory<P>>>,
    drivers: HashMap<Kind, DriverApi>,
    call_rx: mpsc::Receiver<DriversCall<P>>,
    call_tx: mpsc::Sender<DriversCall<P>>,
    statuses_rx: Arc<DashMap<Kind, watch::Receiver<DriverStatus>>>,
}

impl<P> Drivers<P>
where
    P: Platform + 'static,
{
    pub fn new(
        port: Port,
        skel: StarSkel<P>,
        factories: HashMap<Kind, Box<dyn DriverFactory<P>>>,
        call_tx: mpsc::Sender<DriversCall<P>>,
        call_rx: mpsc::Receiver<DriversCall<P>>,
    ) -> DriversApi<P> {
        let statuses_rx = Arc::new(DashMap::new());
        let drivers = HashMap::new();
        let mut drivers = Self {
            port,
            skel,
            drivers,
            call_rx,
            call_tx: call_tx.clone(),
            statuses_rx,
            factories,
        };

        drivers.start();

        DriversApi::new(call_tx)
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.call_rx.recv().await {
                match call {
                    DriversCall::Init { phase, watch } => {
                        self.boot(phase, watch).await;
                    }
                    DriversCall::AddDriver { kind, driver } => {
                        self.drivers.insert(kind, driver);
                    }
                    DriversCall::Visit(traversal) => {
                        self.visit(traversal).await;
                    }
                    DriversCall::Kinds(rtn) => {
                        rtn.send(self.kinds());
                    }
                    DriversCall::Assign { assign, rtn } => {
                        rtn.send(self.assign(assign).await).unwrap_or_default();
                    }
                    DriversCall::Drivers(rtn) => {
                        rtn.send(self.drivers.clone()).unwrap_or_default();
                    }
                    DriversCall::Status { .. } => {}
                }
            }
        });
    }

    pub fn kinds(&self) -> Vec<Kind> {
        self.factories.keys().cloned().collect_vec()
    }

    pub async fn boot(&self, phase: BootPhase, watch: watch::Sender<BootPhaseStatus>) {

        for driver in self.drivers.values() {
            driver.signal_boot_phase(phase.clone()).await;
        }

        let mut factories = self.factories.clone();
        factories.retain(|kind| !self.drivers.contains_key(kind));
        factories.retain(|kind| phase.init(kind));

        let call_tx = self.call_tx.clone();
        let skel = self.skel.clone();
        let logger = self.skel.logger.clone();
        let statuses_rx = self.statuses_rx.clone();
        let point = self.skel.point.push("drivers").unwrap();
        tokio::spawn(async move {
            watch.send(BootPhaseStatus::Initializing);
            let mut statuses = HashMap::new();
            for (kind, factory) in factories {
                let point = point.push(kind.as_point_segments()).unwrap();
                let router = Arc::new(LayerInjectionRouter::new(
                    skel.clone(),
                    point.clone().to_port().with_layer(Layer::Guest),
                ));
                let (shell_request_tx, shell_request_rx) = mpsc::channel(1024);
                let (shell_tx, shell_rx) = mpsc::channel(1024);
                let driver_skel = DriverSkel::new(
                    point.clone(),
                    router,
                    shell_request_tx,
                    skel.clone(),
                    logger.point(point.clone()),
                );
                let (status_tx, status_rx) = watch::channel(DriverStatus::Pending);
                statuses.insert(kind.clone(), status_rx.clone());
                statuses_rx.insert(kind.clone(), status_rx.clone());

                let call_tx = call_tx.clone();
                let logger = logger.clone();
                tokio::spawn(async move {
                    let driver = factory.create(driver_skel, status_tx).await;
                    match driver {
                        Ok(driver) => {
                            let shell = DriverRunner::new(
                                point,
                                skel.clone(),
                                driver,
                                shell_tx,
                                shell_rx,
                                status_rx.clone()
                            );
                            let driver = DriverApi::new(shell, factory.kind());
                            call_tx
                                .send(DriversCall::AddDriver { kind, driver })
                                .await
                                .unwrap_or_default();
                        }
                        Err(err) => {
                            logger.error(err.to_string());
                        }
                    }
                });
            }

            loop {
                let mut fatals = 0;
                let mut retries = 0;
                let mut readies = 0;

                if statuses.is_empty() {
                    break;
                }

                for (kind, status_rx) in statuses.iter() {
                    match status_rx.borrow().clone() {
                        DriverStatus::Ready => {
                            readies = readies + 1;
                        }
                        DriverStatus::Retry(msg) => {
                            logger.warn(format!("DRIVER RETRY: {} {}", kind.to_string(), msg));
                            retries = retries + 1;
                        }
                        DriverStatus::Fatal(msg) => {
                            logger.error(format!("DRIVER FATAL: {} {}", kind.to_string(), msg));
                            fatals = fatals + 1;
                        }
                        _ => {
                            break;
                        }
                    }
                }

                if readies == statuses.len() {
                    watch.send(BootPhaseStatus::Ready);
                    break;
                } else if fatals > 0 {
                    watch.send(BootPhaseStatus::Fatal);
                    break;
                } else if retries > 0 {
                    watch.send(BootPhaseStatus::Retry);
                } else {
                    watch.send(BootPhaseStatus::Initializing);
                }

                for status_rx in statuses.iter_mut() {
                    let mut rx = vec![];
                    rx.push(status_rx.changed().boxed());
                    let (result, _, _) = select_all(rx).await;
                    if logger.result(result).is_err() {
                        break;
                    }
                }
            }
        });
    }
}

impl<P> Drivers<P>
where
    P: Platform,
{
    pub async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        let driver = self
            .drivers
            .get(&assign.details.stub.kind)
            .ok_or::<MsgErr>(
                format!(
                    "kind not supported by these Drivers: {}",
                    assign.details.stub.kind.to_string()
                )
                .into(),
            )?;
        driver.assign(assign).await
    }

    pub async fn handle(&self, wave: DirectedWave) -> Result<ReflectedCore, MsgErr> {
        let record = self
            .skel
            .registry
            .locate(&wave.to().single_or()?.point)
            .await
            .map_err(|e| e.to_cosmic_err())?;
        let driver = self
            .drivers
            .get(&record.details.stub.kind)
            .ok_or::<MsgErr>("do not handle this kind of driver".into())?;
        driver.handle(wave).await
    }

    /*
    pub async fn sys(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, MsgErr> {
        if let Sys::Assign(assign) = &ctx.input {
            match self.drivers.get(&assign.details.stub.kind) {
                None => Err(format!(
                    "do not have driver for Kind: <{}>",
                    assign.details.stub.kind.to_string()
                )
                .into()),
                Some(driver) => {
                    let ctx = ctx.push_input_ref( assign );
                    let state = tokio::time::timeout(
                        Duration::from_secs(self.skel.machine.timeouts.high),
                        driver.assign(ctx).await,
                    )
                    .await??;
                   Ok(ctx.wave().core.ok())
                }
            }
        } else {
            Err(MsgErr::bad_request())
        }
    }

     */

    async fn start_outer_traversal(&self, traversal: Traversal<UltraWave>) {
        let traverse_to_next_tx = self.skel.traverse_to_next_tx.clone();
        tokio::spawn(async move {
            traverse_to_next_tx.send(traversal).await;
        });
    }

    async fn start_inner_traversal(&self, traversal: Traversal<UltraWave>) {}

    pub async fn visit(&self, traversal: Traversal<UltraWave>) {
        if traversal.dir.is_core() {
            match self.drivers.get(&traversal.record.details.stub.kind) {
                None => {
                    traversal.logger.warn(format!(
                        "star does not have a driver for Kind <{}>",
                        traversal.record.details.stub.kind.to_string()
                    ));
                }
                Some(driver) => {
                    let driver = driver.clone();
                    tokio::spawn(async move {
                        driver.traversal(traversal).await;
                    });
                }
            }
        } else {
            self.start_outer_traversal(traversal).await;
        }
    }
}

#[derive(Clone)]
pub struct DriverApi {
    pub tx: mpsc::Sender<DriverShellCall>,
    pub kind: Kind,
}

impl DriverApi {
    pub fn new(tx: mpsc::Sender<DriverShellCall>, kind: Kind) -> Self {
        Self { tx, kind }
    }

    pub async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.tx.send(DriverShellCall::Assign { assign, rtn }).await;
        Ok(rtn_rx.await??)
    }

    pub async fn status(&self) -> Result<DriverStatus, MsgErr> {
        let (tx, mut rx) = oneshot::channel();
        self.tx.send(DriverShellCall::Status(tx)).await;
        Ok(tokio::time::timeout(Duration::from_secs(60), rx).await??)
    }

    pub async fn lifecycle(&self, call: DriverLifecycleCall) -> Result<DriverStatus, MsgErr> {
        let (tx, mut rx) = oneshot::channel();
        self.tx
            .send(DriverShellCall::LifecycleCall { call, tx })
            .await;

        tokio::time::timeout(Duration::from_secs(5 * 60), rx).await??
    }

    pub async fn traversal(&self, traversal: Traversal<UltraWave>) {
        self.tx.send(DriverShellCall::Traversal(traversal)).await;
    }

    pub async fn signal_boot_phase(&self, phase: BootPhase ) {
        self.tx.send(DriverShellCall::BootPhase(phase)).await;
    }

    pub async fn handle(&self, wave: DirectedWave) -> Result<ReflectedCore, MsgErr> {
        let (tx, mut rx) = oneshot::channel();
        self.tx.send(DriverShellCall::Handle { wave, tx }).await;
        tokio::time::timeout(Duration::from_secs(30), rx).await??
    }
}
/*
fn create_driver<P>(
    factory: Box<dyn DriverFactory<P>>,
    drivers_port: Port,
    skel: StarSkel<P>,
) -> Result<DriverApi, MsgErr>
where
    P: Platform + 'static,
{
    let point = drivers_port
        .point
        .push(factory.kind().as_point_segments())?;
    let (shell_tx, shell_rx) = mpsc::channel(1024);
    let (tx, mut rx) = mpsc::channel(1024);
    {
        let shell_tx = shell_tx.clone();
        tokio::spawn(async move {
            while let Some(call) = rx.recv().await {
                match call {
                    DriverShellRequest::Ex { point, tx } => {
                        let call = DriverShellCall::Item { point, tx };
                        shell_tx.send(call).await;
                    }
                    DriverShellRequest::Assign { assign, rtn } => {
                        let call = DriverShellCall::Assign { assign, rtn };
                        shell_tx.send(call).await;
                    }
                }
            }
        });
    }
    let router = Arc::new(LayerInjectionRouter::new(
        skel.clone(),
        point.clone().to_port().with_layer(Layer::Guest),
    ));
    let (driver_skel,status_tx,status_ctx_tx) = DriverSkel::new(point.clone(), router, tx, skel.clone());
    let driver = factory.create(driver_skel, status_tx);
    let state = skel.state.api().with_layer(Layer::Core);
    let shell = DriverShell::new(point, skel.clone(), driver, state, shell_tx, shell_rx);
    let api = DriverApi::new(shell, factory.kind());
    Ok(api)
}

 */

pub enum DriverShellCall {
    LifecycleCall {
        call: DriverLifecycleCall,
        tx: oneshot::Sender<Result<DriverStatus, MsgErr>>,
    },
    Status(oneshot::Sender<DriverStatus>),
    Traversal(Traversal<UltraWave>),
    BootPhase(BootPhase),
    Handle {
        wave: DirectedWave,
        tx: oneshot::Sender<Result<ReflectedCore, MsgErr>>,
    },
    Item {
        point: Point,
        tx: oneshot::Sender<Result<Box<dyn Item>, MsgErr>>,
    },
    Assign {
        assign: Assign,
        rtn: oneshot::Sender<Result<(), MsgErr>>,
    },
}

pub struct ItemShell<P>
where
    P: Platform + 'static,
{
    pub port: Port,
    pub skel: StarSkel<P>,
    pub state: Option<Arc<RwLock<dyn State>>>,
    pub ex: Box<dyn Item>,
    pub router: Arc<dyn Router>,
}

#[async_trait]
impl<P> TraversalLayer for ItemShell<P>
where
    P: Platform,
{
    fn port(&self) -> &cosmic_api::id::id::Port {
        &self.port
    }

    async fn deliver_directed(&self, direct: Traversal<DirectedWave>) {
        self.skel
            .logger
            .track(&direct, || Tracker::new("core:outer", "DeliverDirected"));
        let logger = self
            .skel
            .logger
            .point(self.port().clone().to_point())
            .span();
        let mut transmitter =
            ProtoTransmitterBuilder::new(self.router.clone(), self.skel.exchanger.clone());
        transmitter.from = SetStrategy::Override(self.port.clone());
        let transmitter = transmitter.build();
        let to = direct.to().clone().unwrap_single();
        let reflection = direct.reflection();
        let ctx = RootInCtx::new(direct.payload, to, logger, transmitter);
        match self.ex.handle(ctx).await {
            CoreBounce::Absorbed => {}
            CoreBounce::Reflected(reflected) => {
                let wave = reflection.unwrap().make(reflected, self.port.clone());
                let wave = wave.to_ultra();
                #[cfg(test)]
                self.skel
                    .diagnostic_interceptors
                    .reflected_endpoint
                    .send(wave.clone());
                self.inject(wave).await;
            }
        }
    }

    async fn deliver_reflected(&self, reflect: Traversal<ReflectedWave>) {
        self.exchanger().reflected(reflect.payload).await;
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        self.skel.traverse_to_next_tx.send(traversal).await;
    }

    async fn inject(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.port().clone().with_layer(Layer::Guest), wave);
        self.skel.inject_tx.send(inject).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }
}

#[derive(DirectedHandler)]
pub struct DriverRunner<P>
where
    P: Platform + 'static,
{
    point: Point,
    skel: StarSkel<P>,
    status: DriverStatus,
    call_tx: mpsc::Sender<DriverShellCall>,
    call_rx: mpsc::Receiver<DriverShellCall>,
    driver: Box<dyn Driver<P>>,
    router: LayerInjectionRouter<P>,
    logger: PointLogger,
    status_rx: watch::Receiver<DriverStatus>,
}

#[routes]
impl<P> DriverRunner<P>
where
    P: Platform + 'static,
{
    pub fn new(
        point: Point,
        skel: StarSkel<P>,
        driver: Box<dyn Driver<P>>,
        call_tx: mpsc::Sender<DriverShellCall>,
        call_rx: mpsc::Receiver<DriverShellCall>,
        status_rx: watch::Receiver<DriverStatus>
    ) -> mpsc::Sender<DriverShellCall> {
        let logger = skel.logger.point(point.clone());
        let router = LayerInjectionRouter::new(
            skel.clone(),
            point.clone().to_port().with_layer(Layer::Guest),
        );

        let driver = Self {
            point,
            skel,
            status: DriverStatus::Pending,
            call_tx: call_tx.clone(),
            call_rx: call_rx,
            driver,
            router,
            logger,
            status_rx
        };

        driver.start();

        call_tx
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.call_rx.recv().await {
                match call {
                    DriverShellCall::LifecycleCall { call, tx } => {
                        let result = self.lifecycle(call).await;
                        match result {
                            Ok(status) => {
                                self.status = status.clone();
                                tx.send(Ok(status));
                            }
                            Err(err) => {
                                self.status = DriverStatus::Unknown;
                                tx.send(Err(err));
                            }
                        }
                    }
                    DriverShellCall::Status(tx) => {
                        tx.send(self.status.clone());
                    }
                    DriverShellCall::Traversal(traversal) => {
                        self.traverse(traversal).await;
                    }
                    DriverShellCall::Handle { wave, tx } => {
                        self.logger
                            .track(&wave, || Tracker::new("driver:shell", "Handle"));
                        let port = wave.to().clone().unwrap_single();
                        let logger = self.skel.logger.point(port.clone().to_point()).span();
                        let router = Arc::new(self.router.clone());
                        let transmitter =
                            ProtoTransmitter::new(router, self.skel.exchanger.clone());
                        let ctx = RootInCtx::new(wave, port.clone(), logger, transmitter);
                        match self.handle(ctx).await {
                            CoreBounce::Absorbed => {
                                tx.send(Err(MsgErr::server_error()));
                            }
                            CoreBounce::Reflected(reflect) => {
                                tx.send(Ok(reflect));
                            }
                        }
                    }
                    DriverShellCall::Item { point, tx } => {
                        tx.send(self.driver.item(&point).await);
                    }
                    DriverShellCall::Assign { assign, rtn } => {
                        rtn.send(self.driver.assign(assign).await);
                    }
                    DriverShellCall::BootPhase(phase) => {
                        self.skel.logger.result(self.boot_phase(phase).await);
                    }
                }
            }
        });
    }

    async fn boot_phase( &self, phase: BootPhase ) -> Result<(),MsgErr>{
        if phase == BootPhase::Cross {
            let kind = self.driver.kind();
            let details = Details {
                stub: Stub {
                    point: point.clone(),
                    kind: self.driver.kind(),
                    status: Status::Pending,
                },
                properties: Default::default(),
            };

            let assign = Assign::new(AssignmentKind::Create, details, StateSrc::None);
            let mut ping = DirectedProto::ping();
            ping.kind(DirectedKind::Ping);
            ping.method(SysMethod::Assign);
            ping.body(Substance::Sys(Sys::Assign(assign)))?;
            ping.to(self.skel.point.clone().to_port());
            ping.from(self.skel.point.clone().to_port());
            ping.track = true;

            self.logger.track(&ping, || Tracker::new("init:create", "SendToStarAssign"));

            let pong: Wave<Pong> = log(self.skel.gravity_transmitter.direct(ping).await)?;

            if !pong.core.status.is_success() {
                println!("Status code: {}", pong.core.status.to_string());
                return Err(MsgErr::from_500(format!(
                    "failed to assign driver: {}",
                    kind.to_string()
                ))
                    .into());
            }
        }
        Ok(())
    }

    async fn traverse(&self, traversal: Traversal<UltraWave>) -> Result<(), MsgErr> {
        let core = self.core(&traversal.to.point).await?;
        if traversal.is_directed() {
            core.deliver_directed(traversal.unwrap_directed()).await;
        } else {
            core.deliver_reflected(traversal.unwrap_reflected()).await;
        }
        Ok(())
    }

    async fn lifecycle(&mut self, call: DriverLifecycleCall) -> Result<DriverStatus, MsgErr> {
        self.driver.lifecycle(call).await
    }

    async fn core(&self, point: &Point) -> Result<ItemShell<P>, MsgErr> {
        let port = point.clone().to_port().with_layer(Layer::Core);
        let (tx, mut rx) = oneshot::channel();
        self.skel
            .state
            .states_tx()
            .send(StateCall::Get {
                port: port.clone(),
                tx,
            })
            .await;
        let state = rx.await??;
        Ok(ItemShell {
            port: port.clone(),
            skel: self.skel.clone(),
            state: state.clone(),
            ex: self.driver.item(point).await?,
            router: Arc::new(self.router.clone().with(port)),
        })
    }

    #[route("Sys<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, MsgErr> {
        match ctx.input {
            Sys::Assign(assign) => {
                self.driver.assign(assign.clone()).await?;

                Ok(ReflectedCore::ok_body(Substance::Empty))
            }
            _ => Err(MsgErr::bad_request()),
        }
    }

    fn status(&self) -> &DriverStatus {
        &self.status
    }
}

#[derive(Clone)]
pub struct DriverSkel<P>
where
    P: Platform,
{
    pub point: Point,
    pub router: Arc<dyn Router>,
    pub shell_tx: mpsc::Sender<DriverShellRequest>,
    pub star_skel: StarSkel<P>,
    pub logger: PointLogger,
}

impl<P> DriverSkel<P>
where
    P: Platform,
{
    pub fn status(&self) -> DriverStatus {
        self.driver_status_rx.borrow().clone()
    }

    pub fn ctx_status(&self) -> DriverConStatus {
        self.driver_ctx_status_rx.borrow().clone()
    }
    pub async fn item(&self, point: Point) -> Result<Box<dyn Item>, MsgErr> {
        let (tx, rx) = oneshot::channel();
        self.shell_tx
            .send(DriverShellRequest::Ex { point, tx })
            .await;
        Ok(rx.await??)
    }

    pub fn new(
        point: Point,
        router: Arc<dyn Router>,
        shell_tx: mpsc::Sender<DriverShellRequest>,
        star_skel: StarSkel<P>,
        logger: PointLogger,
    ) -> Self {
        Self {
            point,
            router,
            shell_tx,
            star_skel,
            logger,
        }
    }
}

pub trait DriverFactory<P>: Send + Sync
where
    P: Platform,
{
    type Driver;
    fn kind(&self) -> Kind;
    async fn create(
        &self,
        skel: DriverSkel<P>,
        status_tx: watch::Sender<DriverStatus>,
    ) -> Result<Box<dyn Driver<P>>, MsgErr>;
}

#[async_trait]
pub trait Driver<P>: DirectedHandler + Send + Sync
where
    P: Platform,
    Self::Item: Item,
{
    type Item;
    type Ctx;
    type State;

    fn kind(&self) -> Kind;

    async fn init(skel: DriverSkel<P>, status_tx: watch::Sender<DriverStatus>) -> Box<dyn Self>;

    async fn item(&self, point: &Point) -> Result<Box<dyn Item>, MsgErr>;
    async fn assign(&self, assign: Assign) -> Result<(), MsgErr>;
}

pub trait States: Sync + Sync
where
    Self::ItemState: ItemState,
{
    type ItemState;
    fn new() -> Self;

    fn create(assign: Assign) -> Arc<RwLock<Self::ItemState>>;
    fn get(point: &Point) -> Option<&Arc<RwLock<Self::ItemState>>>;
    fn remove(point: &Point) -> Option<Arc<RwLock<Self::ItemState>>>;
}

pub trait DriverCons<P>
where
    P: Platform,
{
    fn new(skel: DriverSkel<P>, ctx_status_tx: watch::Sender<DriverConStatus>) -> Self;
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum DriverLifecycleCall {
    Init,
    Shutdown,
}

#[derive(Clone, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum DriverStatus {
    Unknown,
    Pending,
    Initializing,
    Ready,
    Retry(String),
    Fatal(String),
}

#[derive(Clone, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum DriverConStatus {
    Pending,
    Initializing,
    Ready,
    Retry(String),
    Fatal(String),
}

impl<E> From<Result<DriverStatus, E>> for DriverStatus {
    fn from(result: Result<DriverStatus, E>) -> Self {
        match result {
            Ok(status) => status,
            Err(_) => DriverStatus::Panic,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct DriverStatusEvent {
    pub driver: Point,
    pub status: DriverStatus,
}

pub trait ItemState: Send + Sync {}

pub trait Item: DirectedHandler + Send + Sync
{
    type State;
    type Ctx;
}

pub enum DriverShellRequest {
    Ex {
        point: Point,
        tx: oneshot::Sender<Result<Box<dyn Item>, MsgErr>>,
    },
    Assign {
        assign: Assign,
        rtn: oneshot::Sender<Result<(), MsgErr>>,
    },
}

#[derive(Clone)]
pub struct CoreSkel<P>
where
    P: Platform,
{
    pub point: Point,
    pub transmitter: ProtoTransmitter,
    phantom: PhantomData<P>,
}

impl<P> CoreSkel<P>
where
    P: Platform,
{
    pub fn new(point: Point, transmitter: ProtoTransmitter) -> Self {
        Self {
            point,
            transmitter,
            phantom: Default::default(),
        }
    }
}

pub struct DriverDriverFactory<P>
where
    P: Platform,
{
    skel: StarSkel<P>,
    drivers_api: DriversApi<P>,
}

impl<P> DriverDriverFactory<P>
where
    P: Platform,
{
    pub fn new(skel: StarSkel<P>, drivers_api: DriversApi<P>) -> Self {
        Self { skel, drivers_api }
    }
}

impl<P> DriverFactory<P> for DriverDriverFactory<P>
where
    P: Platform,
{
    type Driver = DriverDriver<P>;

    fn kind(&self) -> Kind {
        Kind::Driver
    }

    fn create(
        &self,
        skel: DriverSkel<P>,
        status_tx: watch::Sender<DriverStatus>,
    ) -> Box<dyn DriverProxy> {
        Box::new(DriverProxyWrapper::new(DriverDriverInit::new(
            skel,
            self.drivers_api.clone(),
            status_tx,
        )))
    }
}

pub struct DriverDriverInit<P>
where
    P: Platform,
{
    skel: StarSkel<P>,
    drivers_api: DriversApi<P>,
    call_tx: mpsc::Sender<DriverDriverCall>,
}

impl<P> DriverProxy for DriverDriverInit<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Driver
    }

    async fn item(&self, point: &Point) -> Result<Box<dyn Item>, MsgErr> {
        Err(MsgErr::not_found())
    }

    async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        Err(MsgErr::not_found())
    }
}

impl<P> DriverInit<P> for DriverDriverInit<P> where P: Platform {}

#[derive(DirectedHandler)]
pub struct DriverDriver<P>
where
    P: Platform,
{
    skel: DriverSkel<P>,
    drivers_api: DriversApi<P>,
    call_tx: mpsc::Sender<DriverDriverCall>,
    status_tx: mpsc::Sender<DriverStatus>,
}

impl<P> DriverDriver<P>
where
    P: Platform,
{
    pub fn new(
        skel: DriverSkel<P>,
        drivers_api: DriversApi<P>,
        status_tx: watch::Sender<DriverStatus>,
    ) -> Self {
        let call_tx = DriverDriverRunner::new(skel.clone(), drivers_api.clone());
        Self {
            skel,
            drivers_api,
            call_tx,
        }
    }
}

#[routes]
impl<P> DriverDriver<P> where P: Platform {}

#[async_trait]
impl<P> Driver for DriverDriver<P>
where
    P: Platform,
{
    type Item = DriverCore<P>;
    type Ctx = ();
    type State = ();

    fn kind(&self) -> Kind {
        Kind::Driver
    }

    async fn init(
        skel: DriverSkel<P>,
        status_tx: watch::Sender<DriverStatus>,
    ) -> Receiver<Result<Box<dyn DriverProxy>, P::Err>> {
        let (rtn, rtn_rx) = oneshot::channel();
        tokio::spawn(async move {
            async fn create<P>(skel: &DriverSkel<P>, point: Point, kind: Kind) -> Result<(), P::Err>
            where
                P: Platform,
            {
                println!("creating {}", point.to_string());
                let logger = skel
                    .logger
                    .push("drivers")?
                    .push(kind.as_point_segments())?;
                let registration = Registration {
                    point: point.clone(),
                    kind: Kind::Base(BaseSubKind::Drivers),
                    registry: Default::default(),
                    properties: Default::default(),
                    owner: HYPERUSER.clone(),
                };
                skel.registry.register(&registration).await?;
                skel.registry.assign(&point, &skel.point).await?;
                skel.registry
                    .set_status(&point, &Status::Initializing)
                    .await?;

                let details = Details {
                    stub: Stub {
                        point: point.clone(),
                        kind: kind.clone(),
                        status: Status::Unknown,
                    },
                    properties: Default::default(),
                };

                let assign = Assign::new(AssignmentKind::Create, details, StateSrc::None);
                let mut ping = DirectedProto::ping();
                ping.kind(DirectedKind::Ping);
                ping.method(SysMethod::Assign);
                ping.body(Substance::Sys(Sys::Assign(assign)))?;
                ping.to(skel.point.clone().to_port());
                ping.from(skel.star_skel.point.clone().to_port());
                ping.track = true;

                logger.track(&ping, || Tracker::new("init:create", "SendToStarAssign"));

                let pong: Wave<Pong> = log(skel.star_skel.gravity_transmitter.direct(ping).await)?;

                if !pong.core.status.is_success() {
                    println!("Status code: {}", pong.core.status.to_string());
                    return Err(MsgErr::from_500(format!(
                        "failed to assign driver: {}",
                        kind.to_string()
                    ))
                    .into());
                }

                Ok(())
            }

            async fn init<P>(
                skel: &DriverSkel<P>,
                drivers_api: &DriversApi<P>,
            ) -> Result<(), P::Err>
            where
                P: Platform,
            {
                let drivers_point = skel.star_skel.point.push("drivers")?;
                let registration = Registration {
                    point: drivers_point.clone(),
                    kind: Kind::Base(BaseSubKind::Drivers),
                    registry: Default::default(),
                    properties: Default::default(),
                    owner: HYPERUSER.clone(),
                };

                skel.star_skel.registry.register(&registration).await?;

                create(
                    skel,
                    drivers_point.clone(),
                    Kind::Base(BaseSubKind::Drivers),
                )
                .await?;

                for (kind, driver) in drivers_api.drivers().await? {
                    if kind.to_base() != BaseKind::Star && kind.to_base() != BaseKind::Driver {
                        let point = drivers_point.push(kind.as_point_segments())?;
                        create(skel, point, kind).await?;
                    }
                }
                Ok(())
            }

            let result = skel
                .logger
                .result_ctx("init", init(&skel, &drivers_api).await);
            match result {
                Ok(_) => {
                    rtn.send(Ok(Box::new(DriverDriver::new(
                        skel,
                        drivers_api,
                        status_tx,
                    ))))
                    .unwrap_or_default();
                    status_tx.send(DriverStatus::Ready).unwrap_or_default();
                }
                Err(err) => {
                    rtn.send(Err(err)).unwrap_or_default();
                    status_tx
                        .send(DriverStatus::Fatal(format!(
                            "DriverDriverInit: {}",
                            err.to_string()
                        )))
                        .unwrap_or_default();
                }
            }
        });
        rtn_rx
    }

    async fn status(&self) -> DriverStatus {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(DriverDriverCall::Status(rtn)).await;
        rtn_rx.await.into()
    }

    async fn lifecycle(&mut self, event: DriverLifecycleCall) -> Result<DriverStatus, MsgErr> {
        match event {
            DriverLifecycleCall::Init => {
                let (rtn, mut rtn_rx) = oneshot::channel();
                self.call_tx.send(DriverDriverCall::Init(rtn)).await;
                Ok(rtn_rx.await.into())
            }
            DriverLifecycleCall::Shutdown => Ok(DriverStatus::Shutdown),
        }
    }

    async fn item(&self, point: &Point) -> Result<Box<dyn Item>, MsgErr> {
        Ok(Box::new(DriverCore::new(point.clone(), self.skel.clone())))
    }

    async fn assign(&self, assign: Assign) -> Result<(), MsgErr> {
        println!("DriverDriver Assign!");
        Ok(())
    }
}

pub enum DriverDriverCall {
    Init(oneshot::Sender<DriverStatus>),
    Status(oneshot::Sender<DriverStatus>),
}

pub struct DriverDriverRunner<P>
where
    P: Platform,
{
    skel: StarSkel<P>,
    drivers_api: DriversApi<P>,
    call_tx: mpsc::Sender<DriverDriverCall>,
    call_rx: mpsc::Receiver<DriverDriverCall>,
    status: DriverStatus,
    point: Point,
}

impl<P> DriverDriverRunner<P>
where
    P: Platform,
{
    pub fn new(skel: StarSkel<P>, drivers_api: DriversApi<P>) -> mpsc::Sender<DriverDriverCall> {
        let (call_tx, call_rx) = mpsc::channel(1024);

        let point = skel
            .point
            .push("drivers")
            .unwrap()
            .push(Kind::Driver.as_point_segments())
            .unwrap();

        let runner = Self {
            skel,
            drivers_api,
            call_tx: call_tx.clone(),
            call_rx,
            status: DriverStatus::Pending,
            point,
        };

        runner.start();

        call_tx
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.call_rx.recv().await {
                match call {
                    DriverDriverCall::Init(rtn) => {
                        rtn.send(DriverStatus::Ready);
                        P::log_ctx("DriverDriverRunner::init()", self.init().await);
                    }
                    DriverDriverCall::Status(rtn) => {
                        rtn.send(self.status.clone());
                    }
                }
            }
        });
    }
}

#[derive(DirectedHandler)]
pub struct DriverCore<P>
where
    P: Platform,
{
    point: Point,
    skel: StarSkel<P>,
}

#[routes]
impl<P> DriverCore<P>
where
    P: Platform,
{
    pub fn new(point: Point, skel: StarSkel<P>) -> Self {
        Self { point, skel }
    }
}

impl<P> Item for DriverCore<P> where P: Platform {
    type State = ();
    type Ctx = ();
}
