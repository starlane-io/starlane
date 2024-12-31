//pub mod artifact;
//pub mod artifact;
pub mod base;
//pub mod cli;
pub mod control;
pub mod root;
pub mod space;
pub mod star;

pub mod artifact;

pub mod filestore;

use crate::driver::control::ControlErr;
use crate::driver::star::StarDriverFactory;
use crate::executor::dialect::filestore::FileStoreErr;
use crate::machine::MachineErr;
use crate::platform::Platform;
use crate::reg::{Registration, Registry};
use crate::registry::err::RegErr;
use crate::service::{
    Service, ServiceErr, ServiceKind, ServiceRunner, ServiceSelector, ServiceTemplate,
};
use crate::star::{HyperStarSkel, LayerInjectionRouter, StarErr};
use starlane_space::artifact::asynch::{ArtErr, Artifacts};
use starlane_space::artifact::ArtRef;
use starlane_space::command::common::StateSrc::Subst;
use starlane_space::command::common::{SetProperties, StateSrc};
use starlane_space::command::direct::create::{
    Create, KindTemplate, PointSegTemplate, PointTemplate, Strategy, Template,
};
use starlane_space::config::bind::BindConfig;
use starlane_space::err::{any_result, CoreReflector, SpaceErr, SpatialError};
use starlane_space::hyper::{Assign, HyperSubstance, ParticleRecord};
use starlane_space::kind::{BaseKind, Kind, StarSub};
use starlane_space::loc::{Layer, Surface, ToBaseKind, ToPoint, ToSurface};
use starlane_space::log::{Logger, Tracker};
use starlane_space::parse::bind_config;
use starlane_space::parse::util::{parse_errs, result};
use starlane_space::particle::traversal::{
    Traversal, TraversalDirection, TraversalInjection, TraversalLayer,
};
use starlane_space::particle::{Details, Status, Stub};
use starlane_space::point::Point;
use starlane_space::selector::KindSelector;
use starlane_space::substance::{Substance, SubstanceErr};
use starlane_space::util::{log, IdSelector, ValueMatcher};
use starlane_space::wave::core::cmd::CmdMethod;
use starlane_space::wave::core::http2::StatusCode;
use starlane_space::wave::core::{CoreBounce, Method, ReflectedCore};
use starlane_space::wave::exchange::asynch::{
    DirectedHandler, Exchanger, InCtx, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx,
    Router, TraversalRouter,
};
use starlane_space::wave::exchange::SetStrategy;
use starlane_space::wave::{Agent, DirectedWave, ReflectedWave, Wave};
use starlane_space::HYPERUSER;
use anyhow::__private::kind::TraitKind;
use anyhow::{anyhow, Error};
use async_trait::async_trait;
use dashmap::DashMap;
use futures::future::select_all;
use futures::task::Spawn;
use futures::{FutureExt, TryFutureExt};
use once_cell::sync::Lazy;
use starlane_macros::{handler, route, DirectedHandler, ToSpaceErr};
use starlane_macros::push_loc;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::sync::{mpsc, oneshot, watch, RwLock};

pub struct DriversBuilder {
    factories: Vec<Arc<dyn HyperDriverFactory>>,
    kinds: Vec<KindSelector>,
    external_kinds: Vec<KindSelector>,
}

impl DriversBuilder {
    pub fn new(kind: StarSub) -> Self {
        let mut pre: Vec<Arc<dyn HyperDriverFactory>> = vec![];
        let mut kinds = vec![];
        let mut external_kinds = vec![];
        let driver_driver_factory = Arc::new(DriverDriverFactory::new());
        let star_factory = Arc::new(StarDriverFactory::new(kind.clone()));
        kinds.push(<DriverDriverFactory as HyperDriverFactory>::selector(
            &driver_driver_factory,
        ));
        if <DriverDriverFactory as HyperDriverFactory>::avail(&driver_driver_factory)
            == DriverAvail::External
        {
            external_kinds.push(<DriverDriverFactory as HyperDriverFactory>::selector(
                &driver_driver_factory,
            ));
        }
        pre.push(driver_driver_factory);
        kinds.push(kind.to_selector());
        pre.push(star_factory);
        Self {
            factories: pre,
            kinds,
            external_kinds,
        }
    }

    pub fn kinds(&self) -> Vec<KindSelector> {
        self.kinds.clone()
    }

    pub fn add_post(&mut self, factory: Arc<dyn HyperDriverFactory>) {
        self.kinds.push(factory.selector());
        if factory.avail() == DriverAvail::External {
            self.external_kinds.push(factory.selector());
        }
        self.factories.push(factory);
    }

    pub fn add_pre(&mut self, factory: Arc<dyn HyperDriverFactory>) {
        self.kinds.insert(0, factory.selector());
        if factory.avail() == DriverAvail::External {
            self.external_kinds.insert(0, factory.selector());
        }
        self.factories.insert(0, factory);
    }

    pub fn build(
        self,
        skel: HyperStarSkel,
        call_tx: mpsc::Sender<DriversCall>,
        call_rx: mpsc::Receiver<DriversCall>,
        status_tx: watch::Sender<DriverStatus>,
        status_rx: watch::Receiver<DriverStatus>,
    ) -> DriversApi {
        let port = skel.point.push("drivers").unwrap().to_surface();
        Drivers::new(
            port,
            skel.clone(),
            self.factories,
            self.kinds,
            self.external_kinds,
            call_tx,
            call_rx,
            status_tx,
            status_rx,
        )
    }
}

pub enum DriversCall {
    Init0,
    Init1,
    AddDriver {
        driver: DriverApi,
        rtn: oneshot::Sender<()>,
    },
    Visit(Traversal<Wave>),
    InternalKinds(oneshot::Sender<Vec<KindSelector>>),
    ExternalKinds(oneshot::Sender<Vec<KindSelector>>),
    Find {
        kind: Kind,
        rtn: oneshot::Sender<Option<DriverApi>>,
    },
    FindExternalKind {
        kind: Kind,
        rtn: oneshot::Sender<Option<DriverApi>>,
    },
    FindInternalKind {
        kind: Kind,
        rtn: oneshot::Sender<Option<DriverApi>>,
    },
    Drivers(oneshot::Sender<HashMap<KindSelector, DriverApi>>),
    Get {
        kind: Kind,
        rtn: oneshot::Sender<Result<DriverApi, SpaceErr>>,
    },
    LocalDriverLookup {
        kind: Kind,
        rtn: oneshot::Sender<Option<Point>>,
    },
    Status {
        kind: KindSelector,
        rtn: oneshot::Sender<Result<DriverStatus, SpaceErr>>,
    },
    StatusRx(oneshot::Sender<watch::Receiver<DriverStatus>>),
    ByPoint {
        point: Point,
        rtn: oneshot::Sender<Option<DriverApi>>,
    },
}

#[derive(Clone)]
pub struct DriversApi {
    call_tx: mpsc::Sender<DriversCall>,
    status_rx: watch::Receiver<DriverStatus>,
}

impl DriversApi {
    pub fn new(tx: mpsc::Sender<DriversCall>, status_rx: watch::Receiver<DriverStatus>) -> Self {
        Self {
            call_tx: tx,
            status_rx,
        }
    }

    pub fn status(&self) -> DriverStatus {
        self.status_rx.borrow().clone()
    }

    pub async fn status_changed(&mut self) -> Result<DriverStatus, SpaceErr> {
        self.status_rx.changed().await?;
        Ok(self.status())
    }

    pub async fn visit(&self, traversal: Traversal<Wave>) {
        self.call_tx.send(DriversCall::Visit(traversal)).await;
    }

    pub async fn internal_kinds(&self) -> Result<Vec<KindSelector>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(DriversCall::InternalKinds(rtn)).await?;
        Ok(rtn_rx.await?)
    }

    pub async fn external_kinds(&self) -> Result<Vec<KindSelector>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(DriversCall::ExternalKinds(rtn)).await?;
        Ok(rtn_rx.await?)
    }

    pub async fn find(&self, kind: Kind) -> Result<Option<DriverApi>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(DriversCall::Find { kind, rtn }).await?;
        Ok(rtn_rx.await?)
    }

    pub async fn find_external(&self, kind: Kind) -> Result<Option<DriverApi>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriversCall::FindExternalKind { kind, rtn })
            .await?;
        Ok(rtn_rx.await?)
    }

    pub async fn find_internal(&self, kind: Kind) -> Result<Option<DriverApi>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriversCall::FindInternalKind { kind, rtn })
            .await?;
        Ok(rtn_rx.await?)
    }

    pub async fn local_driver_lookup(&self, kind: Kind) -> Result<Option<Point>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriversCall::LocalDriverLookup { kind, rtn })
            .await?;
        Ok(rtn_rx.await?)
    }

    pub async fn drivers(&self) -> Result<HashMap<KindSelector, DriverApi>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx.send(DriversCall::Drivers(rtn)).await;
        Ok(rtn_rx.await?)
    }

    pub async fn get(&self, kind: &Kind) -> Result<DriverApi, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriversCall::Get {
                kind: kind.clone(),
                rtn,
            })
            .await;
        rtn_rx.await?
    }

    pub async fn find_by_point(&self, point: &Point) -> Result<Option<DriverApi>, SpaceErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriversCall::ByPoint {
                point: point.clone(),
                rtn,
            })
            .await;
        Ok(rtn_rx.await?)
    }

    pub async fn init(&self) {
        self.call_tx.send(DriversCall::Init0).await;
    }

    /*
    pub async fn route(&self, wave: UltraWave) {
        self.call_tx.send( DriversCall::Route(wave)).await;
    }

     */
}

#[derive(DirectedHandler)]
pub struct Drivers {
    port: Surface,
    skel: HyperStarSkel,
    factories: Vec<Arc<dyn HyperDriverFactory>>,
    kind_to_driver: HashMap<KindSelector, DriverApi>,
    point_to_driver: HashMap<Point, DriverApi>,
    call_rx: mpsc::Receiver<DriversCall>,
    call_tx: mpsc::Sender<DriversCall>,
    statuses_rx: Arc<DashMap<KindSelector, watch::Receiver<DriverStatus>>>,
    status_tx: mpsc::Sender<DriverStatus>,
    status_rx: watch::Receiver<DriverStatus>,
    kinds: Vec<KindSelector>,
    external_kinds: Vec<KindSelector>,
    init: bool,
}

impl Drivers {
    pub fn new(
        port: Surface,
        skel: HyperStarSkel,
        factories: Vec<Arc<dyn HyperDriverFactory>>,
        kinds: Vec<KindSelector>,
        external_kinds: Vec<KindSelector>,
        call_tx: mpsc::Sender<DriversCall>,
        call_rx: mpsc::Receiver<DriversCall>,
        watch_status_tx: watch::Sender<DriverStatus>,
        watch_status_rx: watch::Receiver<DriverStatus>,
    ) -> DriversApi {
        let statuses_rx = Arc::new(DashMap::new());
        let kind_to_driver = HashMap::new();
        let point_to_driver = HashMap::new();
        let (mpsc_status_tx, mut mpsc_status_rx): (
            tokio::sync::mpsc::Sender<DriverStatus>,
            tokio::sync::mpsc::Receiver<DriverStatus>,
        ) = mpsc::channel(128);

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
            kind_to_driver,
            point_to_driver,
            call_rx,
            call_tx: call_tx.clone(),
            statuses_rx,
            factories,
            status_tx: mpsc_status_tx,
            status_rx: watch_status_rx.clone(),
            init: false,
            kinds,
            external_kinds,
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
                        self.point_to_driver
                            .insert(driver.get_point(), driver.clone());
                        self.kind_to_driver
                            .insert(driver.kind.clone(), driver.clone());
                        if !driver.kind.is_match(&Kind::Driver).is_ok() {
                            let driver_driver = self.find(&Kind::Driver).unwrap();
                            driver_driver.add_driver(driver.clone()).await;
                        }
                        rtn.send(());
                    }
                    DriversCall::Visit(traversal) => {
                        self.visit(traversal).await;
                    }
                    DriversCall::InternalKinds(rtn) => {
                        rtn.send(self.internal_kinds());
                    }
                    DriversCall::Drivers(rtn) => {
                        rtn.send(self.kind_to_driver.clone()).unwrap_or_default();
                    }
                    DriversCall::Status { kind, rtn } => match self.statuses_rx.get(&kind) {
                        None => {
                            rtn.send(Err(SpaceErr::not_found("status_rx")));
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
                            self.find(&kind).cloned().ok_or(
                                format!("star does not have driver for kind: {}", kind.to_string())
                                    .into(),
                            ),
                        );
                    }
                    DriversCall::ExternalKinds(rtn) => {
                        rtn.send(self.external_kinds.clone());
                    }
                    DriversCall::FindExternalKind { kind, rtn } => {
                        rtn.send(self.find_external(&kind).cloned());
                    }
                    DriversCall::FindInternalKind { kind, rtn } => {
                        rtn.send(self.find_internal(&kind).cloned());
                    }
                    DriversCall::Find { kind, rtn } => {
                        rtn.send(self.find(&kind).cloned());
                    }
                    DriversCall::LocalDriverLookup { kind, rtn } => {
                        match self.find(&kind) {
                            None => {
                                rtn.send(None);
                            }
                            Some(driver_api) => {
                                rtn.send(Some(driver_api.get_point()));
                            }
                        };
                    } /*DriversCall::Route(wave) => {
                    println!("DriversApi::Route!");
                    match wave.to().to_single() {
                    Ok(to) => {
                    match self.point_to_driver.get(&to.point ) {
                    Some(api) => {
                    api.route(wave).await;
                    }
                    None => {
                    self.skel.logger.error(format!("Drivers does not have a Driver at surface: {}",to.to_string()));
                    }
                    }
                    }
                    Err(err) => {
                    self.skel.logger.error(format!("expecting single recipient: {}",err.to_string()));
                    }
                    }

                    }

                    */
                    DriversCall::ByPoint { point, rtn } => {
                        let driver = self.point_to_driver.get(&point).cloned();
                        rtn.send(driver);
                    }
                }
            }
        });
    }

    pub fn internal_kinds(&self) -> Vec<KindSelector> {
        self.kinds.clone()
    }

    pub fn external_kinds(&self) -> Vec<KindSelector> {
        self.external_kinds.clone()
    }
    pub async fn init0(&mut self) {
        if self.factories.is_empty() {
            self.status_listen(None).await;
        } else {
            let factory = self.factories.remove(0);
            let (status_tx, mut status_rx) = watch::channel(DriverStatus::Pending);
            self.statuses_rx
                .insert(KindSelector::from_base(BaseKind::Driver), status_rx.clone());

            self.create(
                factory.kind(),
                factory.selector(),
                factory.clone(),
                status_tx,
            )
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
        for kind in &self.kinds {
            let (status_tx, status_rx) = watch::channel(DriverStatus::Pending);
            statuses_tx.insert(kind.clone(), status_tx);
            self.statuses_rx.insert(kind.clone(), status_rx);
        }

        for factory in self.factories.clone() {
            let status_tx = statuses_tx.remove(&factory.selector()).unwrap();
            self.create(factory.kind(), factory.selector(), factory, status_tx)
                .await;
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
        selector: KindSelector,
        factory: Arc<dyn HyperDriverFactory>,
        status_tx: watch::Sender<DriverStatus>,
    ) {
        {
            let star = self.skel.clone();
            let call_tx = self.call_tx.clone();
            let drivers_point = self.skel.point.push("drivers").unwrap();

            async fn register(
                skel: &HyperStarSkel,
                point: &Point,
                logger: &Logger,
            ) -> Result<(), DriverErr> {
                let registration = Registration {
                    point: point.clone(),
                    kind: Kind::Driver,
                    registry: Default::default(),
                    properties: Default::default(),
                    owner: HYPERUSER.clone(),
                    strategy: Strategy::Ensure,
                    status: Status::Init,
                };

                skel.registry.register(&registration).await?;
                skel.api.create_states(point.clone()).await?;
                skel.registry.assign_star(&point, &skel.point).await?;
                Ok(())
            }
            let point = drivers_point
                .push(selector.as_point_segments().unwrap())
                .unwrap();
            let status_rx = status_tx.subscribe();

            {
                let logger = push_loc!((self.skel.logger, &point));
                let kind = selector.clone();
                let mut status_rx = status_rx.clone();
                tokio::spawn(async move {
                    loop {
                        let status = status_rx.borrow().clone();
                        match status {
                            DriverStatus::Unknown => {
                                //                                logger.info(format!("{} {}", kind.to_string(), status.to_string()));
                            }
                            DriverStatus::Pending => {
                                //                               logger.info(format!("{} {}", kind.to_string(), status.to_string()));
                            }
                            DriverStatus::Init => {
                                //                                logger.info(format!("{} {}", kind.to_string(), status.to_string()));
                            }
                            DriverStatus::Ready => {
                                //                                logger.info(format!("{} {}", kind.to_string(), status.to_string()));
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

            {
                let logger = push_loc!((self.skel.logger, &point));
                match self
                    .skel
                    .logger
                    .result(register(&star, &point, &logger).await)
                {
                    Ok(_) => {}
                    Err(err) => {
                        status_tx.send(DriverStatus::Fatal(
                            "Driver registration failed".to_string(),
                        ));
                        return;
                    }
                }
            }

            let mut router = LayerInjectionRouter::new(
                star.clone(),
                point.clone().to_surface().with_layer(Layer::Core),
            );

            router.direction = Some(TraversalDirection::Fabric);

            let mut transmitter =
                ProtoTransmitterBuilder::new(Arc::new(router), star.exchanger.clone());
            transmitter.from =
                SetStrategy::Override(point.clone().to_surface().with_layer(Layer::Core));
            let transmitter = transmitter.build();

            let (runner_tx, runner_rx) = mpsc::channel(1024);
            let (request_tx, mut request_rx) = mpsc::channel(1024);

            let logger = push_loc!((self.skel.logger, &point));
            let driver_skel = DriverSkel::new(
                star,
                kind,
                point.clone(),
                selector.clone(),
                transmitter.clone(),
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
                let ctx = DriverCtx::new(transmitter.clone());

                tokio::spawn(async move {
                    let driver =
                        logger.result(factory.create(skel.clone(), driver_skel.clone(), ctx).await);
                    match driver {
                        Ok(driver) => {
                            let layer = driver.layer();
                            let runner = DriverRunner::new(
                                driver_skel.clone(),
                                skel.clone(),
                                driver,
                                runner_tx,
                                runner_rx,
                                status_rx.clone(),
                                layer,
                            );
                            let driver = DriverApi::new(
                                runner.clone(),
                                driver_skel.point.clone(),
                                factory.selector(),
                            );
                            let (rtn, rtn_rx) = oneshot::channel();
                            call_tx
                                .send(DriversCall::AddDriver {
                                    driver: driver.clone(),
                                    rtn,
                                })
                                .await
                                .unwrap_or_default();
                            if driver.kind.is_match(&Kind::Driver).is_ok() {
                                driver.on_added();
                            }
                            rtn_rx.await;
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

    pub fn find(&self, kind: &Kind) -> Option<&DriverApi> {
        for selector in &self.kinds {
            if selector.is_match(kind).is_ok() {
                return self.kind_to_driver.get(&selector);
            }
        }

        for selector in &self.external_kinds {
            if selector.is_match(kind).is_ok() {
                return self.kind_to_driver.get(selector);
            }
        }
        None
    }

    pub fn find_external(&self, kind: &Kind) -> Option<&DriverApi> {
        for selector in &self.external_kinds {
            if selector.is_match(kind).is_ok() {
                return self.kind_to_driver.get(selector);
            }
        }
        None
    }

    pub fn find_internal(&self, kind: &Kind) -> Option<&DriverApi> {
        for selector in &self.kinds {
            if selector.is_match(kind).is_ok() {
                return self.kind_to_driver.get(selector);
            }
        }
        None
    }
}

impl Drivers {
    /*
    pub async fn assign(&self, assign: Assign) -> Result<(), DriverErr> {
        let driver = self.find(&assign.details.stub.kind).ok_or::<UniErr>(
            format!(
                "Kind {} not supported by Drivers for Star: {}",
                assign.details.stub.kind.to_string(),
                self.skel.kind.to_string()
            )
            .into(),
        )?;
        driver.assign(assign).await
    }

     */

    /*
    pub async fn route(&self, wave: UltraWave) -> Result<(),DriverErr>{
        let record = self
            .skel
            .registry
            .record(&wave.to().single_or()?.point)
            .await
            .map_err(|e| e.to_uni_err())?;
        let driver = self
            .find(&record.details.stub.kind)
            .ok_or::<UniErr>("do not handle this kind of driver".into())?;
        driver.route(wave).await;
        Ok(())
    }

     */

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

    async fn start_outer_traversal(&self, traversal: Traversal<Wave>) {
        let traverse_to_next_tx = self.skel.traverse_to_next_tx.clone();
        tokio::spawn(async move {
            traverse_to_next_tx.send(traversal).await;
        });
    }

    async fn start_inner_traversal(&self, traversal: Traversal<Wave>) {}

    pub async fn visit(&self, traversal: Traversal<Wave>) {
        if traversal.dir.is_core() {
            match self.find(&traversal.record.details.stub.kind) {
                None => {
                    traversal.logger.warn(format!(
                        "star does not have a driver for Kind <{}>",
                        traversal.record.details.stub.kind.to_template().to_string()
                    ));
                }
                Some(driver) => {
                    let driver = driver.clone();
                    tokio::spawn(async move {
                        driver.traverse(traversal).await;
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
    pub call_tx: mpsc::Sender<DriverRunnerCall>,
    pub kind: KindSelector,
    pub point: Point,
}

impl DriverApi {
    pub fn new(tx: mpsc::Sender<DriverRunnerCall>, point: Point, kind: KindSelector) -> Self {
        Self {
            call_tx: tx,
            point,
            kind,
        }
    }

    pub fn get_point(&self) -> Point {
        self.point.clone()
    }

    /// This method call will only work for DriverDriver
    pub async fn add_driver(&self, api: DriverApi) {
        self.call_tx.send(DriverRunnerCall::AddDriver(api)).await;
    }

    pub async fn init_item(&self, point: Point) -> Result<Status, DriverErr> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        self.call_tx
            .try_send(DriverRunnerCall::InitParticle { point, rtn });
        rtn_rx.await?
    }

    pub fn on_added(&self) {
        self.call_tx.try_send(DriverRunnerCall::OnAdded);
    }

    pub async fn bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, DriverErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.call_tx
            .send(DriverRunnerCall::ParticleBind {
                point: point.clone(),
                rtn,
            })
            .await?;
        rtn_rx.await?
    }

    pub async fn driver_bind(&self) -> Result<ArtRef<BindConfig>, DriverErr> {
        let (rtn, rtn_rx) = oneshot::channel();
        self.call_tx.send(DriverRunnerCall::DriverBind(rtn)).await?;
        Ok(rtn_rx.await?)
    }

    pub async fn traverse(&self, traversal: Traversal<Wave>) {
        self.call_tx
            .send(DriverRunnerCall::Traverse(traversal))
            .await;
    }

    pub async fn handle(&self, traversal: Traversal<Wave>) {
        self.call_tx.send(DriverRunnerCall::Handle(traversal)).await;
    }

    /*
    pub async fn route(&self, wave: UltraWave)  {
        self.call_tx
            .send(DriverRunnerCall::Route(wave) )
            .await;
    }

    pub fn route_sync(&self, wave: UltraWave)  {
        self.call_tx
            .try_send(DriverRunnerCall::Route(wave) );
    }

     */
}

#[derive(strum_macros::Display)]
pub enum DriverRunnerCall {
    AddDriver(DriverApi),
    GetPoint(oneshot::Sender<Point>),
    Traverse(Traversal<Wave>),
    Handle(Traversal<Wave>),
    Particle {
        point: Point,
        tx: oneshot::Sender<Result<ParticleSphere, DriverErr>>,
    },

    OnAdded,
    InitParticle {
        point: Point,
        rtn: oneshot::Sender<Result<Status, DriverErr>>,
    },
    DriverRunnerRequest(DriverRunnerRequest),
    DriverBind(oneshot::Sender<ArtRef<BindConfig>>),
    ParticleBind {
        point: Point,
        rtn: oneshot::Sender<Result<ArtRef<BindConfig>, DriverErr>>,
    },
}

pub enum DriverRunnerRequest {
    Create {
        agent: Agent,
        create: Create,
        rtn: oneshot::Sender<Result<Stub, DriverErr>>,
    },
}

pub struct ParticleOuter {
    pub surface: Surface,
    pub skel: HyperStarSkel,
    pub particle: ParticleSphere,
    pub router: Arc<dyn Router>,
}

impl ParticleOuter {}

#[async_trait]
impl TraversalLayer for ParticleOuter {
    fn surface(&self) -> Surface {
        self.surface.clone()
    }

    async fn deliver_directed(&self, direct: Traversal<DirectedWave>) -> Result<(), SpaceErr> {
        self.skel
            .logger
            .track(&direct, || Tracker::new("core:outer", "DeliverDirected"));
        let logger = push_loc!((self.skel.logger, &self.surface));

        match &self.particle.inner {
            ParticleSphereInner::Handler(handler) => {
                let mut transmitter =
                    ProtoTransmitterBuilder::new(self.router.clone(), self.skel.exchanger.clone());
                transmitter.from = SetStrategy::Override(self.surface.clone());
                let transmitter = transmitter.build();
                let to = direct.to.clone();
                let reflection = direct.reflection();
                let ctx = RootInCtx::new(direct.payload, to, logger, transmitter);

                match handler.handle(ctx).await {
                    CoreBounce::Absorbed => {}
                    CoreBounce::Reflected(reflected) => {
                        let reflection = reflection?;

                        let wave = reflection.make(reflected, self.surface.clone());
                        let wave = wave.to_wave();
                        #[cfg(test)]
                        self.skel
                            .diagnostic_interceptors
                            .reflected_endpoint
                            .send(wave.clone());
                        self.inject(wave).await;
                    }
                }
            }
            ParticleSphereInner::Router(router) => {
                self.skel
                    .logger
                    .result(router.traverse(direct.wrap()).await)?;
            }
        }

        Ok(())
    }

    async fn deliver_reflected(&self, reflect: Traversal<ReflectedWave>) -> Result<(), SpaceErr> {
        if reflect.to().layer == self.surface.layer {
            self.exchanger().reflected(reflect.payload).await
        } else {
            match &self.particle.inner {
                ParticleSphereInner::Router(router) => {
                    router.traverse(reflect.wrap()).await;
                    Ok(())
                }
                ParticleSphereInner::Handler(_) => {
                    Err("cannot deliver a reflected to an ParticleSphere::Handler::Handler, it must be an ParticleSphere::Router".into())
                }
            }
        }
    }

    async fn traverse_next(&self, traversal: Traversal<Wave>) {
        self.skel.traverse_to_next_tx.send(traversal).await;
    }

    async fn inject(&self, wave: Wave) {
        let inject = TraversalInjection::new(self.surface().clone().with_layer(Layer::Guest), wave);
        self.skel.inject_tx.send(inject).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }
}

#[derive(starlane_macros::DirectedHandler)]
pub struct DriverRunner {
    skel: DriverSkel,
    star_skel: HyperStarSkel,
    call_tx: mpsc::Sender<DriverRunnerCall>,
    call_rx: mpsc::Receiver<DriverRunnerCall>,
    driver: Box<dyn Driver>,
    router: LayerInjectionRouter,
    logger: Logger,
    status_rx: watch::Receiver<DriverStatus>,
    layer: Layer,
}

impl DriverRunner {
    pub fn new(
        skel: DriverSkel,
        star_skel: HyperStarSkel,
        driver: Box<dyn Driver>,
        call_tx: mpsc::Sender<DriverRunnerCall>,
        call_rx: mpsc::Receiver<DriverRunnerCall>,
        status_rx: watch::Receiver<DriverStatus>,
        layer: Layer,
    ) -> mpsc::Sender<DriverRunnerCall> {
        let logger = push_loc!((star_skel.logger, &skel.point));
        let router = LayerInjectionRouter::new(
            star_skel.clone(),
            skel.point.clone().to_surface().with_layer(Layer::Guest),
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
            layer,
        };

        driver.start();

        call_tx
    }
}

impl DriverRunner {
    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.call_rx.recv().await {
                match call {
                    DriverRunnerCall::OnAdded => {
                        let mut router = LayerInjectionRouter::new(
                            self.star_skel.clone(),
                            self.skel.point.clone().to_surface().with_layer(Layer::Core),
                        );
                        router.direction = Some(TraversalDirection::Fabric);

                        let mut transmitter = ProtoTransmitter::new(
                            Arc::new(router),
                            self.star_skel.exchanger.clone(),
                        );
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
                    DriverRunnerCall::Traverse(traversal) => {
                        self.traverse(traversal).await.unwrap();
                    }
                    DriverRunnerCall::Handle(traversal) => {
                        if traversal.is_directed() {
                            let wave = traversal.payload.to_directed().unwrap();
                            self.logger
                                .track(&wave, || Tracker::new("driver:shell", "Route"));
                            let reflection = wave.reflection();
                            let port = wave.to().clone().unwrap_single();
                            let logger = push_loc!((self.star_skel.logger, &port));
                            let router = Arc::new(self.router.clone());
                            let transmitter =
                                ProtoTransmitter::new(router, self.star_skel.exchanger.clone());
                            let ctx =
                                RootInCtx::new(wave, port.clone(), logger, transmitter.clone());
                            let handler = self.handler().await;
                            let skel = self.skel.clone();
                            tokio::spawn(async move {
                                match handler.handle(ctx).await {
                                    CoreBounce::Absorbed => {
                                        // do nothing
                                    }
                                    CoreBounce::Reflected(core) => {
                                        let reflection = reflection.unwrap();
                                        let reflect = reflection.make(
                                            core,
                                            skel.point.to_surface().with_layer(Layer::Core),
                                        );
                                        transmitter.route(reflect.to_wave()).await;
                                    }
                                }
                            });
                        } else {
                            let wave = traversal.payload.to_reflected().unwrap();
                            self.star_skel.exchanger.reflected(wave).await.unwrap();
                        }
                    }
                    DriverRunnerCall::Particle { point, tx } => {
                        let result = self.logger.result(self.driver.particle(&point).await);
                        tx.send(result);
                    }
                    DriverRunnerCall::DriverRunnerRequest(request) => match request {
                        DriverRunnerRequest::Create { .. } => {
                            todo!()
                        }
                    },
                    DriverRunnerCall::ParticleBind { point, rtn } => {
                        let bind = self
                            .skel
                            .artifacts()
                            .get_bind(&self.driver.kind().to_base().bind())
                            .await
                            .map_err(|e| e.into());
                        rtn.send(bind);
                    }
                    DriverRunnerCall::InitParticle { point, rtn } => {
                        let particle = self.driver.particle(&point).await.unwrap();
                        rtn.send(particle.init().await);
                    }
                    DriverRunnerCall::GetPoint(rtn) => {
                        rtn.send(self.skel.point.clone());
                    }
                    DriverRunnerCall::DriverBind(rtn) => {
                        let bind = self
                            .skel
                            .artifacts()
                            .get_bind(&BaseKind::Driver.bind())
                            .await
                            .unwrap();
                        rtn.send(bind);
                    }
                    DriverRunnerCall::AddDriver(api) => {
                        self.driver.add_driver(api.clone()).await;
                        api.on_added();
                    }
                }
            }
        });
    }

    async fn traverse(&self, traversal: Traversal<Wave>) -> Result<(), DriverErr> {
        self.skel.logger.track(&traversal, || {
            Tracker::new(
                format!("drivers -> {}", traversal.dir.to_string()),
                "Traverse",
            )
        });

        let outer_particle = self
            .skel
            .logger
            .result(self.particle(&traversal.to.point).await)?;
        let logger = outer_particle.skel.logger.clone();
        tokio::spawn(async move {
            if traversal.is_directed() {
                logger
                    .result(
                        outer_particle
                            .deliver_directed(traversal.unwrap_directed())
                            .await,
                    )
                    .unwrap_or_default();
            } else {
                logger
                    .result(
                        outer_particle
                            .deliver_reflected(traversal.unwrap_reflected())
                            .await,
                    )
                    .unwrap_or_default();
            }
        });
        Ok(())
    }

    async fn particle(&self, point: &Point) -> Result<ParticleOuter, DriverErr> {
        let port = point.clone().to_surface().with_layer(self.layer.clone());

        Ok(ParticleOuter {
            surface: port.clone(),
            skel: self.star_skel.clone(),
            particle: self.skel.logger.result(self.driver.particle(point).await)?,
            router: Arc::new(self.router.clone().with(port)),
        })
    }

    async fn handler(&self) -> Box<dyn DriverHandler> {
        self.driver.handler().await
    }
}

#[derive(Clone)]
pub struct DriverCtx {
    pub transmitter: ProtoTransmitter,
}

impl DriverCtx {
    pub fn new(transmitter: ProtoTransmitter) -> Self {
        Self { transmitter }
    }
}

#[derive(Clone)]
pub struct DriverSkel {
    star: HyperStarSkel,
    pub selector: KindSelector,
    pub kind: Kind,
    pub point: Point,
    pub logger: Logger,
    pub status_rx: watch::Receiver<DriverStatus>,
    pub status_tx: mpsc::Sender<DriverStatus>,
    pub request_tx: mpsc::Sender<DriverRunnerRequest>,
}

impl DriverSkel {
    pub async fn select_service(
        &self,
        kind: ServiceKind,
    ) -> Result<Service<ServiceRunner>, ServiceErr> {
        let selector = ServiceSelector {
            name: IdSelector::Always,
            kind,
            driver: Some(self.kind.clone()),
        };
        let service = self.star.machine_api.select_service(selector).await?;
        let service = service.into();
        Ok(service)
    }

    pub fn data_dir(&self) -> String {
        self.star.data_dir()
    }

    pub fn status(&self) -> DriverStatus {
        self.status_rx.borrow().clone()
    }

    pub fn drivers(&self) -> &DriversApi {
        &self.star.drivers
    }

    pub fn artifacts(&self) -> &Artifacts {
        &self.star.machine_api.artifacts
    }

    pub fn registry(&self) -> &Registry {
        &self.star.machine_api.registry
    }

    pub fn new(
        star: HyperStarSkel,
        kind: Kind,
        point: Point,
        selector: KindSelector,
        transmitter: ProtoTransmitter,
        logger: Logger,
        status_tx: watch::Sender<DriverStatus>,
        request_tx: mpsc::Sender<DriverRunnerRequest>,
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
            star,
            kind,
            selector,
            point,
            logger,
            status_tx: mpsc_status_tx,
            status_rx: watch_status_rx,
            request_tx,
        }
    }

    pub async fn create_in_driver(
        &self,
        child_segment_template: PointSegTemplate,
        kind: KindTemplate,
    ) -> Result<Details, DriverErr> {
        let create = Create {
            template: Template::new(
                PointTemplate {
                    parent: self.point.clone(),
                    child_segment_template,
                },
                kind,
            ),
            properties: Default::default(),
            strategy: Strategy::Ensure,
            state: StateSrc::None,
        };
        Ok(self.star.logger.result(
            self.star
                .create_in_star(create)
                .await
                .map_err(|e| SpaceErr::to_space_err(e)),
        )?)
    }

    pub async fn locate(&self, point: &Point) -> Result<ParticleRecord, RegErr> {
        self.star.registry.record(point).await
    }

    pub async fn local_driver_lookup(&self, kind: Kind) -> Result<Option<Point>, SpaceErr> {
        self.star.drivers.local_driver_lookup(kind).await
    }

    pub fn item_ctx(&self, point: &Point, layer: Layer) -> Result<ParticleCtx, DriverErr> {
        let mut router = LayerInjectionRouter::new(
            self.star.clone(),
            point.to_surface().with_layer(Layer::Core),
        );
        router.direction = Some(TraversalDirection::Fabric);
        let mut transmitter =
            ProtoTransmitterBuilder::new(Arc::new(router), self.star.exchanger.clone());
        transmitter.from = SetStrategy::Fill(point.to_surface().with_layer(layer));
        transmitter.agent = SetStrategy::Fill(Agent::Point(point.clone()));
        let transmitter = transmitter.build();
        let ctx = ParticleCtx { transmitter };
        Ok(ctx)
    }
}

pub struct DriverFactoryWrapper {
    pub factory: Box<dyn DriverFactory>,
}

impl DriverFactoryWrapper {
    pub fn wrap(factory: Box<dyn DriverFactory>) -> Arc<dyn HyperDriverFactory> {
        Arc::new(Self { factory })
    }
}

#[async_trait]
impl HyperDriverFactory for DriverFactoryWrapper {
    fn kind(&self) -> Kind {
        self.factory.kind()
    }

    fn selector(&self) -> KindSelector {
        self.factory.selector()
    }

    async fn create(
        &self,
        star_skel: HyperStarSkel,
        driver_skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        self.factory.create(driver_skel, ctx).await
    }
}

#[async_trait]
pub trait DriverFactory: Send + Sync {
    fn kind(&self) -> Kind;
    fn selector(&self) -> KindSelector;

    fn avail(&self) -> DriverAvail {
        DriverAvail::External
    }

    async fn create(&self, skel: DriverSkel, ctx: DriverCtx) -> Result<Box<dyn Driver>, DriverErr>;

    fn properties(&self) -> SetProperties {
        SetProperties::default()
    }
}

#[async_trait]
pub trait HyperDriverFactory: Send + Sync {
    fn kind(&self) -> Kind;
    fn selector(&self) -> KindSelector;

    fn avail(&self) -> DriverAvail {
        DriverAvail::External
    }

    async fn create(
        &self,
        star: HyperStarSkel,
        skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr>;

    fn properties(&self) -> SetProperties {
        SetProperties::default()
    }
}

#[derive(Clone)]
pub struct HyperSkel {
    pub star: HyperStarSkel,
    pub driver: DriverSkel,
}

impl HyperSkel {
    pub fn new(star: HyperStarSkel, driver: DriverSkel) -> Self {
        Self { star, driver }
    }
}

#[async_trait]
pub trait Driver: Send + Sync {
    fn kind(&self) -> Kind;

    fn layer(&self) -> Layer {
        Layer::Core
    }

    fn avail(&self) -> DriverAvail {
        DriverAvail::External
    }

    async fn init(&mut self, skel: DriverSkel, _: DriverCtx) -> Result<(), DriverErr> {
        skel.logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();
        Ok(())
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr>;

    async fn handler(&self) -> Box<dyn DriverHandler> {
        Box::new(DefaultDriverHandler::restore())
    }

    /// This is sorta a hack, it only works for DriverDriver
    async fn add_driver(&self, _driver: DriverApi) {}
}

#[async_trait]
pub trait DriverHandler: DirectedHandler {}

#[derive(DirectedHandler)]
pub struct DefaultDriverHandler {}

impl DriverHandler for DefaultDriverHandler {}

impl DefaultDriverHandler {
    fn restore() -> Self {
        DefaultDriverHandler {}
    }
}

#[handler]
impl DefaultDriverHandler {
    #[route("Hyp<Assign>")]
    pub async fn assign(&self, _ctx: InCtx<'_, HyperSubstance>) -> Result<(), SpaceErr> {
        Ok(())
    }
}

pub trait States: Sync + Sync
where
    Self::State: ParticleState,
{
    type State;
    fn new() -> Self;

    fn create(assign: Assign) -> Arc<RwLock<Self::State>>;
    fn get(point: &Point) -> Option<&Arc<RwLock<Self::State>>>;
    fn remove(point: &Point) -> Option<Arc<RwLock<Self::State>>>;
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

pub trait ParticleState: Send + Sync {}

pub struct ParticleSphere {
    inner: ParticleSphereInner,
}

impl ParticleSphere {
    pub async fn init(&self) -> Result<Status, DriverErr> {
        Ok(Status::Ready)
    }

    pub fn new_handler<H>(handler: H) -> Self
    where
        H: DirectedHandler + 'static,
    {
        Self {
            inner: ParticleSphereInner::Handler(Box::new(handler)),
        }
    }

    pub fn new_router<R>(router: R) -> Self
    where
        R: TraversalRouter + 'static,
    {
        Self {
            inner: ParticleSphereInner::Router(Box::new(router)),
        }
    }
}

impl Deref for ParticleSphere {
    type Target = ParticleSphereInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub enum ParticleSphereInner {
    Handler(Box<dyn DirectedHandler>),
    Router(Box<dyn TraversalRouter>),
}

impl ParticleSphereInner {}

#[derive(Clone, Eq, PartialEq)]
pub enum DriverAvail {
    Internal,
    External,
}

#[async_trait]
pub trait Particle
where
    Self::Err: ParticleErr,
{
    type Skel;
    type Ctx;
    type State;
    type Err;

    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self;

    async fn init(&self) -> Result<Status, Self::Err> {
        Ok(Status::Ready)
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err>;
}

/*
#[async_trait]
pub trait ParticleHandler: DirectedHandler + Send + Sync + Particle
{

}

 */

#[async_trait]
pub trait ParticleRouter: TraversalRouter + Send + Sync + Particle {}

#[derive(Clone)]
pub struct HyperParticleSkel {
    pub skel: DriverSkel,
    pub point: Point,
    pub kind: Kind,
}

#[derive(Clone)]
pub struct ParticleSkel {
    skel: DriverSkel,
    pub point: Point,
    pub kind: Kind,
    pub bind: ArtRef<BindConfig>,
}

impl ParticleSkel {
    pub fn new(point: Point, kind: Kind, bind: ArtRef<BindConfig>, skel: DriverSkel) -> Self {
        Self {
            point,
            kind,
            skel,
            bind,
        }
    }

    pub fn data_dir(&self) -> String {
        self.skel.data_dir()
    }
}

pub struct ParticleCtx {
    pub transmitter: ProtoTransmitter,
}

pub struct DriverDriverFactory {}

impl DriverDriverFactory {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl HyperDriverFactory for DriverDriverFactory {
    fn kind(&self) -> Kind {
        Kind::Driver
    }

    fn selector(&self) -> KindSelector {
        KindSelector::from_base(BaseKind::Driver)
    }

    fn avail(&self) -> DriverAvail {
        DriverAvail::Internal
    }

    async fn create(
        &self,
        star: HyperStarSkel,
        driver: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        Ok(Box::new(DriverDriver::new(driver).await?))
    }
}

#[derive(Clone)]
pub struct DriverDriverParticleSkel {
    skel: DriverSkel,
    pub api: DriverApi,
}

impl DriverDriverParticleSkel {
    pub fn new(skel: DriverSkel, api: DriverApi) -> Self {
        Self { skel, api }
    }
}

impl Deref for DriverDriverParticleSkel {
    type Target = DriverSkel;

    fn deref(&self) -> &Self::Target {
        &self.skel
    }
}

pub struct DriverDriver {
    skel: DriverSkel,
    map: DashMap<Point, DriverApi>,
}

impl DriverDriver {
    async fn new(skel: DriverSkel) -> Result<Self, DriverErr> {
        let map = DashMap::new();
        Ok(Self { skel, map })
    }
}

impl DriverDriver {
    pub fn get_driver(&self, point: &Point) -> Option<DriverApi> {
        let rtn = self.map.get(point);
        match rtn {
            None => None,
            Some(driver) => Some(driver.value().clone()),
        }
    }
}

#[async_trait]
impl Driver for DriverDriver {
    fn kind(&self) -> Kind {
        Kind::Driver
    }

    async fn add_driver(&self, api: DriverApi) {
        let point = api.get_point();
        self.map.insert(point, api);
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let api = self
            .get_driver(point)
            .ok_or(DriverErr::DriverApiNotFound(point.clone()))?;
        let skel = DriverDriverParticleSkel::new(self.skel.clone(), api);
        let particle = Box::new(DriverParticle::restore(skel, (), ()));

        Ok(any_result(particle.sphere())?)
    }
}

#[derive(DirectedHandler)]
pub struct DriverParticle {
    skel: DriverDriverParticleSkel,
}

impl Particle for DriverParticle {
    type Skel = DriverDriverParticleSkel;
    type Ctx = ();
    type State = ();
    type Err = ParticleDriverErr;

    fn restore(skel: Self::Skel, _: Self::Ctx, _: Self::State) -> Self {
        Self { skel }
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
        //        let router: dyn TraversalRouter = Box::new(self);
        Ok(ParticleSphere::new_router(self))
    }
}

#[async_trait]
impl TraversalRouter for DriverParticle {
    async fn traverse(&self, traversal: Traversal<Wave>) -> Result<(), SpaceErr> {
        self.skel.api.handle(traversal).await;
        Ok(())
    }
}

impl ParticleRouter for DriverParticle {}

#[derive(Error, Debug, ToSpaceErr)]
pub enum ParticleDriverErr {
    #[error(transparent)]
    DriverErr(#[from] DriverErr),
}

impl ParticleDriverErr {
    pub fn unwrap(self) -> DriverErr {
        match self {
            ParticleDriverErr::DriverErr(err) => err,
        }
    }
}

#[derive(Error, Debug, ToSpaceErr)]
pub enum ParticleStarErr {
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
    #[error(transparent)]
    StarErr(#[from] StarErr),
    #[error(transparent)]
    RegErr(#[from] RegErr),
    #[error("{0}")]
    Anyhow(#[source] Arc<anyhow::Error>),
}

impl From<anyhow::Error> for ParticleStarErr {
    fn from(err: anyhow::Error) -> Self {
        ParticleStarErr::Anyhow(Arc::new(err))
    }
}

impl From<DriverErr> for ParticleStarErr {
    fn from(err: DriverErr) -> Self {
        StarErr::Anyhow(Arc::new(anyhow!("{}", err))).into()
    }
}

impl ParticleStarErr {
    pub fn unwrap(self) -> StarErr {
        match self {
            ParticleStarErr::StarErr(err) => err,
            ParticleStarErr::RegErr(err) => err.into(),
            ParticleStarErr::SpaceErr(err) => StarErr::SpaceErr(err),
            ParticleStarErr::Anyhow(any) => StarErr::Anyhow(any),
            e => StarErr::Anyhow(Arc::new(anyhow!("{}", e))),
            _ => StarErr::Anyhow(Arc::new(anyhow!("exhausted error"))),
        }
    }
}

impl CoreReflector for ParticleStarErr {
    fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::fail(),
            body: Substance::Err(SubstanceErr(self.to_string())),
        }
    }
}

impl CoreReflector for ParticleDriverErr {
    fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::fail(),
            body: Substance::Err(SubstanceErr(self.to_string())),
        }
    }
}

impl ParticleErr for ParticleStarErr {}

impl ParticleErr for ParticleDriverErr {}

#[derive(Error, Debug, ToSpaceErr)]
pub enum DriverErr {
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
    #[error(transparent)]
    ArtErr(#[from] ArtErr),
    #[error(transparent)]
    RegErr(#[from] RegErr),
    #[error(transparent)]
    MachineErr(#[from] MachineErr),
    #[error(transparent)]
    StdParticleErr(#[from] StdParticleErr),
    #[error(transparent)]
    ParticleStarErr(#[from] ParticleStarErr),
    #[error(transparent)]
    ControlErr(#[from] ControlErr),
    #[error(transparent)]
    FileStoreErr(#[from] FileStoreErr),
    #[error(transparent)]
    ServiceErr(#[from] ServiceErr),
    #[error("particle router for '{point}<{kind}>' is not set")]
    ParticleRouterNotSet { point: Point, kind: Kind },
    #[error("tokio recv error")]
    OneshotRecvErr(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("tokio send error")]
    TokioMpscSendErr,
    #[error("expected environment variable to be set: {0}")]
    ExpectEnvVar(String),
    #[error("DriverApi is not associated with point: '{0}'")]
    DriverApiNotFound(Point),
    #[error("'{0}'")]
    String(String),
    #[error("{0}")]
    Anyhow(#[from] Arc<anyhow::Error>),
}

impl DriverErr {
    pub fn driver_not_found<S>(name: S) -> Self
    where
        S: ToString,
    {
        Self::String(name.to_string())
    }
    pub fn particle_router_not_set(point: &Point, kind: &Kind) -> Self {
        let point = point.clone();
        let kind = kind.clone();
        DriverErr::ParticleRouterNotSet { point, kind }
    }

    pub fn expected_env<S>(key: S) -> Self
    where
        S: ToString,
    {
        Self::ExpectEnvVar(key.to_string())
    }
}

pub trait ParticleErr: std::error::Error + Send + Sync + 'static + CoreReflector + Sized {}

pub type StdParticleErr = Box<StdParticleErrInner>;

#[derive(Error, Debug, Clone, ToSpaceErr)]
pub enum StdParticleErrInner {
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
}

impl CoreReflector for StdParticleErr {
    fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::fail(),
            body: Substance::Err(SubstanceErr(self.to_string())),
        }
    }
}

impl ParticleErr for StdParticleErr {}

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for DriverErr {
    fn from(value: SendError<T>) -> Self {
        DriverErr::TokioMpscSendErr
    }
}
