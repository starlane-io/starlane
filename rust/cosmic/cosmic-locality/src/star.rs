use std::collections::HashMap;
use crate::driver::Drivers;
use crate::field::{FieldEx, FieldState};
use crate::machine::MachineSkel;
use crate::portal::{PortalInlet, PortalShell};
use crate::shell::ShellEx;
use crate::state::{PortalInletState, PortalShellState, ShellState};
use mesh_portal_versions::version::v0_0_1::wave::{HyperWave, DirectedCore, SysMethod, UltraWave, Exchanger};
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
use mesh_portal_versions::version::v0_0_1::sys::{Assign, Location, Sys};
use mesh_portal_versions::version::v0_0_1::util::ValueMatcher;
use mesh_portal_versions::version::v0_0_1::wave::{Bounce,Agent, DirectedHandler, DirectedHandlerSelector, RecipientSelector,  InCtx, ProtoTransmitter, Ping, Reflectable, ReflectedCore, Pong, RootInCtx, Router, SetStrategy, Wave};
use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::error::Elapsed;
use mesh_portal::version::latest::messaging::Scope;
use mesh_portal::version::latest::util::uuid;
use mesh_portal_versions::{State, RegistryApi, StateFactory};
use mesh_portal_versions::version::v0_0_1::bin::Bin;

#[derive(Clone)]
pub struct StarState {
    states: Arc<DashMap<Port,Arc<RwLock<dyn State>>>>,
    topic: Arc<DashMap<Port,Arc<dyn TopicHandler>>>,
    tx: mpsc::Sender<StateCall>,
    field: Arc<DashMap<Port,FieldState>>,
    shell: Arc<DashMap<Port,ShellState>>,
}

impl StarState {
    pub fn new() -> Self {
        let states : Arc<DashMap<Port,Arc<RwLock<dyn State>>>>= Arc::new(DashMap::new());

        let (tx,mut rx) = mpsc::channel(32*1024);

        {
            let states = states.clone();
            tokio::spawn(async move {
                while let Some(call) = rx.recv().await {
                    match call {
                        StateCall::Get { port, tx } => {
                            match states.get(&port) {
                                None => {
                                    tx.send(Err(MsgErr::not_found()));
                                }
                                Some(state) => {
                                    tx.send(Ok(state.value().clone()));
                                }
                            }
                        }
                    }
                }
            });
        }

        Self {
            states,
            topic: Arc::new(DashMap::new()),
            field: Arc::new( DashMap::new() ),
            shell: Arc::new( DashMap::new() ),
            tx,
        }
    }

    pub fn topic_handler( &self, port: Port, handler: Arc<dyn TopicHandler>) {
        self.topic.insert(port, handler);
    }

    pub async fn find_state<S>(&self, port: &Port ) -> Result<Arc<RwLock<dyn State>>,MsgErr> {
        Ok(self.states.get(port).ok_or(format!("could not find state for: {}",port.to_string()))?.value().clone())
    }

    pub fn find_topic(
        &self,
        port: &Port,
        source: &Port,
    ) -> Option<Result<Arc<dyn TopicHandler>, MsgErr>> {
        match self.topic.get(port) {
            None => None,
            Some(topic) => {
                let topic = topic.value().clone();
                if topic.source_selector().is_match(source).is_ok() {
                    Some(Ok(topic))
                } else {
                    Some(Err(MsgErr::forbidden()))
                }
            }
        }
    }

    pub fn find_field(&self, port: &Port ) -> Result<FieldState,MsgErr> {
        Ok(self.field.get(port).ok_or("expected field state")?.value().clone())
    }

    pub fn find_shell(&self, port: &Port) -> Result<ShellState,MsgErr> {
        Ok(self.shell.get(port).ok_or("expected shell state")?.value().clone())
    }
}

#[derive(Clone)]
pub struct StarSkel {
    pub key: StarKey,
    pub kind: StarSub,
    pub logger: PointLogger,
    pub registry: Arc<dyn RegistryApi>,
    pub surface: mpsc::Sender<UltraWave>,
    pub traverse_to_next: mpsc::Sender<Traversal<UltraWave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub fabric: mpsc::Sender<UltraWave>,
    pub machine: MachineSkel,
    pub exchanger: Exchanger,
    pub state: StarState,
}

impl StarSkel {
    pub fn location(&self) -> &Point {
        &self.logger.point
    }
}

pub enum StarCall {
    HyperWave(HyperWave),
    TraverseToNext(Traversal<UltraWave>),
    Inject(TraversalInjection)
}

pub struct StarTx {
    surface: mpsc::Sender<HyperWave>,
    traverse_to_next: mpsc::Sender<Traversal<UltraWave>>,
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

pub enum StateCall {
    Get{ port: Port, tx: oneshot::Sender<Result<Arc<RwLock<dyn State>>,MsgErr>> }
}

#[derive(DirectedHandler)]
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

        let wave = wave.wave;

        let record = match self
            .skel
            .registry
            .locate(&wave.to().clone().unwrap_single())
            .await{
            Ok(record) => record,
            Err(err) => {
                self.skel.logger.error(err.to_string());
                return;
            }
        };

        match record.location {
            Location::Central => {
                self.skel.logger.error("attempt to send a wave to a point that is Nowhere");
                return;
            }
            Location::Nowhere => {
                self.skel.logger.error("attempt to send a wave to a point that is Nowhere");
                return;
            }
            Location::Somewhere(location) => {
                if location != *self.skel.location() {
                    // need to send a not_found to sender... even if the wave type was Response!
                    self.skel.logger.warn("attempt to send a wave to a point that is not hosted by this star");
                    return;
                }
            }
        }


        if wave.to().unwrap_single().point == self.injector.point {
            self.skel.logger.warn("attempt to spoof Star injector");
            return;
        }

        self.start_traversal(wave,&self.injector).await;
    }

    async fn start_traversal(&self, wave: UltraWave, injector: &Port ) -> Result<(),MsgErr> {
        let record = match self
            .skel
            .registry
            .locate(&wave.to().clone().unwrap_single().to_point())
            .await {
            Ok(record) => record,
            Err(err) => {
                self.skel.logger.error( err.to_string() );
                return Err(err);
            }
        };

        let location = record.location.clone().ok_or()?;
        let plan = record.details.stub.kind.wave_traversal_plan().clone();

        let mut dest = None;
        let mut dir = TraversalDirection::Core;
        // determine layer destination. A dest of None will send all the way to the Fabric or Core
        if location == *self.skel.location()  {

            // now we check if we are doing an inter point delivery (from one layer to another in the same Particle)
            if wave.to().clone().unwrap_single().point == wave.from().point {
                // it's the SAME point, so the to layer becomes our dest
                dest.replace(wave.to().clone().unwrap_single().layer );

                // make sure we have this layer in the plan
                if !plan.has_layer(&wave.to().clone().unwrap_single().layer ) {
                    self.skel.logger.warn("attempt to send wave to layer that the recipient Kind does not have in its traversal plan");
                    return Err(MsgErr::forbidden());
                }

                // dir is from inject_layer to dest
                dir = match TraversalDirection::new(&injector.layer, &wave.to().clone().unwrap_single().layer) {
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
                if injector.point == wave.from().point {
                    dir = TraversalDirection::Fabric;
                } else {
                    // if this was injected by something else (like the Star)
                    // then it needs to traverse towards the Core
                    dir = TraversalDirection::Core;
                    // and dest will be the to layer
                    dest.replace(wave.to().clone().unwrap_single().layer);
                }
            }
        } else {
           // location is outside of this Star, so dest is None and direction if Fabric
           dir = TraversalDirection::Fabric;
           dest = None;
        }

        let logger = self.skel.logger.point(wave.to().clone().unwrap_single().to_point());
        let logger = logger.span();
        let to = wave.to().clone().unwrap_single();

        let mut traversal = Traversal::new(
            wave,
            record,
            location,
            injector.layer.clone(),
            logger,
            dir,
            dest,
            to
        );

        // in the case that we injected into a layer that is not part
        // of this plan, we need to send the traversal to the next layer
        if !plan.has_layer(&injector.layer ) {
            traversal.next();
        }

        // alright, let's visit the injection layer first...
        self.visit_layer(traversal).await;
        Ok(())
    }


    async fn visit_layer(&self, traversal: Traversal<UltraWave>) -> Result<(),MsgErr>{
        if traversal.is_directed()
            && self.skel.state.topic.contains_key(&traversal.to )
        {
            let topic = self.skel.state.find_topic(&traversal.to, traversal.from());
            match topic {
                None => {
                    // send some sort of Not_found
                    /*
                    let mut traversal = traversal.unwrap_directed();
                    let mut traversal = traversal.with(traversal.not_found());
                    traversal.reverse();
                    let traversal = traversal.wrap();
                    self.traverse_to_next(traversal).await;

                     */
                    return Err(MsgErr::not_found());
                }
                Some(result) => {
                    match result {
                        Ok(topic_handler) => {
                            let transmitter = LayerInjectionRouter::new(self.skel.clone(), traversal.to.clone() );
                            let transmitter = ProtoTransmitter::new(Arc::new(transmitter), self.skel.exchanger.clone() );
                            let to = traversal.to.clone();
                            let directed = traversal.unwrap_directed().payload;
                            let ctx = RootInCtx::new(
                                directed,
                                to,
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

                Layer::Field => {
                    let field = FieldEx::new(
                        self.skel.clone(),
                        self.skel.state.find_field(&traversal.to)?,
                        traversal.logger.clone()
                    );
                    field.visit(traversal).await;
                }
                Layer::Shell => {
                    let shell = ShellEx::new(
                        self.skel.clone(),
                        self.skel.state.find_shell(&traversal.to)?,
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
        Ok(())
    }

    async fn traverse_to_next(&self, mut traversal: Traversal<UltraWave>) {
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



    async fn to_fabric(&self, wave: UltraWave) {
        let skel = self.skel.clone();
        tokio::spawn(async move {
            skel.fabric.send(wave).await;
        });
    }
}

#[routes]
impl Star {
    #[route("Sys<Assign>")]
    pub async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, MsgErr> {
        self.drivers.assign(ctx).await
    }
}

#[derive(Clone)]
pub struct LayerInjectionRouter {
    pub skel: StarSkel,
    pub injector: Port
}

impl LayerInjectionRouter {
    pub fn new(skel: StarSkel, injector: Port) -> Self {
        Self { skel, injector }
    }
}

#[async_trait]
impl Router for LayerInjectionRouter {
    async fn route(&self, wave: UltraWave ) {
        let inject = TraversalInjection::new(self.injector.clone(), wave );
        self.skel.inject_tx.send(inject).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.injector.clone(), wave );
        self.skel.inject_tx.try_send(inject);
    }
}

pub trait TopicHandler: Send + Sync + DirectedHandler {
    fn source_selector(&self) -> &PortSelector;
}

pub trait TopicHandlerSerde<T: TopicHandler> {
    fn serialize(&self, handler: T) -> Substance;
    fn deserialize(&self, ser: Substance ) -> T;
}