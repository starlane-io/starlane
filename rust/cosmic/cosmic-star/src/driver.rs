use cosmic_api::RegistryApi;
use crate::machine::MachineSkel;
use crate::star::{LayerInjectionRouter, StarSkel, StarState, StateApi, StateCall};
use cosmic_api::State;
use dashmap::DashMap;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{Kind, Layer, Point, Port, ToPoint, ToPort, TraversalLayer, Uuid};
use cosmic_api::id::{StarKey, Traversal, TraversalInjection};
use cosmic_api::log::PointLogger;
use cosmic_api::particle::particle::Status;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, Sys};
use cosmic_api::wave::{CoreBounce, DirectedHandler, DirectedHandlerSelector, DirectedWave, Exchanger, InCtx, Ping, Pong, ProtoTransmitter, RecipientSelector, ReflectedCore, ReflectedWave, RootInCtx, Router, SetStrategy, UltraWave, Wave};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Sender;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use cosmic_api::config::config::bind::RouteSelector;
use cosmic_api::parse::route_attribute;
use cosmic_api::util::ValuePattern;
use cosmic_driver::{CoreEx, DriverCore, DriverFactory, DriverLifecycleCall, DriverShellRequest, DriverSkel, DriverStatus, DriverStatusEvent};

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
        self.skel.traverse_to_next_tx.send(traversal).await;
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
    pub tx: mpsc::Sender<DriverShellCall>
}


impl DriverApi {

    pub fn new(tx: mpsc::Sender<DriverShellCall>) -> Self {
        Self {
            tx
        }
    }

    pub async fn status(&self) -> Result<DriverStatus,MsgErr> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( DriverShellCall::Status(tx) ).await;
        Ok(tokio::time::timeout(Duration::from_secs(60),rx).await??)
    }

    pub async fn lifecycle(&self, call: DriverLifecycleCall ) -> Result<DriverStatus,MsgErr> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( DriverShellCall::LifecycleCall{call, tx} ).await;

        tokio::time::timeout(Duration::from_secs(5*60),rx).await??
    }

    pub async fn traversal(&self, traversal: Traversal<UltraWave>) {
        self.tx.send( DriverShellCall::Traversal(traversal) ).await;
    }

    pub async fn handle(&self, wave: DirectedWave ) -> Result<ReflectedCore,MsgErr> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( DriverShellCall::Handle{ wave, tx } ).await;
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
            let (shell_tx,shell_rx) = mpsc::channel(1024);
            let (tx, mut rx) = mpsc::channel(1024);
            {
                let shell_tx = shell_tx.clone();
                tokio::spawn(async move {
                    while let Some(call) = rx.recv().await {
                        match call {
                            DriverShellRequest::Ex { point, tx } => {
                                let call = DriverShellCall::Ex { point, tx };
                                shell_tx.send(call).await;
                            }
                        }
                    }
                });
            }
            let router = Arc::new(LayerInjectionRouter::new( skel.clone(), point.clone().to_port().with_layer(Layer::Core)));
            let driver_skel = DriverSkel::new( point.clone(), router, tx );
            let core = factory.create(driver_skel);
            let state = skel.state.api().with_layer(Layer::Core);
            let shell = DriverShell::new(point, skel.clone(), core, state, shell_tx, shell_rx);
            let shell = DriverApi::new(shell);
            drivers.insert(factory.kind().clone(), shell);
        }
        Ok(Drivers::new(skel, drivers))
    }
}

pub enum DriverShellCall {
    LifecycleCall{ call: DriverLifecycleCall, tx: oneshot::Sender<Result<DriverStatus,MsgErr>>},
    Status(oneshot::Sender<DriverStatus>),
    Traversal(Traversal<UltraWave>),
    Handle{ wave: DirectedWave, tx:oneshot::Sender<Result<ReflectedCore,MsgErr>>},
    Ex{ point: Point, tx: oneshot::Sender<Result<Box<dyn CoreEx>,MsgErr>>}
}

pub struct Core {
   pub port: Port,
   pub skel: StarSkel,
   pub state: Option<Arc<RwLock<dyn State>>>,
   pub ex: Box<dyn CoreEx>,
   pub router: Arc<dyn Router>
}

#[async_trait]
impl TraversalLayer for Core {
    fn port(&self) -> &cosmic_api::id::id::Port {
        &self.port
    }

    async fn delivery_directed(&self, direct: Traversal<DirectedWave> ) {
        let logger = self.skel.logger.point(self.port().clone().to_point()).span();
        let mut transmitter = ProtoTransmitter::new( self.router.clone(), self.skel.exchanger.clone() );
        transmitter.from = SetStrategy::Override(self.port.clone());
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
    tx: mpsc::Sender<DriverShellCall>,
    rx: mpsc::Receiver<DriverShellCall>,
    state: StateApi,
    core: Box<dyn DriverCore>,
    router: Arc<LayerInjectionRouter>
}

#[routes]
impl DriverShell {

    pub fn new( point: Point, skel: StarSkel, core: Box<dyn DriverCore>, states: StateApi, tx: mpsc::Sender<DriverShellCall>, rx: mpsc::Receiver<DriverShellCall>) -> mpsc::Sender<DriverShellCall>{

        let router = Arc::new(LayerInjectionRouter::new(skel.clone(), point.clone().to_port().with_layer(Layer::Driver) ));

        let driver = Self {
            point,
            skel,
            status: DriverStatus::Started,
            tx: tx.clone(),
            rx,
            state: states,
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
                    DriverShellCall::Handle{ wave, tx } => {
                        let port = wave.to().clone().unwrap_single();
                        let logger = self.skel.logger.point(port.clone().to_point()).span();
                        let transmitter = ProtoTransmitter::new( self.router.clone(), self.skel.exchanger.clone() );
                        let ctx = RootInCtx::new( wave, port.clone(), logger, transmitter );
                        match self.handle(ctx).await {
                            CoreBounce::Absorbed => {
                                tx.send(Err(MsgErr::server_error()));
                            }
                            CoreBounce::Reflect(reflect) => {
                                tx.send(Ok(reflect));
                            }
                        }
                    }
                    DriverShellCall::Ex { point, tx } => {
                        match self.state.get_state(point.clone().to_port().with_layer(Layer::Core)).await {
                            Ok(state) => {
                                tx.send(Ok(self.core.ex(&point,state)));
                            }
                            Err(err) => {
                                tx.send(Err(err));
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
            skel: self.skel.clone(),
            state: state.clone(),
            ex: self.core.ex(point,state),
            router: self.router.clone(),
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

