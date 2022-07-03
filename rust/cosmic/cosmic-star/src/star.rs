use crate::driver::Drivers;
use crate::field::{FieldEx, FieldState};
use crate::machine::MachineSkel;
use crate::portal::{PortalInlet, PortalShell};
use crate::shell::ShellEx;
use crate::state::{PortalInletState, PortalShellState, ShellState};
use cosmic_api::bin::Bin;
use cosmic_api::cli::RawCommand;
use cosmic_api::config::config::bind::RouteSelector;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{
    Kind, Layer, Point, Port, PortSelector, RouteSeg, ToPoint, ToPort, Topic, TraversalLayer, Uuid,
};
use cosmic_api::id::{StarKey, StarSub, TraversalInjection};
use cosmic_api::id::{Traversal, TraversalDirection};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::parse::{route_attribute, Env};
use cosmic_api::quota::Timeouts;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::{Assign, Location, Sys};
use cosmic_api::util::{ValueMatcher, ValuePattern};
use cosmic_api::wave::{
    Agent, Bounce, DirectedHandler, DirectedHandlerSelector, InCtx, Ping, Pong, ProtoTransmitter,
    RecipientSelector, Reflectable, ReflectedCore, RootInCtx, Router, SetStrategy, Wave,
};
use cosmic_api::wave::{DirectedCore, Exchanger, HyperWave, SysMethod, UltraWave};
use cosmic_api::{RegistryApi, State, StateFactory};
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::time::error::Elapsed;
use cosmic_driver::DriverFactory;

#[derive(Clone)]
pub struct StarState {
    states: Arc<DashMap<Port, Arc<RwLock<dyn State>>>>,
    topic: Arc<DashMap<Port, Arc<dyn TopicHandler>>>,
    tx: mpsc::Sender<StateCall>,
    field: Arc<DashMap<Port, FieldState>>,
    shell: Arc<DashMap<Port, ShellState>>,
}

impl StarState {
    pub fn new() -> Self {
        let states: Arc<DashMap<Port, Arc<RwLock<dyn State>>>> = Arc::new(DashMap::new());

        let (tx, mut rx) = mpsc::channel(32 * 1024);

        {
            let states = states.clone();
            tokio::spawn(async move {
                while let Some(call) = rx.recv().await {
                    match call {
                        StateCall::Get { port, tx } => match states.get(&port) {
                            None => {
                                tx.send(Ok(None));
                            }
                            Some(state) => {
                                tx.send(Ok(Some(state.value().clone())));
                            }
                        },
                        StateCall::Put { port, state, tx } => {
                            if states.contains_key(&port) {
                                tx.send(Err(MsgErr::bad_request()));
                            } else {
                                states.insert(port, state);
                                tx.send(Ok(()));
                            }
                        }
                    }
                }
            });
        }

        Self {
            states,
            topic: Arc::new(DashMap::new()),
            field: Arc::new(DashMap::new()),
            shell: Arc::new(DashMap::new()),
            tx,
        }
    }

    pub fn api(&self) -> StateApi {
        StateApi::new(self.tx.clone())
    }

    pub fn states_tx(&self) -> mpsc::Sender<StateCall> {
        self.tx.clone()
    }

    pub fn topic_handler(&self, port: Port, handler: Arc<dyn TopicHandler>) {
        self.topic.insert(port, handler);
    }

    pub async fn find_state<S>(&self, port: &Port) -> Result<Arc<RwLock<dyn State>>, MsgErr> {
        Ok(self
            .states
            .get(port)
            .ok_or(format!("could not find state for: {}", port.to_string()))?
            .value()
            .clone())
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

    pub fn find_field(&self, port: &Port) -> Result<FieldState, MsgErr> {
        Ok(self
            .field
            .get(port)
            .ok_or("expected field state")?
            .value()
            .clone())
    }

    pub fn find_shell(&self, port: &Port) -> Result<ShellState, MsgErr> {
        Ok(self
            .shell
            .get(port)
            .ok_or("expected shell state")?
            .value()
            .clone())
    }
}

#[derive(Clone)]
pub struct StarSkel {
    pub key: StarKey,
    pub kind: StarSub,
    pub logger: PointLogger,
    pub registry: Arc<dyn RegistryApi>,
    pub surface_tx: mpsc::Sender<UltraWave>,
    pub traverse_to_next_tx: mpsc::Sender<Traversal<UltraWave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub fabric_tx: mpsc::Sender<UltraWave>,
    pub machine: MachineSkel,
    pub exchanger: Exchanger,
    pub state: StarState,
    pub connections: Vec<StarCon>
}

impl StarSkel {

    pub fn new( template: StarTemplate, machine: MachineSkel, fabric_tx: mpsc::Sender<UltraWave>) -> Self {
        let point = template.key.clone().to_point();
        let logger = machine.logger.point(point.clone());
        let star_tx = StarTx::new(point.clone());
        let exchanger = Exchanger::new(point.to_port(), machine.timeouts.clone() );
        let state = StarState::new();

        Self {
            key: template.key,
            kind: template.kind,
            logger,
            surface_tx: star_tx.surface.clone(),
            traverse_to_next_tx: star_tx.traverse_to_next.clone(),
            inject_tx: star_tx.inject_tx.clone(),
            fabric_tx: fabric_tx,
            machine: machine,
            exchanger,
            state,
            connections: template.connections,
            registry: machine.registry.clone()
        }
    }


    pub fn location(&self) -> &Point {
        &self.logger.point
    }
}

pub enum StarCall {
    HyperWave(HyperWave),
    TraverseToNext(Traversal<UltraWave>),
    Inject(TraversalInjection),
}

pub struct StarTx {
    surface: mpsc::Sender<UltraWave>,
    traverse_to_next: mpsc::Sender<Traversal<UltraWave>>,
    inject_tx: mpsc::Sender<TraversalInjection>,
    call_rx: mpsc::Receiver<StarCall>,
}

impl StarTx {
    pub fn new(point: Point) -> Self {
        let (surface_tx, mut surface_rx) = mpsc::channel(1024);
        let (inject_tx, mut inject_rx) = mpsc::channel(1024);
        let (traverse_to_next_tx, mut traverse_to_next_rx) = mpsc::channel(1024);

        let (call_tx, call_rx) = mpsc::channel(1024);

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(wave) = surface_rx.recv().await {
                    let wave = HyperWave {
                        wave,
                        from: point.clone()
                    };
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

#[derive(Clone)]
pub struct StarApi {
    tx: mpsc::Sender<StarCall>
}

impl StarApi {
    pub fn new( tx: mpsc::Sender<StarCall>) -> Self {
        Self {
            tx
        }
    }
    pub async fn hyperwave( &self, hyperwave: HyperWave ) {
        self.tx.send( StarCall::HyperWave(hyperwave)).await;
    }

    pub async fn traverse_to_next( &self, traversal: Traversal<UltraWave>) {
        self.tx.send( StarCall::TraverseToNext(traversal)).await;
    }

    pub async fn inject( &self, inject: TraversalInjection) {
        self.tx.send( StarCall::Inject(inject)).await;
    }
}

#[derive(DirectedHandler)]
pub struct Star {
    skel: StarSkel,
    star_rx: mpsc::Receiver<StarCall>,
    drivers: Drivers,
    injector: Port,
}

impl Star {
    pub fn new(skel: StarSkel, drivers: Drivers) -> StarApi {
        let (star_tx,star_rx) = mpsc::channel(32*1024);
        let mut injector = skel.location().clone().push("injector").unwrap().to_port();
        injector.layer = Layer::Surface;

        {
            let star = Self {
                skel,
                star_rx,
                drivers,
                injector,
            };
            star.start();
        }
        StarApi::new(star_tx)
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.star_rx.recv().await {
                match call {
                    StarCall::HyperWave(wave) => {
                        self.hyperwave(wave).await;
                    }
                    StarCall::TraverseToNext(traversal) => {
                        self.traverse_to_next(traversal).await;
                    }
                    StarCall::Inject(inject) => {
                        self.start_traversal(inject.wave, &inject.injector).await;
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
            .await
        {
            Ok(record) => record,
            Err(err) => {
                self.skel.logger.error(err.to_string());
                return;
            }
        };

        match record.location {
            Location::Central => {
                self.skel
                    .logger
                    .error("attempt to send a wave to a point that is Nowhere");
                return;
            }
            Location::Nowhere => {
                self.skel
                    .logger
                    .error("attempt to send a wave to a point that is Nowhere");
                return;
            }
            Location::Somewhere(location) => {
                if location != *self.skel.location() {
                    // need to send a not_found to sender... even if the wave type was Response!
                    self.skel
                        .logger
                        .warn("attempt to send a wave to a point that is not hosted by this star");
                    return;
                }
            }
        }

        if wave.to().unwrap_single().point == self.injector.point {
            self.skel.logger.warn("attempt to spoof Star injector");
            return;
        }

        self.start_traversal(wave, &self.injector).await;
    }

    async fn start_traversal(&self, wave: UltraWave, injector: &Port) -> Result<(), MsgErr> {
        let record = match self
            .skel
            .registry
            .locate(&wave.to().clone().unwrap_single().to_point())
            .await
        {
            Ok(record) => record,
            Err(err) => {
                self.skel.logger.error(err.to_string());
                return Err(err);
            }
        };

        let location = record.location.clone().ok_or()?;
        let plan = record.details.stub.kind.wave_traversal_plan().clone();

        let mut dest = None;
        let mut dir = TraversalDirection::Core;
        // determine layer destination. A dest of None will send all the way to the Fabric or Core
        if location == *self.skel.location() {
            // now we check if we are doing an inter point delivery (from one layer to another in the same Particle)
            if wave.to().clone().unwrap_single().point == wave.from().point {
                // it's the SAME point, so the to layer becomes our dest
                dest.replace(wave.to().clone().unwrap_single().layer);

                // make sure we have this layer in the plan
                if !plan.has_layer(&wave.to().clone().unwrap_single().layer) {
                    self.skel.logger.warn("attempt to send wave to layer that the recipient Kind does not have in its traversal plan");
                    return Err(MsgErr::forbidden());
                }

                // dir is from inject_layer to dest
                dir = match TraversalDirection::new(
                    &injector.layer,
                    &wave.to().clone().unwrap_single().layer,
                ) {
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

        let logger = self
            .skel
            .logger
            .point(wave.to().clone().unwrap_single().to_point());
        let logger = logger.span();
        let to = wave.to().clone().unwrap_single();

        let point = if *injector == self.injector {
            // if injected by the star then the destination is the point that this traversal belongs to
            to.clone().to_point()
        } else {
            // if injected by any other point then the injector is the point that this traversal belongs to
            injector.clone().to_point()
        };

        let mut traversal = Traversal::new(
            wave,
            record,
            location,
            injector.layer.clone(),
            logger,
            dir,
            dest,
            to,
            point,
        );

        // in the case that we injected into a layer that is not part
        // of this plan, we need to send the traversal to the next layer
        if !plan.has_layer(&injector.layer) {
            traversal.next();
        }

        // alright, let's visit the injection layer first...
        self.visit_layer(traversal).await;
        Ok(())
    }

    async fn visit_layer(&self, traversal: Traversal<UltraWave>) -> Result<(), MsgErr> {
        if traversal.is_directed() && self.skel.state.topic.contains_key(&traversal.to) {
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
                            let transmitter =
                                LayerInjectionRouter::new(self.skel.clone(), traversal.to.clone());
                            let transmitter = ProtoTransmitter::new(
                                Arc::new(transmitter),
                                self.skel.exchanger.clone(),
                            );
                            let to = traversal.to.clone();
                            let directed = traversal.unwrap_directed().payload;
                            let ctx =
                                RootInCtx::new(directed, to, self.skel.logger.span(), transmitter);

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
                        traversal.point.clone(),
                        self.skel.clone(),
                        self.skel.state.find_field(&traversal.to)?,
                        traversal.logger.clone(),
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
                    self.skel.fabric_tx.send(traversal.payload);
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
            skel.fabric_tx.send(wave).await;
        });
    }
}

#[routes]
impl Star {
    #[route("Sys<Assign>")]
    pub async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, MsgErr> {
        self.drivers.handle(ctx.wave().clone()).await
    }
}

#[derive(Clone)]
pub struct LayerInjectionRouter {
    pub skel: StarSkel,
    pub injector: Port,
}

impl LayerInjectionRouter {
    pub fn new(skel: StarSkel, injector: Port) -> Self {
        Self { skel, injector }
    }

    pub fn with(&self, injector: Port) -> Self {
        Self {
            skel: self.skel.clone(),
            injector,
        }
    }
}

#[async_trait]
impl Router for LayerInjectionRouter {
    async fn route(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.injector.clone(), wave);
        self.skel.inject_tx.send(inject).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.injector.clone(), wave);
        self.skel.inject_tx.try_send(inject);
    }
}

pub trait TopicHandler: Send + Sync + DirectedHandler {
    fn source_selector(&self) -> &PortSelector;
}

pub trait TopicHandlerSerde<T: TopicHandler> {
    fn serialize(&self, handler: T) -> Substance;
    fn deserialize(&self, ser: Substance) -> T;
}

impl StateApi {
    pub fn new(tx: mpsc::Sender<StateCall>) -> Self {
        Self {
            tx,
            layer_filter: None,
        }
    }

    pub fn with_layer(self, layer: Layer) -> Self {
        Self {
            tx: self.tx,
            layer_filter: Some(layer),
        }
    }
}

#[derive(Clone)]
pub struct StateApi {
    pub tx: mpsc::Sender<StateCall>,
    layer_filter: Option<Layer>,
}

impl StateApi {
    pub async fn get_state(&self, port: Port) -> Result<Option<Arc<RwLock<dyn State>>>, MsgErr> {
        if let Some(layer) = &self.layer_filter {
            if port.layer != *layer {
                return Err(MsgErr::forbidden_msg(format!(
                    "not allowed to get state from Port Layer {} try layer {}",
                    port.layer.to_string(),
                    layer.to_string()
                )));
            }
        }
        let (tx, rx) = oneshot::channel();
        self.tx.send(StateCall::Get { port, tx }).await;
        rx.await?
    }

    pub async fn put_state(&self, port: Port, state: Arc<RwLock<dyn State>>) -> Result<(), MsgErr> {
        if let Some(layer) = &self.layer_filter {
            if port.layer != *layer {
                return Err(MsgErr::forbidden_msg(format!(
                    "not allowed to put state on Port Layer {} try layer {}",
                    port.layer.to_string(),
                    layer.to_string()
                )));
            }
        }
        let (tx, rx) = oneshot::channel();
        self.tx.send(StateCall::Put { port, state, tx }).await;
        rx.await?
    }
}

pub enum StateCall {
    Get {
        port: Port,
        tx: oneshot::Sender<Result<Option<Arc<RwLock<dyn State>>>, MsgErr>>,
    },
    Put {
        port: Port,
        state: Arc<RwLock<dyn State>>,
        tx: oneshot::Sender<Result<(), MsgErr>>,
    },
}

pub struct StarTemplate {
    pub key: StarKey,
    pub kind: StarSub,
    pub connections: Vec<StarCon>,
}

#[derive(Clone)]
pub enum StarCon {
    Receive(StarKey),
    Connect(StarKey),
}

impl StarCon {
    pub fn key(&self) -> &StarKey {
        match self {
            StarCon::Receive(key) => key,
            StarCon::Connect(key) => key,
        }
    }
}


