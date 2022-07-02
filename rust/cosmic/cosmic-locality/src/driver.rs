use mesh_portal_versions::RegistryApi;
use crate::machine::MachineSkel;
use crate::star::{LayerInjectionRouter, StarSkel, StarState, StateCall};
use mesh_portal_versions::State;
use dashmap::DashMap;
use mesh_portal_versions::error::MsgErr;
use mesh_portal_versions::version::v0_0_1::id::id::{Kind, Layer, ToPoint, ToPort, TraversalLayer, Uuid};
use mesh_portal_versions::version::v0_0_1::id::{StarKey, Traversal, TraversalInjection};
use mesh_portal_versions::version::v0_0_1::log::PointLogger;
use mesh_portal_versions::version::v0_0_1::particle::particle::Status;
use mesh_portal_versions::version::v0_0_1::substance::substance::Substance;
use mesh_portal_versions::version::v0_0_1::sys::{Assign, Sys};
use mesh_portal_versions::version::v0_0_1::wave::{Bounce, DirectedHandler, DirectedHandlerSelector, RecipientSelector, RootInCtx, InCtx, Ping, ReflectedCore, Pong, Wave, UltraWave, Exchanger, DirectedWave, ReflectedWave, Router, ProtoTransmitter};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use mesh_portal::version::latest::config::bind::RouteSelector;
use mesh_portal::version::latest::id::{Point, Port};

#[derive(DirectedHandler)]
pub struct Drivers {
    pub skel: StarSkel,
    pub drivers: HashMap<Kind,DriverApi>,
}

impl Drivers {
    pub fn new(skel: StarSkel, drivers: HashMap<Kind, DriverApi>) -> Self {
        Self { skel, drivers }
    }

    pub fn kinds(&self) -> Vec<Kind> {
        let mut rtn = vec![];
        for (kind,_) in &self.drivers {
            rtn.push(kind.clone())
        }
        rtn
    }

    pub async fn init(&self) -> Result<(), MsgErr> {
        let mut errs = vec![];
        for driver in self.drivers.values() {
            let status = driver.status().await?;
            if status != DriverStatus::Ready
                && status != DriverStatus::Initializing
            {
                match driver.lifecycle(DriverLifecycleCall::Init).await  {
                    Ok(status) => {
                        if status != DriverStatus::Ready {
                            errs.push(MsgErr::server_error());
                        }
                    }
                    Err(err) => {
                        errs.push(err);
                    }
                }
            }
        }

        if !errs.is_empty() {
            // need to fold these errors into one
            Err(MsgErr::server_error())
        } else {
            Ok(())
        }
    }
}

impl Drivers {

    pub async fn handle( &self, wave: DirectedWave ) -> Result<ReflectedCore,MsgErr> {
        let record = self.skel.registry.locate(&wave.to().single_or()?.point).await?;
        let driver = self.drivers.get(&record.details.stub.kind).ok_or::<MsgErr>("do not handle this kind of driver".into())?;
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
        self.skel.traverse_to_next.send(traversal).await;
    }

    async fn start_inner_traversal(&self, traversal: Traversal<UltraWave>) {
    }


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
                    driver.traversal(traversal).await;
                }
            }
        } else {
            self.start_outer_traversal(traversal).await;
        }
    }
}

#[derive(Clone)]
pub struct DriverApi {
    pub tx: mpsc::Sender<DriverCall>
}


impl DriverApi {

    pub fn new(tx: mpsc::Sender<DriverCall>) -> Self {
        Self {
            tx
        }
    }

    pub async fn status(&self) -> Result<DriverStatus,MsgErr> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( DriverCall::Status(tx) ).await;
        Ok(tokio::time::timeout(Duration::from_secs(60),rx).await??)
    }

    pub async fn lifecycle(&self, call: DriverLifecycleCall ) -> Result<DriverStatus,MsgErr> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( DriverCall::LifecycleCall{call, tx} ).await;

        tokio::time::timeout(Duration::from_secs(5*60),rx).await??
    }

    pub async fn traversal(&self, traversal: Traversal<UltraWave>) {
        self.tx.send( DriverCall::Traversal(traversal) ).await;
    }

    pub async fn handle(&self, wave: DirectedWave ) -> Result<ReflectedCore,MsgErr> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( DriverCall::Handle{ wave, tx } ).await;
        tokio::time::timeout(Duration::from_secs(30),rx).await??
    }
}

pub struct DriversBuilder {
    pub factories: HashMap<Kind, Box<dyn DriverFactory>>,
    pub logger: Option<PointLogger>,
}

impl DriversBuilder {
    pub fn add(&mut self, factory: Box<dyn DriverFactory>) {
        self.factories.insert(factory.kind().clone(), factory);
    }

    pub fn logger(&mut self, logger: PointLogger) {
        self.logger.replace(logger);
    }

    pub fn build(self, drivers_port: Port, skel: StarSkel) -> Result<Drivers, MsgErr> {
        if self.logger.is_none() {
            return Err("expected point logger to be set".into());
        }
        let mut drivers = HashMap::new();
        for (kind,factory) in self.factories {
            let point = drivers_port.point.push( kind.as_point_segments() )?;
            let driver_skel = DriverSkel::new( skel.clone(), point.clone() );
            let core = factory.create(driver_skel);
            let shell = DriverShell::new(point, skel.clone(), core, skel.state.states_tx());
            let shell = DriverApi::new(shell);
            drivers.insert(factory.kind().clone(), shell);
        }
        Ok(Drivers::new(skel, drivers))
    }
}

pub trait DriverFactory {
    fn kind(&self) -> &Kind;
    fn create(&self, skel: DriverSkel) -> Box<dyn DriverCore>;
}

pub enum DriverCall {
    LifecycleCall{ call: DriverLifecycleCall, tx: oneshot::Sender<Result<DriverStatus,MsgErr>>},
    Status(oneshot::Sender<DriverStatus>),
    Traversal(Traversal<UltraWave>),
    Handle{ wave: DirectedWave, tx:oneshot::Sender<Result<ReflectedCore,MsgErr>>}
}

pub struct Core {
   pub port: Port,
   pub skel: DriverSkel,
   pub state: Arc<RwLock<dyn State>>,
   pub ex: Box<dyn CoreEx>
}

#[async_trait]
impl TraversalLayer for Core {
    fn port(&self) -> &mesh_portal_versions::version::v0_0_1::id::id::Port {
        &self.port
    }

    async fn delivery_directed(&self, direct: Traversal<DirectedWave> ) {
        let logger = self.skel.logger.point(self.port().clone().to_point()).span();
        let transmitter = ProtoTransmitter::new( Arc::new(self.skel.router.with(self.port.clone())), self.skel.exchanger.clone() );
        let to = direct.to().clone().unwrap_single();
        let ctx = RootInCtx::new( direct.payload, to, logger, transmitter );
        self.ex.handle(ctx ).await;
    }

    async fn deliver_reflected(&self, reflect: Traversal<ReflectedWave>) {
        self.exchanger().reflected(reflect.payload).await;
    }


    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        self.skel.traversal_router.send(traversal).await;
    }

    async fn inject(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.port().clone(),wave);
        self.skel.inject_tx.send(inject).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }
}

#[derive(DirectedHandler)]
pub struct DriverShell {
    point: Point,
    skel: StarSkel,
    status: DriverStatus,
    tx: mpsc::Sender<DriverCall>,
    rx: mpsc::Receiver<DriverCall>,
    states_tx: mpsc::Sender<StateCall>,
    core: Box<dyn DriverCore>,
    router: Arc<LayerInjectionRouter>
}

#[routes]
impl DriverShell {

    pub fn new( point: Point, skel: StarSkel, core: Box<dyn DriverCore>, states_tx: mpsc::Sender<StateCall>) -> mpsc::Sender<DriverCall>{

        let (tx,rx) = mpsc::channel(1024);
        let router = Arc::new(LayerInjectionRouter::new(skel.clone(), point.clone().to_port().with_layer(Layer::Driver) ));

        let driver = Self {
            point,
            skel,
            status: DriverStatus::Started,
            tx: tx.clone(),
            rx,
            states_tx,
            core,
            router
        };

        driver.start();

        tx
    }

    fn start( mut self ) {
        tokio::spawn(async move {
            while let Some(call) = self.rx.recv().await {
                match call {
                    DriverCall::LifecycleCall { call, tx } => {
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
                    DriverCall::Status(tx) => {
                        tx.send(self.status.clone());
                    }
                    DriverCall::Traversal(traversal) => {
                        self.traverse(traversal).await;
                    }
                    DriverCall::Handle{ wave, tx } => {
                        let port = wave.to().clone().unwrap_single();
                        let logger = self.skel.logger.point(port.clone().to_point()).span();
                        let transmitter = ProtoTransmitter::new( self.router.clone(), self.skel.exchanger.clone() );
                        let ctx = RootInCtx::new( wave, port.clone(), logger, transmitter );
                        match self.handle(ctx).await {
                            Bounce::Absorbed => {
                                tx.send(Err(MsgErr::server_error()));
                            }
                            Bounce::Reflect(reflect) => {
                                tx.send(Ok(reflect));
                            }
                        }

                    }
                }
            }
        });
    }

    async fn traverse( &self, traversal: Traversal<UltraWave> )  {

    }

    async fn lifecycle(&self, call: DriverLifecycleCall) -> Result<DriverStatus,MsgErr> {
        self.core.lifecycle(call).await
    }

    async fn core(&self, point: &Point ) -> Result<Core,MsgErr> {
        let port = point.clone().to_port().with_layer(Layer::Core);
        let (tx,mut rx) = oneshot::channel();
        self.skel.state.states_tx().send(StateCall::Get{ port: port.clone(), tx }).await;
        let state = rx.await??;
        Ok(Core {
            port,
            skel: DriverSkel::new(self.skel.clone(), point.clone()),
            state: state.clone(),
            ex: self.core.ex(point,state)
        })
    }

    #[route("Sys<Assign>")]
    async fn assign(&self, ctx: InCtx<'_,Sys>) -> Result<ReflectedCore, MsgErr> {
        match ctx.input {
            Sys::Assign(assign) => {
                let ctx = ctx.push_input_ref(assign);
                self.core.assign(ctx).await
            }
            _ => {
                Err(MsgErr::bad_request())
            }
        }
    }

    fn status(&self) -> &DriverStatus {
        & self.status
    }

}




#[async_trait]
pub trait DriverCore: CoreEx {
    fn kind(&self) -> &Kind;
    async fn status(&self) -> DriverStatus;
    async fn lifecycle(&self, event: DriverLifecycleCall) -> Result<DriverStatus,MsgErr>;
    fn ex(&self, point: &Point, state: Arc<RwLock<dyn State>>) -> Box<dyn CoreEx>;
    async fn assign(&self, ctx: InCtx<'_,Assign>) -> Result<ReflectedCore, MsgErr>;
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum DriverLifecycleCall {
    Init,
    Shutdown,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub enum DriverStatus {
    Unknown,
    Started,
    Initializing,
    Ready,
    Unavailable,
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct DriverStatusEvent {
    pub driver: Point,
    pub status: DriverStatus
}

#[derive(Clone)]
pub struct DriverSkel {
    pub location: Point,
    pub star: StarKey,
    pub logger: PointLogger,
    pub registry: Arc<dyn RegistryApi>,
    pub surface: mpsc::Sender<UltraWave>,
    pub traversal_router: mpsc::Sender<Traversal<UltraWave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub fabric: mpsc::Sender<UltraWave>,
    pub machine: MachineSkel,
    pub exchanger: Exchanger,
    pub status_tx: broadcast::Sender<DriverStatusEvent>,
    pub point: Point,
    pub state_tx: mpsc::Sender<StateCall>,
    pub router: Arc<LayerInjectionRouter>
}

impl  DriverSkel {
    fn new(skel: StarSkel, point: Point) -> Self {
        let location = skel.location().clone();
        let (status_tx,_) = broadcast::channel(16);
        let logger = skel.logger.point(point.clone());
        let router = Arc::new( LayerInjectionRouter::new( skel.clone(), point.clone().to_port().with_layer(Layer::Core)));
        Self {
            location,
            star: skel.key,
            logger,
            registry: skel.registry,
            surface: skel.surface,
            traversal_router: skel.traverse_to_next,
            fabric: skel.fabric,
            machine: skel.machine,
            exchanger: skel.exchanger,
            inject_tx: skel.inject_tx,
            status_tx,
            point,
            state_tx: skel.state.states_tx(),
            router,
        }
    }
}

pub trait CoreEx: DirectedHandler+Send+Sync {
    fn create(&self) -> Option<Box<dyn State>>;
}
