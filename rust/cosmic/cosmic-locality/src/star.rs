use crate::driver::Drivers;
use crate::field::{FieldEx, FieldState};
use crate::machine::MachineSkel;
use crate::portal::{PortalInlet, PortalShell};
use crate::shell::ShellEx;
use crate::state::{ParticleStates, PortalInletState, PortalShellState, ShellState};
use mesh_portal_versions::version::v0_0_1::wave::{HyperWave, DirectedCore, SysMethod};
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use mesh_portal_versions::error::MsgErr;
use mesh_portal_versions::version::v0_0_1::cli::RawCommand;
use mesh_portal_versions::version::v0_0_1::id::id::{Kind, Layer, Point, Port, PortSelector, RouteSeg, Topic, ToPoint, ToPort, TraversalLayer, Uuid};
use mesh_portal_versions::version::v0_0_1::id::{StarKey, StarSub, TraversalInjection};
use mesh_portal_versions::version::v0_0_1::id::{Traversal, TraversalDirection};
use mesh_portal_versions::version::v0_0_1::log::PointLogger;
use mesh_portal_versions::version::v0_0_1::parse::Env;
use mesh_portal_versions::version::v0_0_1::quota::Timeouts;
use mesh_portal_versions::version::v0_0_1::substance::substance::Substance;
use mesh_portal_versions::version::v0_0_1::sys::{Assign, Sys};
use mesh_portal_versions::version::v0_0_1::util::ValueMatcher;
use mesh_portal_versions::version::v0_0_1::wave::{Agent, DirectedHandler, Transmitter, InCtx, ProtoTransmitter, Ping, Reflectable, ReflectedCore, Pong, RootInCtx, Router, SetStrategy, Wave};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot};
use tokio::time::error::Elapsed;
use mesh_portal::version::latest::messaging::Scope;
use mesh_portal::version::latest::util::uuid;
use mesh_portal_versions::{DriverState, RegistryApi};
use mesh_portal_versions::version::v0_0_1::bin::Bin;

#[derive(Clone)]
pub struct StarState {
    pub field: Arc<DashMap<Point, FieldState>>,
    pub shell: Arc<DashMap<Point, ShellState>>,
    pub driver: Arc<DashMap<Kind,Arc<DashMap<Point, DriverState>>>>,
    pub portal_inlet: Arc<DashMap<Point, PortalInletState>>,
    pub portal_shell: Arc<DashMap<Point, PortalShellState>>,
    pub topic: Arc<DashMap<Port, Box<dyn TopicHandler>>>,
}

impl StarState {
    pub fn new() -> Self {
        Self {
            field: Arc::new(DashMap::new()),
            shell: Arc::new(DashMap::new()),
            driver: Arc::new(DashMap::new()),
            portal_inlet: Arc::new(DashMap::new()),
            portal_shell: Arc::new(DashMap::new()),
            topic: Arc::new(DashMap::new()),
        }
    }

    pub fn find_topic(
        &self,
        port: &Port,
        source: &Port,
    ) -> Option<Result<&Box<dyn TopicHandler>, MsgErr>> {
        match self.topic.get(port) {
            None => None,
            Some(topic) => {
                let topic = topic.value();
                if topic.source_selector().is_match(source).is_ok() {
                    Some(Ok(topic))
                } else {
                    Some(Err(MsgErr::forbidden()))
                }
            }
        }
    }

    pub fn find_field(&self, point: &Point) -> FieldState {
        match self.field.get(point) {
            None => {
                let rtn = FieldState::new();
                self.field.insert(point.clone(), rtn.clone());
                rtn
            }
            Some(rtn) => rtn.value().clone(),
        }
    }

    pub fn find_shell(&self, point: &Point) -> ShellState {
        match self.shell.get(point) {
            None => {
                let rtn = ShellState::new();
                self.shell.insert(point.clone(), rtn.clone());
                rtn
            }
            Some(rtn) => rtn.value().clone(),
        }
    }

    pub fn find_portal_inlet(&self, point: &Point) -> PortalInletState {
        match self.portal_inlet.get(point) {
            None => {
                let rtn = PortalInletState::new();
                self.portal_inlet.insert(point.clone(), rtn.clone());
                rtn
            }
            Some(rtn) => rtn.value().clone(),
        }
    }

    pub fn find_portal_shell(&self, point: &Point) -> PortalShellState {
        match self.portal_shell.get(point) {
            None => {
                let rtn = PortalShellState::new();
                self.portal_shell.insert(point.clone(), rtn.clone());
                rtn
            }
            Some(rtn) => rtn.value().clone(),
        }
    }

    pub fn find_driver(&self, point: &Point) -> DriverState {
        match self.driver.get(point) {
            None => {
                let rtn = DriverState::None;
                self.driver.insert(point.clone(), rtn.clone());
                rtn
            }
            Some(rtn) => rtn.value().clone(),
        }
    }
}

#[derive(Clone)]
pub struct StarSkel {
    pub key: StarKey,
    pub kind: StarSub,
    pub logger: PointLogger,
    pub registry: Arc<dyn RegistryApi>,
    pub surface: mpsc::Sender<Wave>,
    pub traverse_to_next: mpsc::Sender<Traversal<Wave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub fabric: mpsc::Sender<Wave>,
    pub machine: MachineSkel,
    pub exchange: Arc<DashMap<Uuid, oneshot::Sender<Pong>>>,
    pub state: StarState,
}

impl StarSkel {
    pub fn location(&self) -> &Point {
        &self.logger.point
    }
}

pub enum StarCall {
    HyperWave(HyperWave),
    TraverseToNext(Traversal<Wave>),
    Inject(TraversalInjection)
}

pub struct StarTx {
    surface: mpsc::Sender<HyperWave>,
    traverse_to_next: mpsc::Sender<Traversal<Wave>>,
    inject_tx: mpsc::Sender<TraversalInjection>,
    call_rx: mpsc::Receiver<StarCall>,
}

impl StarTx {
    pub fn new() -> Self {
        let (surface_tx, mut surface_rx) = mpsc::channel(1024);
        let (inject_tx, mut inject_rx) = mpsc::channel(1024);
        let (traverse_to_next_tx, mut traverse_to_next_rx) = mpsc::channel(1024);

        let (call_tx, call_rx) = mpsc::channel(1024);

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(wave) = surface_rx.recv().await {
                    call_tx.send(StarCall::HyperWave(wave)).await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(traversal) = traverse_to_next_rx.recv().await {
                    call_tx.send(StarCall::TraverseToNext(traversal)).await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(inject) = inject_rx.recv().await {
                    call_tx.send(StarCall::Inject(inject)).await;
                }
            });
        }


        Self {
            surface: surface_tx,
            traverse_to_next: traverse_to_next_tx,
            inject_tx,
            call_rx,
        }
    }
}

#[derive(AsyncRequestHandler)]
pub struct Star {
    skel: StarSkel,
    call_rx: mpsc::Receiver<StarCall>,
    drivers: Drivers,
    injector: Port
}

impl Star {
    pub fn new(skel: StarSkel, mut call_rx: mpsc::Receiver<StarCall>, drivers: Drivers) {
        let mut injector = skel.location().clone().push("injector").unwrap().to_port();
        injector.layer = Layer::Surface;

        let star = Self {
            skel,
            call_rx,
            drivers,
            injector
        };
        star.start();
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.call_rx.recv().await {
                match call {
                    StarCall::HyperWave(wave) => {
                        self.hyperwave(wave).await;
                    }
                    StarCall::TraverseToNext(traversal) => {
                        self.traverse_to_next(traversal).await;
                    }
                    StarCall::Inject(inject) => {
                        self.start_traversal(inject.wave,&inject.injector).await;
                    }
                }
            }
        });
    }

    async fn hyperwave(&self, wave: HyperWave) {

        let record = match self
            .skel
            .registry
            .locate(&wave.wave.to().clone().to_point())
            .await{
            Ok(record) => record,
            Err(err) => {
                self.skel.logger.error(err.to_string());
            }
        };

        if record.location != *self.skel.location() {
            // need to send a not_found to sender... even if the wave type was Response!
            self.skel.logger.warn("attempt to send a wave to a point that is not hosted by this star");
            return;
        }

        // first check if this wave was intended for the star itself... if it was
        // we will need to keep the HyperWave data
        let wave = if wave.to().point == *self.skel.location() {
            if wave.wave.is_req() {
                let req = Ping {
                    id: uuid(),
                    agent: Agent::Point(self.skel.location().clone()),
                    scope: Scope::Full,
                    handling: Default::default(),
                    from: self.skel.location().clone().to_port().with_layer(Layer::Surface),
                    to: self.skel.location().clone().to_port().with_layer(Layer::Core),
                    core: DirectedCore {
                        headers: Default::default(),
                        method: SysMethod::HyperWave.into(),
                        uri: Default::default(),
                        body: Substance::HyperWave(Box::new(wave))
                    }
                };
                Wave::Req(req)
            } else {
                wave.wave
            }
        } else {
            wave.wave
        };

        if wave.to().point == self.injector.point {
            self.skel.logger.warn("attempt to spoof Star injector");
            return;
        }

        self.start_traversal(wave,&self.injector).await;
    }

    async fn start_traversal(&self, wave: Wave, injector: &Port ) {
        let record = match self
            .skel
            .registry
            .locate(&wave.to().clone().to_point())
            .await {
            Ok(record) => record,
            Err(err) => {
                self.skel.logger.error( err.to_string() );
                return;
            }
        };

        let location = record.location.clone().ok_or()?;
        let plan = record.details.stub.kind.wave_traversal_plan();

        let mut dest = None;
        let mut dir = TraversalDirection::Core;
        // determine layer destination. A dest of None will send all the way to the Fabric or Core
        if location == *self.skel.location()  {

            // now we check if we are doing an inter point delivery (from one layer to another in the same Particle)
            if wave.to().point == wave.from().point {
                // it's the SAME point, so the to layer becomes our dest
                dest.replace(wave.to().layer.clone() );

                // make sure we have this layer in the plan
                let plan = record.details.stub.kind.wave_traversal_plan();
                if !plan.has_layer(&wave.to().layer) {
                    self.skel.logger.warn("attempt to send wave to layer that the recipient Kind does not have in its traversal plan");
                    return;
                }

                // dir is from inject_layer to dest
                dir = match TraversalDirection::new(&injector.layer, &wave.to().layer) {
                    Ok(dir) => dir,
                    Err(_) => {
                        // looks like we are already on the dest layer...
                        // that means it doesn't matter what the TraversalDirection is
                        TraversalDirection::Fabric
                    }
                }
            } else {
                // if this wave was injected by the from Particle, then we need to first
                // traverse towards the fabric
                if injector.point == *wave.from() {
                    dir = TraversalDirection::Fabric;
                } else {
                    // if this was injected by something else (like the Star)
                    // then it needs to traverse towards the Core
                    dir = TraversalDirection::Core;
                    // and dest will be the to layer
                    dest.replace(wave.to().layer.clone());
                }
            }
        } else {
           // location is outside of this Star, so dest is None and direction if Fabric
           dir = TraversalDirection::Fabric;
           dest = None;
        }

        // next we determine the direction of the traversal

        // if the recipient is not even in this star, traverse towards fabric
        if location != *self.skel.location() {
            TraversalDirection::Fabric
        }
        // if the recipient and from are the same perform a normal traversal
        else if wave.to().point == wave.from().point {
            TraversalDirection::new( &self.layer, &wave.to().layer ).unwrap()
        } else {
            // finally we handle the case where we traverse towards another point within this Star
            // in this case it just depends upon if we are Requesting or Responding
            if wave.is_req() {
                TraversalDirection::Core
            } else {
                TraversalDirection::Fabric
            }
        }

        let logger = self.skel.logger.point(wave.to().clone().to_point());
        let logger = logger.span();

        let mut traversal = Traversal::new(
            wave,
            record,
            location,
            injector.layer.clone(),
            logger,
            dir,
            dest
        );

        // in the case that we injected into a layer that is not part
        // of this plan, we need to send the traversal to the next layer
        if !plan.has_layer(&injector) {
            traversal.next();
        }

        // alright, let's visit the injection layer first...
        self.visit_layer(traversal).await;
    }


    async fn visit_layer(&self, traversal: Traversal<Wave>) {
        if traversal.is_req()
            && self.skel.state.topic.contains_key(traversal.to())
        {
            let topic = self.skel.state.find_topic(traversal.to(), traversal.from());
            match topic {
                None => {
                    // send some sort of Not_found
                    let mut traversal = traversal.unwrap_req();
                    let mut traversal = traversal.with(traversal.not_found());
                    traversal.reverse();
                    let traversal = traversal.wrap();
                    self.traverse_to_next(traversal).await;
                    return;
                }
                Some(result) => {
                    match result {
                        Ok(topic_handler) => {
                            let transmitter = StarInjectTransmitter::new( self.skel.clone(), traversal.to().clone() );
                            let transmitter = ProtoTransmitter::new(Arc::new(transmitter));
                            let req = traversal.unwrap_req().payload;
                            let ctx = RootInCtx::new(
                                req,
                                self.skel.logger.span(),
                                transmitter
                            );

                            topic_handler.handle(ctx).await;
                        }
                        Err(err) => {
                            // some some 'forbidden' error message sending towards_core...
                        }
                    }
                }
            }
        } else {
            match traversal.layer {
                Layer::PortalInlet => {
                    let inlet = PortalInlet::new(
                        self.skel.clone(),
                        self.skel.state.find_portal_inlet(&traversal.location),
                    );
                    inlet.visit(traversal).await;
                }
                Layer::Field => {
                    let field = FieldEx::new(
                        self.skel.clone(),
                        self.skel.state.find_field(traversal.payload.to()),
                        traversal.logger.clone()
                    );
                    field.visit(traversal).await;
                }
                Layer::Shell => {
                    let shell = ShellEx::new(
                        self.skel.clone(),
                        self.skel.state.find_shell(traversal.payload.to()),
                    );
                    shell.visit(traversal).await;
                }
                Layer::Driver => {
                    self.drivers.visit(traversal).await;
                }
                _ => {
                    self.skel.logger.warn("attempt to traverse wave in the inner layers which the Star does not manage");
                }
            }
        }
    }

    async fn traverse_to_next(&self, mut traversal: Traversal<Wave>) {
        if traversal.dest.is_some() && traversal.layer == *traversal.dest.as_ref().unwrap() {
            self.visit_layer(traversal).await;
            return;
        }

        let next = traversal.next();
        match next {
            None => match traversal.dir {
                TraversalDirection::Fabric => {
                    self.skel.fabric.send(traversal.payload);
                }
                TraversalDirection::Core => {
                    self.skel
                        .logger
                        .warn("should not have traversed a wave all the way to the core in Star");
                }
            },
            Some(_) => {
                self.visit_layer(traversal).await;
            }
        }
    }



    async fn to_fabric(&self, wave: Wave) {
        let skel = self.skel.clone();
        tokio::spawn(async move {
            skel.fabric.send(wave).await;
        });
    }
}

#[routes_async]
impl Star {
    #[route("Sys<Assign>")]
    pub async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, MsgErr> {
        self.drivers.assign(ctx).await
    }
}

#[derive(Clone)]
pub struct StarInjectTransmitter {
    pub skel: StarSkel,
    pub injector: Port
}

impl StarInjectTransmitter {
    pub fn new(skel: StarSkel, injector: Port) -> Self {
        Self { skel, injector }
    }
}

#[async_trait]
impl Transmitter for StarInjectTransmitter {
    async fn direct(&self, request: Ping) -> Result<Pong,MsgErr> {
        let (tx,mut rx) = oneshot::channel();
        self.skel.exchange.insert( request.id.clone(), tx );
        Ok(tokio::time::timeout(Duration::from_secs(self.skel.machine.timeouts.from(&request.handling.wait) ), rx).await??)
    }

    async fn route(&self, wave: Wave) {
        let inject = TraversalInjection::new(self.injector.clone(), wave );
        self.skel.inject_tx.send(inject).await;
    }
}

pub trait TopicHandler: Send + Sync + DirectedHandler {
    fn source_selector(&self) -> &PortSelector;
}

pub trait TopicHandlerSerde<T: TopicHandler> {
    fn serialize(&self, handler: T) -> Substance;
    fn deserialize(&self, ser: Substance ) -> T;
}