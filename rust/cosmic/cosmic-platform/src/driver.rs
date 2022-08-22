use crate::machine::MachineSkel;
use crate::star::StarCall::LayerTraversalInjection;
use crate::star::{
    LayerInjectionRouter, StarDriver, StarDriverFactory, StarSkel, StarState, StateApi, StateCall,
};
use crate::{PlatErr, Platform, Registry, RegistryApi};
use cosmic_api::command::command::common::{SetProperties, StateSrc};
use cosmic_api::command::request::create::{Create, PointSegTemplate, Strategy};
use cosmic_api::config::config::bind::{BindConfig, RouteSelector};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{
    BaseKind, Kind, Layer, Point, Port, ToBaseKind, ToPoint, ToPort, TraversalLayer, Uuid,
};
use cosmic_api::id::{BaseSubKind, StarKey, StarSub, Traversal, TraversalInjection};
use cosmic_api::log::{PointLogger, Tracker};
use cosmic_api::parse::model::Subst;
use cosmic_api::parse::{bind_config, route_attribute};
use cosmic_api::particle::particle::{Details, Status, Stub};
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, AssignmentKind, Sys};
use cosmic_api::util::{log, ValuePattern};
use cosmic_api::wave::{Agent, Bounce, CmdMethod, CoreBounce, DirectedCore, DirectedHandler, DirectedHandlerSelector, DirectedKind, DirectedProto, DirectedWave, Exchanger, InCtx, Ping, Pong, ProtoTransmitter, ProtoTransmitterBuilder, RecipientSelector, ReflectedCore, ReflectedWave, RootInCtx, Router, SetStrategy, SysMethod, UltraWave, Wave, WaveKind};
use cosmic_api::{ArtRef, Registration, State, HYPERUSER};
use dashmap::DashMap;
use futures::future::select_all;
use futures::FutureExt;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::Receiver;
use tokio::sync::watch::Ref;
use tokio::sync::{broadcast, mpsc, oneshot, watch, RwLock};

lazy_static! {
    static ref DEFAULT_BIND: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(default_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/default.bind").unwrap()
    );
}

fn default_bind() -> BindConfig {
    log(bind_config(r#" Bind(version=1.0.0) { } "#)).unwrap()
}

pub struct DriversBuilder<P>
where
    P: Platform,
{
    pre: Vec<Arc<dyn HyperDriverFactory<P>>>,
    factories: HashMap<Kind, Arc<dyn HyperDriverFactory<P>>>,
}

impl<P> DriversBuilder<P>
where
    P: Platform,
{
    pub fn new(kind: StarSub) -> Self {
        let mut pre: Vec<Arc<dyn HyperDriverFactory<P>>> = vec![];
        pre.push(Arc::new(DriverDriverFactory::new()));
        pre.push(Arc::new(StarDriverFactory::new(kind)));
        Self {
            pre,
            factories: HashMap::new(),
        }
    }

    pub fn kinds(&self) -> HashSet<Kind> {
        self.factories.keys().cloned().into_iter().collect()
    }

    pub fn add_pre(&mut self, factory: Arc<dyn HyperDriverFactory<P>>) {
        self.pre.push(factory);
    }

    pub fn add(&mut self, factory: Box<dyn DriverFactory<P>>) {
        self.factories
            .insert(factory.kind(), DriverFactoryWrapper::wrap(factory));
    }

    pub fn add_hyper(&mut self, factory: Arc<dyn HyperDriverFactory<P>>) {
        self.factories.insert(factory.kind(), factory);
    }

    pub fn build(
        self,
        skel: StarSkel<P>,
        call_tx: mpsc::Sender<DriversCall<P>>,
        call_rx: mpsc::Receiver<DriversCall<P>>,
        status_tx: watch::Sender<DriverStatus>,
        status_rx: watch::Receiver<DriverStatus>,
    ) -> DriversApi<P> {
        let port = skel.point.push("drivers").unwrap().to_port();
        Drivers::new(
            port,
            skel.clone(),
            self.pre,
            self.factories,
            call_tx,
            call_rx,
            status_tx,
            status_rx,
        )
    }
}

pub enum DriversCall<P>
where
    P: Platform,
{
    Init0,
    Init1,
    AddDriver {
        driver: DriverApi<P>,
        rtn: oneshot::Sender<()>,
    },
    Visit(Traversal<UltraWave>),
    Kinds(oneshot::Sender<Vec<Kind>>),
    Assign {
        assign: Assign,
        rtn: oneshot::Sender<Result<(), P::Err>>,
    },
    Drivers(oneshot::Sender<HashMap<Kind, DriverApi<P>>>),
    Get {
        kind: Kind,
        rtn: oneshot::Sender<Result<DriverApi<P>, MsgErr>>,
    },
    Status {
        kind: Kind,
        rtn: oneshot::Sender<Result<DriverStatus, MsgErr>>,
    },
    StatusRx(oneshot::Sender<watch::Receiver<DriverStatus>>),
}

#[derive(Clone)]
pub struct DriversApi<P>
where
    P: Platform,
{
    call_tx: mpsc::Sender<DriversCall<P>>,
    status_rx: watch::Receiver<DriverStatus>,
}

impl<P> DriversApi<P>
where
    P: Platform,
{
    pub fn new(tx: mpsc::Sender<DriversCall<P>>, status_rx: watch::Receiver<DriverStatus>) -> Self {
        Self {
            call_tx: tx,
            status_rx,
        }
    }

    pub fn status(&self) -> DriverStatus {
        self.status_rx.borrow().clone()
    }

    pub async fn status_changed(&mut self) -> Result<DriverStatus, MsgErr> {
        self.status_rx.changed().await?;
        Ok(self.status())
    }

    pub async fn visit(&self, traversal: Traversal<UltraWave>) {
        self.call_tx.send(DriversCall::Visit(traversal)).await;
    }

    pub async fn kinds(&self) -> Result<Vec<Kind>, MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(DriversCall::Kinds(rtn)).await;
        Ok(rtn_rx.await?)
    }

    pub async fn drivers(&self) -> Result<HashMap<Kind, DriverApi<P>>, MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(DriversCall::Drivers(rtn)).await;
        Ok(rtn_rx.await?)
    }

    pub async fn get(&self, kind: &Kind) -> Result<DriverApi<P>, MsgErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriversCall::Get {
                kind: kind.clone(),
                rtn,
            })
            .await;
        rtn_rx.await?
    }

    pub async fn init(&self) {
        self.call_tx.send(DriversCall::Init0).await;
    }

    pub async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        println!("DriversApi ENTERING ASSIGN");
        let (rtn, rtn_rx) = oneshot::channel();
        self.call_tx.send(DriversCall::Assign { assign, rtn }).await;
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
    pre_factories: Vec<Arc<dyn HyperDriverFactory<P>>>,
    factories: HashMap<Kind, Arc<dyn HyperDriverFactory<P>>>,
    drivers: HashMap<Kind, DriverApi<P>>,
    call_rx: mpsc::Receiver<DriversCall<P>>,
    call_tx: mpsc::Sender<DriversCall<P>>,
    statuses_rx: Arc<DashMap<Kind, watch::Receiver<DriverStatus>>>,
    status_tx: mpsc::Sender<DriverStatus>,
    status_rx: watch::Receiver<DriverStatus>,
    kinds: Vec<Kind>,
    init: bool,
}

impl<P> Drivers<P>
where
    P: Platform + 'static,
{
    pub fn new(
        port: Port,
        skel: StarSkel<P>,
        pre_factories: Vec<Arc<dyn HyperDriverFactory<P>>>,
        factories: HashMap<Kind, Arc<dyn HyperDriverFactory<P>>>,
        call_tx: mpsc::Sender<DriversCall<P>>,
        call_rx: mpsc::Receiver<DriversCall<P>>,
        watch_status_tx: watch::Sender<DriverStatus>,
        watch_status_rx: watch::Receiver<DriverStatus>,
    ) -> DriversApi<P> {
        let statuses_rx = Arc::new(DashMap::new());
        let drivers = HashMap::new();
        let (mpsc_status_tx, mut mpsc_status_rx): (
            tokio::sync::mpsc::Sender<DriverStatus>,
            tokio::sync::mpsc::Receiver<DriverStatus>,
        ) = mpsc::channel(128);

        let mut kinds: Vec<Kind> = factories.keys().cloned().into_iter().collect();
        let mut pres = pre_factories
            .clone()
            .into_iter()
            .map(|f| f.kind())
            .collect();
        kinds.append(&mut pres);
        tokio::spawn(async move {
            while let Some(status) = mpsc_status_rx.recv().await {
                watch_status_tx.send(status.clone());
                if let DriverStatus::Fatal(_) = status {
                    break;
                }
            }
        });

        let mut drivers = Self {
            port,
            skel,
            drivers,
            call_rx,
            call_tx: call_tx.clone(),
            statuses_rx,
            factories,
            pre_factories,
            status_tx: mpsc_status_tx,
            status_rx: watch_status_rx.clone(),
            init: false,
            kinds,
        };

        drivers.start();

        DriversApi::new(call_tx, watch_status_rx)
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.call_rx.recv().await {
                match call {
                    DriversCall::Init0 => {
                        self.init0().await;
                    }
                    DriversCall::Init1 => {
                        self.init1().await;
                    }
                    DriversCall::AddDriver { driver, rtn } => {
                        self.drivers.insert(driver.kind.clone(), driver);
                        rtn.send(());
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
                    DriversCall::Status { kind, rtn } => match self.statuses_rx.get(&kind) {
                        None => {
                            rtn.send(Err(MsgErr::not_found()));
                        }
                        Some(status_rx) => {
                            rtn.send(Ok(status_rx.borrow().clone()));
                        }
                    },
                    DriversCall::StatusRx(rtn) => {
                        rtn.send(self.status_rx.clone());
                    }

                    DriversCall::Get { kind, rtn } => {
                        rtn.send(
                            self.drivers.get(&kind).cloned().ok_or(
                                format!("star does not have driver for kind: {}", kind.to_string())
                                    .into(),
                            ),
                        );
                    }
                }
            }
        });
    }

    pub fn kinds(&self) -> Vec<Kind> {
        self.kinds.clone()
    }
    pub async fn init0(&mut self) {
        if self.pre_factories.is_empty() {
            self.status_listen(None).await;
        } else {
            let factory = self.pre_factories.remove(0);
            let (status_tx, mut status_rx) = watch::channel(DriverStatus::Pending);
            self.statuses_rx.insert(Kind::Driver, status_rx.clone());

            self.create(factory.kind(), factory.clone(), status_tx)
                .await;

            let (rtn, mut rtn_rx) = oneshot::channel();
            self.status_listen(Some(rtn)).await;
            let call_tx = self.call_tx.clone();
            tokio::spawn(async move {
                match rtn_rx.await {
                    Ok(Ok(_)) => {
                        call_tx.send(DriversCall::Init0).await;
                    }
                    _ => {
                        // should be logged by status
                    }
                }
            });
        }
    }

    pub async fn init1(&mut self) {
        let mut statuses_tx = HashMap::new();
        for kind in self.factories.keys() {
            let (status_tx, status_rx) = watch::channel(DriverStatus::Pending);
            statuses_tx.insert(kind.clone(), status_tx);
            self.statuses_rx.insert(kind.clone(), status_rx);
        }

        for (kind, status_tx) in statuses_tx {
            let factory = self.factories.get(&kind).unwrap().clone();
            self.create(kind, factory, status_tx).await;
        }

        self.status_listen(None).await;
    }

    async fn status_listen(&self, on_complete: Option<oneshot::Sender<Result<(), ()>>>) {
        let logger = self.skel.logger.clone();
        let status_tx = self.status_tx.clone();
        let statuses_rx = self.statuses_rx.clone();
        tokio::spawn(async move {
            loop {
                let mut inits = 0;
                let mut fatals = 0;
                let mut retries = 0;
                let mut readies = 0;

                if statuses_rx.is_empty() {
                    break;
                }

                for multi in statuses_rx.iter() {
                    let kind = multi.key();
                    let status_rx = multi.value();
                    match status_rx.borrow().clone() {
                        DriverStatus::Ready => {
                            readies = readies + 1;
                        }
                        DriverStatus::Retrying(msg) => {
                            logger.warn(format!("DRIVER RETRY: {} {}", kind.to_string(), msg));
                            retries = retries + 1;
                        }
                        DriverStatus::Fatal(msg) => {
                            logger.error(format!("DRIVER FATAL: {} {}", kind.to_string(), msg));
                            fatals = fatals + 1;
                        }
                        DriverStatus::Init => {
                            inits = inits + 1;
                        }
                        _ => {}
                    }
                }

                //println!("readies {} ({}) num: {} {}", readies, on_complete.is_some(), statuses_rx.len(), readies == statuses_rx.len());
                if readies == statuses_rx.len() {
                    if on_complete.is_some() {
                        on_complete.unwrap().send(Ok(()));
                        break;
                    } else {
                        status_tx.send(DriverStatus::Ready).await;
                    }
                } else if fatals > 0 {
                    status_tx
                        .send(DriverStatus::Fatal(
                            "One or more Drivers have a Fatal condition".to_string(),
                        ))
                        .await;
                    if on_complete.is_some() {
                        on_complete.unwrap().send(Err(()));
                    }
                    break;
                } else if retries > 0 {
                    status_tx
                        .send(DriverStatus::Fatal(
                            "One or more Drivers is Retrying initialization".to_string(),
                        ))
                        .await;
                } else if inits > 0 {
                    status_tx.send(DriverStatus::Init).await;
                } else {
                    status_tx.send(DriverStatus::Unknown).await;
                }

                for mut multi in statuses_rx.iter_mut() {
                    let status_rx = multi.value_mut();
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

    async fn create(
        &self,
        kind: Kind,
        factory: Arc<dyn HyperDriverFactory<P>>,
        status_tx: watch::Sender<DriverStatus>,
    ) {
        {
            let skel = self.skel.clone();
            let call_tx = self.call_tx.clone();
            let drivers_point = self.skel.point.push("drivers").unwrap();

            async fn register<P>(
                skel: &StarSkel<P>,
                point: &Point,
                logger: &PointLogger,
            ) -> Result<(), P::Err>
            where
                P: Platform,
            {
                let registration = Registration {
                    point: point.clone(),
                    kind: Kind::Driver,
                    registry: Default::default(),
                    properties: Default::default(),
                    owner: HYPERUSER.clone(),
                    strategy: Strategy::Override,
                    status: Status::Init
                };

                skel.registry.register(&registration).await?;
                skel.api.create_states(point.clone()).await?;
                skel.registry.assign(&point).send(skel.point.clone());
                Ok(())
            }
            let point = drivers_point.push(kind.as_point_segments()).unwrap();
            let logger = self.skel.logger.point(point.clone());
            let status_rx = status_tx.subscribe();

            {
                let logger = logger.point(point.clone());
                let kind = kind.clone();
                let mut status_rx = status_rx.clone();
                tokio::spawn(async move {
                    loop {
                        let status = status_rx.borrow().clone();
                        match status {
                            DriverStatus::Unknown => {
                                logger.info(format!("{} {}", kind.to_string(), status.to_string()));
                            }
                            DriverStatus::Pending => {
                                logger.info(format!("{} {}", kind.to_string(), status.to_string()));
                            }
                            DriverStatus::Init => {
                                logger.info(format!("{} {}", kind.to_string(), status.to_string()));
                            }
                            DriverStatus::Ready => {
                                logger.info(format!("{} {}", kind.to_string(), status.to_string()));
                            }
                            DriverStatus::Retrying(ref reason) => {
                                logger.warn(format!(
                                    "{} {}({})",
                                    kind.to_string(),
                                    status.to_string(),
                                    reason
                                ));
                            }
                            DriverStatus::Fatal(ref reason) => {
                                logger.error(format!(
                                    "{} {}({})",
                                    kind.to_string(),
                                    status.to_string(),
                                    reason
                                ));
                            }
                        }
                        match status_rx.changed().await {
                            Ok(_) => {}
                            Err(_) => {
                                break;
                            }
                        }
                    }
                });
            }

            match logger.result(register(&skel, &point, &logger).await) {
                Ok(_) => {}
                Err(err) => {
                    status_tx.send(DriverStatus::Fatal(
                        "Driver registration failed".to_string(),
                    ));
                    return;
                }
            }

            let router = Arc::new(LayerInjectionRouter::new(
                skel.clone(),
                point.clone().to_port().with_layer(Layer::Guest),
            ));
            let mut transmitter = ProtoTransmitterBuilder::new(router, skel.exchanger.clone());
            transmitter.from =
                SetStrategy::Override(point.clone().to_port().with_layer(Layer::Core));
            let transmitter = transmitter.build();

            let (runner_tx, runner_rx) = mpsc::channel(1024);
            let (request_tx, mut request_rx) = mpsc::channel(1024);
            let driver_skel = DriverSkel::new(
                skel,
                kind.clone(),
                point.clone(),
                transmitter,
                logger.clone(),
                status_tx,
                request_tx,
            );

            {
                let runner_tx = runner_tx.clone();
                let logger = logger.clone();
                tokio::spawn(async move {
                    while let Some(request) = request_rx.recv().await {
                        logger
                            .result(
                                runner_tx
                                    .send(DriverRunnerCall::DriverRunnerRequest(request))
                                    .await,
                            )
                            .unwrap_or_default();
                    }
                });
            }

            {
                let skel = self.skel.clone();
                let call_tx = call_tx.clone();
                let logger = logger.clone();
                let router = Arc::new(self.skel.gravity_router.clone());
                let mut transmitter =
                    ProtoTransmitterBuilder::new(router, self.skel.exchanger.clone());
                transmitter.from = SetStrategy::Override(
                    self.skel.point.clone().to_port().with_layer(Layer::Gravity),
                );
                transmitter.agent = SetStrategy::Override(Agent::HyperUser);
                let ctx = DriverCtx::new(transmitter.build());

                tokio::spawn(async move {
                    let driver =
                        logger.result(factory.create(skel.clone(), driver_skel.clone(), ctx).await);
                    match driver {
                        Ok(driver) => {
                            let runner = DriverRunner::new(
                                driver_skel.clone(),
                                skel.clone(),
                                driver,
                                runner_tx,
                                runner_rx,
                                status_rx.clone(),
                            );
                            let driver = DriverApi::new(runner.clone(), factory.kind());
                            let (rtn, rtn_rx) = oneshot::channel();
                            call_tx
                                .send(DriversCall::AddDriver {  driver, rtn })
                                .await
                                .unwrap_or_default();
                            rtn_rx.await;
                            runner.send(DriverRunnerCall::OnAdded).await;
                        }
                        Err(err) => {
                            logger.error(err.to_string());
                            driver_skel
                                .status_tx
                                .send(DriverStatus::Fatal(
                                    "Driver Factory creation error".to_string(),
                                ))
                                .await;
                        }
                    }
                });
            }
        }
    }
}

impl<P> Drivers<P>
where
    P: Platform,
{
    pub async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
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
pub struct DriverApi<P>
where
    P: Platform,
{
    pub call_tx: mpsc::Sender<DriverRunnerCall<P>>,
    pub kind: Kind,
}

impl<P> DriverApi<P>
where
    P: Platform,
{
    pub fn new(tx: mpsc::Sender<DriverRunnerCall<P>>, kind: Kind) -> Self {
        Self { call_tx: tx, kind }
    }

    pub fn on_added(&self) {
        self.call_tx.try_send(DriverRunnerCall::OnAdded);
    }

    pub async fn bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, P::Err> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriverRunnerCall::Bind {
                point: point.clone(),
                rtn,
            })
            .await;
        rtn_rx.await?
    }

    pub async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriverRunnerCall::Assign { assign, rtn })
            .await;
        Ok(rtn_rx.await??)
    }

    pub async fn traversal(&self, traversal: Traversal<UltraWave>) {
        self.call_tx
            .send(DriverRunnerCall::Traversal(traversal))
            .await;
    }

    pub async fn handle(&self, wave: DirectedWave) -> Result<ReflectedCore, MsgErr> {
        let (tx, mut rx) = oneshot::channel();
        self.call_tx
            .send(DriverRunnerCall::Handle { wave, tx })
            .await;
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

pub enum DriverRunnerCall<P>
where
    P: Platform,
{
    Traversal(Traversal<UltraWave>),
    Handle {
        wave: DirectedWave,
        tx: oneshot::Sender<Result<ReflectedCore, MsgErr>>,
    },
    Item {
        point: Point,
        tx: oneshot::Sender<Result<ItemHandler<P>, P::Err>>,
    },
    Assign {
        assign: Assign,
        rtn: oneshot::Sender<Result<(), P::Err>>,
    },
    OnAdded,
    DriverRunnerRequest(DriverRunnerRequest<P>),
    Bind {
        point: Point,
        rtn: oneshot::Sender<Result<ArtRef<BindConfig>, P::Err>>,
    },
}

pub enum DriverRunnerRequest<P>
where
    P: Platform,
{
    Create {
        agent: Agent,
        create: Create,
        rtn: oneshot::Sender<Result<Stub, P::Err>>,
    },
}

pub struct ItemShell<P>
where
    P: Platform + 'static,
{
    pub port: Port,
    pub skel: StarSkel<P>,
    pub item: ItemHandler<P>,
    pub router: Arc<dyn Router>,
}

impl<P> ItemShell<P>
where
    P: Platform + 'static,
{
    pub async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        self.item.bind().await
    }
}

#[async_trait]
impl<P> TraversalLayer for ItemShell<P>
where
    P: Platform,
{
    fn port(&self) -> cosmic_api::id::id::Port {
        self.port.clone()
    }

    async fn deliver_directed(&self, direct: Traversal<DirectedWave>) -> Result<(), MsgErr> {
        self.skel
            .logger
            .track(&direct, || Tracker::new("core:outer", "DeliverDirected"));
        let logger = self
            .skel
            .logger
            .point(self.port().clone().to_point())
            .span();

        match &self.item {
            ItemHandler::Handler(item) => {
                let mut transmitter =
                    ProtoTransmitterBuilder::new(self.router.clone(), self.skel.exchanger.clone());
                transmitter.from = SetStrategy::Override(self.port.clone());
                let transmitter = transmitter.build();
                let to = direct.to().clone().unwrap_single();
                let reflection = direct.reflection();
                let ctx = RootInCtx::new(direct.payload, to, logger, transmitter);

                match item.handle(ctx).await {
                    CoreBounce::Absorbed => {}
                    CoreBounce::Reflected(reflected) => {
                        let reflection = reflection.unwrap();

                        let wave = reflection.make(reflected, self.port.clone());
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
            ItemHandler::Router(router) => {
                let wave = direct.payload.to_ultra();
                router.route(wave).await;
            }
        }

        Ok(())
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
    skel: DriverSkel<P>,
    star_skel: StarSkel<P>,
    call_tx: mpsc::Sender<DriverRunnerCall<P>>,
    call_rx: mpsc::Receiver<DriverRunnerCall<P>>,
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
        skel: DriverSkel<P>,
        star_skel: StarSkel<P>,
        driver: Box<dyn Driver<P>>,
        call_tx: mpsc::Sender<DriverRunnerCall<P>>,
        call_rx: mpsc::Receiver<DriverRunnerCall<P>>,
        status_rx: watch::Receiver<DriverStatus>,
    ) -> mpsc::Sender<DriverRunnerCall<P>> {
        let logger = star_skel.logger.point(skel.point.clone());
        let router = LayerInjectionRouter::new(
            star_skel.clone(),
            skel.point.clone().to_port().with_layer(Layer::Guest),
        );

        let driver = Self {
            skel,
            star_skel: star_skel,
            call_tx: call_tx.clone(),
            call_rx: call_rx,
            driver,
            router,
            logger,
            status_rx,
        };

        driver.start();

        call_tx
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.call_rx.recv().await {
                match call {
                    DriverRunnerCall::OnAdded => {
                        let router = Arc::new(LayerInjectionRouter::new(
                            self.star_skel.clone(),
                            self.skel.point.clone().to_port().with_layer(Layer::Core),
                        ));
                        let transmitter =
                            ProtoTransmitter::new(router, self.star_skel.exchanger.clone());
                        let ctx = DriverCtx::new(transmitter);
                        match self
                            .skel
                            .logger
                            .result(self.driver.init(self.skel.clone(), ctx).await)
                        {
                            Ok(_) => {}
                            Err(err) => {
                                self.skel
                                    .status_tx
                                    .send(DriverStatus::Fatal(err.to_string()))
                                    .await;
                            }
                        }
                    }
                    DriverRunnerCall::Traversal(traversal) => {
                        self.traverse(traversal).await;
                    }
                    DriverRunnerCall::Handle { wave, tx } => {
                        self.logger
                            .track(&wave, || Tracker::new("driver:shell", "Handle"));
                        let port = wave.to().clone().unwrap_single();
                        let logger = self.star_skel.logger.point(port.clone().to_point()).span();
                        let router = Arc::new(self.router.clone());
                        let transmitter =
                            ProtoTransmitter::new(router, self.star_skel.exchanger.clone());
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
                    DriverRunnerCall::Item { point, tx } => {
                        tx.send(self.driver.item(&point).await);
                    }
                    DriverRunnerCall::Assign { assign, rtn } => {
                        rtn.send(self.driver.assign(assign).await);
                    }
                    DriverRunnerCall::DriverRunnerRequest(request) => match request {
                        DriverRunnerRequest::Create { .. } => {}
                    },
                    DriverRunnerCall::Bind { point, rtn } => {
                        let item = self.driver.item(&point).await;
                        match item {
                            Ok(item) => {
                                tokio::spawn(async move {
                                    rtn.send(item.bind().await);
                                });
                            }
                            Err(err) => {
                                rtn.send(Err(err));
                            }
                        }
                    }
                }
            }
        });
    }

    async fn traverse(&self, traversal: Traversal<UltraWave>) -> Result<(), P::Err> {
        let item = self.item(&traversal.to.point).await?;
        let logger = item.skel.logger.clone();
        tokio::spawn(async move {
            if traversal.is_directed() {
                logger.result(item.deliver_directed(traversal.unwrap_directed()).await).unwrap_or_default();
            } else {
                logger.result(item.deliver_reflected(traversal.unwrap_reflected()).await).unwrap_or_default();
            }
        });
        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemShell<P>, P::Err> {
        let port = point.clone().to_port().with_layer(Layer::Core);

        Ok(ItemShell {
            port: port.clone(),
            skel: self.star_skel.clone(),
            item: self.driver.item(point).await?,
            router: Arc::new(self.router.clone().with(port)),
        })
    }


    #[route("Sys<Assign>")]
    async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, P::Err> {
        match ctx.input {
            Sys::Assign(assign) => {
                self.driver.assign(assign.clone()).await?;

                Ok(ReflectedCore::ok_body(Substance::Empty))
            }
            _ => Err(MsgErr::bad_request().into()),
        }
    }
}

pub struct DriverCtx {
    pub transmitter: ProtoTransmitter,
}

impl DriverCtx {
    pub fn new(transmitter: ProtoTransmitter) -> Self {
        Self { transmitter }
    }
}

#[derive(Clone)]
pub struct DriverSkel<P>
where
    P: Platform,
{
    skel: StarSkel<P>,
    pub kind: Kind,
    pub point: Point,
    pub logger: PointLogger,
    pub status_rx: watch::Receiver<DriverStatus>,
    pub status_tx: mpsc::Sender<DriverStatus>,
    pub request_tx: mpsc::Sender<DriverRunnerRequest<P>>,
    pub phantom: PhantomData<P>,
}

impl<P> DriverSkel<P>
where
    P: Platform,
{
    pub fn status(&self) -> DriverStatus {
        self.status_rx.borrow().clone()
    }

    pub fn new(
        skel: StarSkel<P>,
        kind: Kind,
        point: Point,
        transmitter: ProtoTransmitter,
        logger: PointLogger,
        status_tx: watch::Sender<DriverStatus>,
        request_tx: mpsc::Sender<DriverRunnerRequest<P>>,
    ) -> Self {
        let (mpsc_status_tx, mut mpsc_status_rx): (
            tokio::sync::mpsc::Sender<DriverStatus>,
            tokio::sync::mpsc::Receiver<DriverStatus>,
        ) = mpsc::channel(128);

        let watch_status_rx = status_tx.subscribe();
        tokio::spawn(async move {
            while let Some(status) = mpsc_status_rx.recv().await {
                status_tx.send(status.clone());
                if let DriverStatus::Fatal(_) = status {
                    break;
                }
            }
        });

        Self {
            skel,
            kind,
            point,
            logger,
            status_tx: mpsc_status_tx,
            status_rx: watch_status_rx,
            phantom: Default::default(),
            request_tx,
        }
    }

    pub async fn create_driver_particle(&self, point: Point, kind: Kind ) -> Result<(),P::Err> {
        let registration = Registration {
            point: point.clone(),
            kind: kind,
            registry: Default::default(),
            properties: Default::default(),
            owner: self.skel.point.clone(),
            strategy: Strategy::Override,
            status: Status::Ready
        };

        self.skel.registry.register(&registration).await?;
//        self.skel.registry.assign(&point, self.skel.location() ).await?;
        Ok(())
    }

}

pub struct DriverFactoryWrapper<P>
where
    P: Platform,
{
    pub factory: Box<dyn DriverFactory<P>>,
}

impl<P> DriverFactoryWrapper<P>
where
    P: Platform,
{
    pub fn wrap(factory: Box<dyn DriverFactory<P>>) -> Arc<dyn HyperDriverFactory<P>> {
        Arc::new(Self { factory })
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for DriverFactoryWrapper<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        self.factory.kind()
    }

    async fn create(
        &self,
        star_skel: StarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        self.factory.create(driver_skel, ctx).await
    }
}

#[async_trait]
pub trait DriverFactory<P>: Send + Sync
where
    P: Platform,
{
    fn kind(&self) -> Kind;

    async fn create(
        &self,
        skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err>;

    fn properties(&self) -> SetProperties {
        SetProperties::default()
    }
}

#[async_trait]
pub trait HyperDriverFactory<P>: Send + Sync
where
    P: Platform,
{
    fn kind(&self) -> Kind;

    async fn create(
        &self,
        skel: StarSkel<P>,
        driver_skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err>;

    fn properties(&self) -> SetProperties {
        SetProperties::default()
    }
}

#[derive(Clone)]
pub struct HyperSkel<P>
where
    P: Platform,
{
    pub star: StarSkel<P>,
    pub driver: DriverSkel<P>,
}

impl<P> HyperSkel<P>
where
    P: Platform,
{
    pub fn new(star: StarSkel<P>, driver: DriverSkel<P>) -> Self {
        Self { star, driver }
    }
}

#[async_trait]
pub trait Driver<P>: DirectedHandler + Send + Sync
where
    P: Platform,
{
    fn kind(&self) -> Kind;

    fn layer(&self) -> Layer {
        Layer::Core
    }

    async fn init(&mut self, skel: DriverSkel<P>, ctx: DriverCtx) -> Result<(), P::Err> {
        skel.logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();
        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemHandler<P>, P::Err>;
    async fn assign(&self, assign: Assign) -> Result<(), P::Err> {
        Ok(())
    }
    fn default_bind(&self) -> ArtRef<BindConfig> {
        DEFAULT_BIND.clone()
    }
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

#[derive(Clone, Eq, PartialEq, Hash, strum_macros::Display)]
pub enum DriverStatus {
    Unknown,
    Pending,
    Init,
    Ready,
    Retrying(String),
    Fatal(String),
}

impl<E> From<Result<DriverStatus, E>> for DriverStatus
where
    E: ToString,
{
    fn from(result: Result<DriverStatus, E>) -> Self {
        match result {
            Ok(status) => status,
            Err(e) => DriverStatus::Fatal(e.to_string()),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct DriverStatusEvent {
    pub driver: Point,
    pub status: DriverStatus,
}

pub trait ItemState: Send + Sync {}

pub enum ItemHandler<P>
where
    P: Platform,
{
    Handler(Box<dyn ItemDirectedHandler<P>>),
    Router(Box<dyn ItemRouter<P>>),
}

impl<P> ItemHandler<P>
where
    P: Platform,
{
    pub async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        match self {
            ItemHandler::Handler(handler) => handler.bind().await,
            ItemHandler::Router(router) => router.bind().await,
        }
    }
}

#[async_trait]
pub trait Item<P> where P: Platform{
    type Skel;
    type Ctx;
    type State;

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self;

    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(DEFAULT_BIND.clone())
    }
}

#[async_trait]
pub trait ItemDirectedHandler<P>: DirectedHandler + Send + Sync
where
    P: Platform,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err>;
}

#[async_trait]
pub trait ItemRouter<P>: Router + Send + Sync
where
    P: Platform,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err>;
}

#[derive(Clone)]
pub struct ItemSkel<P>
where
    P: Platform,
{
    pub point: Point,
    pub transmitter: ProtoTransmitter,
    phantom: PhantomData<P>,
}

impl<P> ItemSkel<P>
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

pub struct DriverDriverFactory {}

impl DriverDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for DriverDriverFactory
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Driver
    }

    async fn create(
        &self,
        star: StarSkel<P>,
        driver: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(DriverDriver::new(driver).await?))
    }
}

#[derive(DirectedHandler)]
pub struct DriverDriver<P>
where
    P: Platform,
{
    skel: DriverSkel<P>,
}

#[routes]
impl<P> DriverDriver<P>
where
    P: Platform,
{
    async fn new(skel: DriverSkel<P>) -> Result<Self, P::Err> {
        Ok(Self { skel })
    }
}

#[async_trait]
impl<P> Driver<P> for DriverDriver<P>
where
    P: Platform,
{
    fn kind(&self) -> Kind {
        Kind::Driver
    }

    async fn item(&self, point: &Point) -> Result<ItemHandler<P>, P::Err> {
        todo!()
    }
}

#[derive(DirectedHandler)]
pub struct DriverCore<P>
where
    P: Platform,
{
    skel: ItemSkel<P>,
}

#[routes]
impl<P> DriverCore<P>
where
    P: Platform,
{
    pub fn new(skel: ItemSkel<P>) -> Self {
        Self { skel }
    }
}

impl<P> Item<P> for DriverCore<P>
where
    P: Platform,
{
    type Skel = ItemSkel<P>;
    type Ctx = ();
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self {
        Self { skel }
    }
}
