use std::cmp::Ordering;
use crate::driver::Drivers;
use crate::field::{FieldEx, FieldState};
use crate::machine::MachineSkel;
use crate::portal::{PortalInlet, PortalShell};
use crate::shell::ShellEx;
use crate::state::{PortalInletState, PortalShellState, ShellState};
use cosmic_api::bin::Bin;
use cosmic_api::cli::RawCommand;
use cosmic_api::command::request::set::Set;
use cosmic_api::config::config::bind::RouteSelector;
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{
    Kind, Layer, Point, Port, PortSelector, RouteSeg, Sub, ToPoint, ToPort, Topic, TraversalLayer,
    Uuid,
};
use cosmic_api::id::{StarKey, StarSub, TraversalInjection};
use cosmic_api::id::{Traversal, TraversalDirection};
use cosmic_api::log::{PointLogger, RootLogger};
use cosmic_api::parse::{route_attribute, Env};
use cosmic_api::quota::Timeouts;
use cosmic_api::substance::substance::{Substance, ToSubstance};
use cosmic_api::sys::{Assign, AssignmentKind, Discovery, Location, Search, Sys};
use cosmic_api::util::{ValueMatcher, ValuePattern};
use cosmic_api::wave::{Agent, Bounce, CoreBounce, DirectedHandler, DirectedHandlerSelector, DirectedKind, DirectedProto, InCtx, Method, Ping, Pong, ProtoTransmitter, RecipientSelector, Recipients, Reflectable, ReflectedCore, RootInCtx, Router, SetStrategy, Signal, Wave, TxRouter, BounceBacks, Echo, Ripple, Handling, HandlingKind, Retries, WaitTime, Priority, Scope, ReflectedWave};
use cosmic_api::wave::{DirectedCore, Exchanger, HyperWave, SysMethod, UltraWave};
use cosmic_api::{MountKind, RegErr, Registration, RegistryApi, State, StateFactory};
use cosmic_driver::{Core, Driver, DriverFactory, DriverLifecycleCall, DriverSkel, DriverStatus};
use cosmic_hyperlane::HyperRouter;
use dashmap::mapref::one::Ref;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use futures::future::{BoxFuture, join_all};
use futures::FutureExt;
use http::StatusCode;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, Mutex, oneshot, RwLock};
use tokio::time::error::Elapsed;
use cosmic_api::command::command::common::StateSrc;
use cosmic_api::particle::particle::{Details, Status, Stub};

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
pub struct StarSkel<E> where E: RegErr {
    pub key: StarKey,
    pub point: Point,
    pub kind: StarSub,
    pub logger: PointLogger,
    pub registry: Arc<dyn RegistryApi<E>>,
    pub field_tx: mpsc::Sender<UltraWave>,
    pub traverse_to_next_tx: mpsc::Sender<Traversal<UltraWave>>,
    pub inject_tx: mpsc::Sender<TraversalInjection>,
    pub fabric_tx: mpsc::Sender<UltraWave>,
    pub machine: MachineSkel<E>,
    pub exchanger: Exchanger,
    pub state: StarState,
    pub connections: Vec<StarCon>,
    pub searcher: StarSearcher,
    pub adjacents: HashSet<Point>,
    pub wrangles: Arc<DashMap<StarSub,StarWrangle>>
}

impl <E> StarSkel<E> where E: RegErr {
    pub fn new(
        template: StarTemplate,
        machine: MachineSkel<E>,
    ) -> Self {
        let point = template.key.clone().to_point();
        let logger = machine.logger.point(point.clone());
        let star_tx = StarTx::new(point.clone());
        let exchanger = Exchanger::new(point.clone().to_port(), machine.timeouts.clone());
        let state = StarState::new();

        let mut adjacents = HashSet::new();
        // prime the searcher by mapping the immediate lanes
        for hyperway in template.hyperway.clone() {
            searcher.add(
                hyperway.key().clone().to_point(),
                hyperway.key().clone().to_point(),
            );

            adjacents.insert(hyperway.key().clone().to_point());
        }

        Self {
            key: template.key,
            point,
            kind: template.kind,
            logger,
            field_tx: star_tx.surface.clone(),
            traverse_to_next_tx: star_tx.traverse_to_next.clone(),
            inject_tx: star_tx.inject_tx.clone(),
            exchanger,
            state,
            connections: template.hyperway,
            registry: machine.registry.clone(),
            machine,
            adjacents,
        }
    }

    pub fn location(&self) -> &Point {
        &self.logger.point
    }

    pub fn create_star_drivers(&self, driver_skel: DriverSkel) -> HashMap<Kind,Box<dyn Driver>> {
        let mut rtn = HashMap::new();
        let star_driver = StarDriver::new( self.clone(), driver_skel );
        rtn.insert( star_driver.kind().clone(), Box::new(star_driver) );
    }
}

pub enum StarCall {
    HyperWave(HyperWave),
    TraverseToNextLayer(Traversal<UltraWave>),
    LayerTraversalInjection(TraversalInjection),
    CreateMount { agent: Agent, kind: MountKind, tx: oneshot::Sender<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>)>}
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
                        from: point.clone(),
                    };
                    call_tx.send(StarCall::HyperWave(wave)).await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(traversal) = traverse_to_next_rx.recv().await {
                    call_tx.send(StarCall::TraverseToNextLayer(traversal)).await;
                }
            });
        }

        {
            let call_tx = call_tx.clone();
            tokio::spawn(async move {
                while let Some(inject) = inject_rx.recv().await {
                    call_tx
                        .send(StarCall::LayerTraversalInjection(inject))
                        .await;
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
    pub kind: StarSub,
    tx: mpsc::Sender<StarCall>,
}

impl StarApi {
    pub fn new(kind: StarSub, tx: mpsc::Sender<StarCall>) -> Self {
        Self { kind, tx }
    }

    pub async fn surface(&self, hyperwave: HyperWave) {
        self.tx.send(StarCall::HyperWave(hyperwave)).await;
    }

    pub async fn traverse_to_next(&self, traversal: Traversal<UltraWave>) {
        self.tx.send(StarCall::TraverseToNextLayer(traversal)).await;
    }

    pub async fn inject(&self, inject: TraversalInjection) {
        self.tx
            .send(StarCall::LayerTraversalInjection(inject))
            .await;
    }
}

#[derive(DirectedHandler)]
pub struct Star<E> where E: RegErr {
    skel: StarSkel<E>,
    star_rx: mpsc::Receiver<StarCall>,
    drivers: Drivers<E>,
    injector: Port,
    mounts: HashMap<Point,StarMount>,
    field_router: TxRouter,
    transmitter: ProtoTransmitter,
    golden_path: DashMap<StarKey,StarKey>
}

impl <E> Star<E> where E: RegErr{
    pub fn new(skel: StarSkel<E>, drivers: Drivers<E>) -> StarApi {
        let (star_tx, star_rx) = mpsc::channel(32 * 1024);
        let mut injector = skel.location().clone().push("injector").unwrap().to_port();
        injector.layer = Layer::Surface;

        let mut golden_path = DashMap::new();
        for con in skel.connections {
            golden_path.insert( con.key().clone(), con.key.clone() );
        }

        let (field_tx,mut field_rx) = mpsc::channel(32*1024);
        {
            let star_tx = star_tx.clone();
            let from = skel.point.clone();
            tokio::spawn(async move {
                while let Some(wave) = field_rx.recv().await {
                    let wave = HyperWave {
                        wave,
                        from: from.clone()
                    };
                    star_tx.send( StarCall::HyperWave(wave) ).await;
                }
            }
            );
        }
        let field_router = TxRouter::new(field_tx);
        let mut transmitter = ProtoTransmitter::new(Arc::new(field_router.clone()), skel.exchanger.clone() );
        transmitter.from = SetStrategy::Override(skel.point.clone().to_port());
        transmitter.handling = SetStrategy::Fill(Handling{
            kind: HandlingKind::Immediate,
            priority: Priority::High,
            retries: Retries::None,
            wait: WaitTime::High
        });
        transmitter.agent = SetStrategy::Fill(Agent::HyperUser);
        transmitter.scope = SetStrategy::Fill(Scope::Full);

        {
            let star = Self {
                skel,
                star_rx,
                drivers,
                injector,
                mounts: HashMap::new(),
                field_router,
                transmitter,
                golden_path
            };
            star.start();
        }
        StarApi::new(skel.kind.clone(), star_tx)
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(call) = self.star_rx.recv().await {
                match call {
                    StarCall::HyperWave(wave) => {
                        self.field(wave).await;
                    }
                    StarCall::TraverseToNextLayer(traversal) => {
                        self.traverse_to_next_layer(traversal).await;
                    }
                    StarCall::LayerTraversalInjection(inject) => {
                        self.star_layer_traversal(inject.wave, &inject.injector)
                            .await;
                    }
                    StarCall::CreateMount { agent, kind, tx } => {
                        self.create_mount(agent, kind, tx).await;
                    }
                }
            }
        });
    }

    async fn surface(&self, wave: HyperWave) -> Result<(), MsgErr> {
        let mut wave = wave.wave;

        let wave = if wave.to().is_single() && wave.to().unwrap_single().point == self.skel.point {
            if let Some(&Method::Sys(SysMethod::Transport)) = wave.method() {
                let signal = wave.to_signal()?;
                if let Substance::UltraWave(wave) = signal.variant.core.body {
                    *wave
                } else {
                    return Err(
                        "expecting an UltraWave Substance body when receiving a transport signal"
                            .into(),
                    );
                }
            } else {
                wave
            }
        } else {
            wave
        };

        wave.inc_hops();
        if wave.hops() > 255 {
            self.skel.logger.warn( "wave went over the hop limit of 255!");
            return Ok(());
        }

        wave.add_to_history(self.skel.point.clone());

        if wave.is_directed() && wave.to().is_match( & self.skel.point) {
            let wave = wave.to_directed()?;
            let reflection = wave.reflection();
            let ctx = RootInCtx::new(wave.to_directed()?, self.skel.point.clone().to_port(), self.skel.logger.span(), self.transmitter.clone() );
            match self.handle(ctx).await {
                CoreBounce::Absorbed => {
                    return Ok(());
                }
                CoreBounce::Reflected(core) => {
                    let reflected = reflection.make( core, self.skel.point.clone().to_port(),self.skel.point.clone().to_port() );
                    self.skel.field_tx.send(reflected.to_ultra() ).await;
                    return Ok(());
                }
            }
        }

        if let UltraWave::Ripple(mut ripple) = wave {
            {
                let mut ripples = ripple.shard_by_location(&self.skel.adjacents, &self.skel.registry).await?;
                if let Some(ripple) = ripples.remove(self.skel.location()) {
                    // this shard of the ripple is targeted towards this star, so
                    // send to layer traversal
                    for singular in ripple
                        .to_singulars(&self.skel.adjacents, &self.skel.registry)
                        .await?
                    {
                        self.star_layer_traversal(singular.to_multiple().to_ultra(), &self.injector)
                            .await;
                    }
                }
                let mut routes = 0;
                // the rest of the shards must belong to other star locations
                for (location, ripple) in ripples {
                    if !ripple.history.contains(&location) {
                        self.route_to_fabric(ripple.to_ultra(), location).await;
                        routes = routes + 1;
                    }
                }
            }

            if routes == 0 && ripple.method == SysMethod::Search.into() {
                if BounceBacks::None != ripple.bounce_backs {
                    let echo = Wave::new( Echo::new( ))
                }
            }

            return Ok(());
        } else {



        }

        self.star_layer_traversal(wave, &self.injector).await;
    }

    async fn star_layer_traversal(&self, wave: UltraWave, injector: &Port) -> Result<(), MsgErr> {
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

    async fn traverse_to_next_layer(&self, mut traversal: Traversal<UltraWave>) {
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
        self.skel.fabric_tx.send(wave).await;
    }

    async fn create_mount(&mut self, agent: Agent, kind: MountKind, tx: oneshot::Sender<(mpsc::Sender<UltraWave>, mpsc::Receiver<UltraWave>)> ) -> Result<(),MsgErr> {
        let point = self.skel.point.clone().push("controls").unwrap();
        let index = self.skel.registry.sequence(&point).await?;
        let point = point.push( format!("control-{}",index)).unwrap();
        let registration = Registration {
            point:point.clone(),
            kind: kind.kind(),
            registry: Default::default(),
            properties: Default::default(),
            owner: agent.point()
        };
        self.skel.registry.register(&registration).await?;
        self.skel.registry.assign(&point, self.skel.location()).await?;

        let (in_mount_tx,mut in_mount_rx) = mpsc::channel(32*1024);
        let (out_mount_tx,out_mount_rx) = mpsc::channel(32*1024);

        tx.send( (in_mount_tx,out_mount_rx) );

        let inject_tx = self.skel.inject_tx.clone();
        {
            let point = point.clone();
            tokio::spawn(async move {
                let port = point.to_port().with_layer(Layer::Core);
                while let Some(wave) = in_mount_rx.recv().await {
                    let inject = TraversalInjection::new(port.clone(), wave);
                    inject_tx.send(inject).await;
                }
            });
        }

        let mount = StarMount {
            point: point.clone(),
            kind,
            tx: out_mount_tx
        };

        self.mounts.insert( point, mount );
        Ok(())
    }

    async fn route_to_fabric(&self, wave: UltraWave, star: Point) -> Result<(), MsgErr> {
        let mut proto = DirectedProto::new();
        proto.to(star.to_port());
        proto.from(self.skel.point.clone().to_port());
        proto.kind(DirectedKind::Signal);
        let mut core = DirectedCore::new(SysMethod::Transport.into());
        core.body = wave.to_substance();
        proto.core(core);
        let directed = proto.build()?;
        let wave = directed.to_ultra();
        self.field_router.route(wave).await;
        Ok(())
    }



    async fn find_golden_way(&self, star_key: &StarKey ) -> Result<Option<StarKey>,MsgErr> {
        if let Some(adjacent) = self.golden_path.get(star_key) {
            Ok(Some(adjacent.value().clone()))
        } else {
            let mut ripple = DirectedProto::new();
            ripple.kind(DirectedKind::Ripple);
            ripple.method(SysMethod::Search.into());
            ripple.body(Substance::Sys(Sys::Search(Search::Star(star_key.clone()))));
            ripple.bounce_backs = Option::new(BounceBacks::Count(self.skel.adjacents.len()));
            ripple.to(Recipients::Stars);
            let echoes:Vec<ReflectedWave> = self.transmitter.direct(ripple).await?;

            let mut colated = vec![];
            for echo in echoes {
                let echo = echo.to_echo()?;
                if let Substance::Sys(Sys::Discoveries(discoveries)) = &echo.core().body {
                    for discovery in discoveries {
                        colated.push(StarDiscovery::new(StarPair::new(self.skel.key.clone(), StarKey::try_from(echo.from.point.clone())?), discovery.clone()));
                    }
                } else {
                    self.skel.logger.warn("unexpected reflected core substance from search echo");
                }
            }

            colated.sort();

            match colated.first() {
                None => {
                    Ok(None)
                }
                Some(discovery) => {
                    let key = discovery.pair.not(&self.skel.key).clone();
                    self.golden_path.insert( star_key.clone(), key.clone() );
                    Ok(Some(key))
                }
            }
        }
    }


    async fn re_ripple( &self, ripple: Wave<Ripple> ) -> Result<Vec<Wave<Echo>>,MsgErr> {
        let mut reflections: Vec<BoxFuture<Wave<Echo>>> = vec![];

        for (location, ripple) in ripple.shard_by_location(&self.skel.adjacents, &self.skel.registry ).await?
        {
           if !ripple.history.contains(&location) {
               let key = StarKey::try_from(location)?;
               let adjacent = self.find_golden_way(&key).await?.ok_or("could not find golden way")?.to_port();
               let mut wave = DirectedProto::new();
               wave.kind(DirectedKind::Signal);
               wave.to(adjacent);
               wave.method(SysMethod::Transport.into());
               wave.handling(ripple.handling.clone());
               wave.agent(Agent::HyperUser);
               wave.body(Substance::UltraWave(Box::new(ripple.to_ultra())));
               reflections.push(self.transmitter.direct(wave).boxed());
           }
        }

        if reflections.is_empty() {
            return Ok(vec![]);
        }

        let echoes = join_all(reflections).await?;

        Ok(echoes)
    }

}

pub struct StarMount {
    pub point: Point,
    pub kind: MountKind,
    pub tx: mpsc::Sender<UltraWave>
}

#[routes]
impl <E> Star<E> where E: RegErr {
    #[route("Sys<Assign>")]
    pub async fn assign(&self, ctx: InCtx<'_, Sys>) -> Result<ReflectedCore, MsgErr> {
        self.drivers.handle(ctx.wave().clone()).await
    }

    #[route("Sys<Search>")]
    pub async fn search_discovery(&self, ctx: InCtx<'_, Sys>) -> CoreBounce {
        fn reflect(star: &Star<E>) -> ReflectedCore where E: RegErr{
            let discovery = Discovery {
                hops: ctx.wave().hops(),
                star_key: star.skel.key.clone(),
                kinds: star.drivers.kinds().clone()
            };
            let mut core = ReflectedCore::new();
            core.body = Substance::Sys(Sys::Discoveries(vec![discovery]));
            core.status = StatusCode::from_u16(200).unwrap();
            core
        }

        if let Sys::Search(search) = ctx.input {
            match search {
                Search::Star(star) => {
                    if self.skel.key == *star {
                        CoreBounce::Reflected(reflect(self))
                    }
                }
                Search::StarKind(kind) => {
                    if *kind == self.skel.kind {
                        CoreBounce::Reflected(reflect(self))
                    }
                }
                Search::Kinds => {
                    CoreBounce::Reflected(reflect(self))
                }
            }
            CoreBounce::Absorbed
        } else {
            CoreBounce::Reflected(ctx.bad_request())
        }
    }
}

#[derive(Clone)]
pub struct LayerInjectionRouter<E> where E:RegErr {
    pub skel: StarSkel<E>,
    pub injector: Port,
}

impl <E> LayerInjectionRouter<E> where E: RegErr {
    pub fn new(skel: StarSkel<E>, injector: Port) -> Self {
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
impl <E> Router for LayerInjectionRouter<E> where E: RegErr {
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

#[derive(Clone)]
pub struct StarTemplate {
    pub key: StarKey,
    pub kind: StarSub,
    pub hyperway: Vec<StarCon>,
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

pub struct StarRouter {
    pub star_api: StarApi,
}

impl StarRouter {
    pub fn new(star_api: StarApi) -> Self {
        Self { star_api }
    }
}

#[async_trait]
impl HyperRouter for StarRouter {
    async fn route(&self, wave: HyperWave) {
        self.star_api.surface(wave).await;
    }
}

#[derive(Clone,Eq,PartialEq,Hash,Ord,PartialOrd)]
pub struct StarPair {
    pub a: StarKey,
    pub b: StarKey,
}

impl StarPair {
   pub fn new( a: StarKey, b: StarKey ) -> Self {
       if a < b {
           Self {
               a,
               b
           }
       } else {
           Self {
               a: b,
               b: a
           }
       }
   }

   pub fn not( &self, this: &StarKey ) -> &StarKey {
       if self.a == *this {
           &self.b
       } else {
           &self.a
       }
   }
}

#[derive(Clone,Eq,PartialEq)]
pub struct StarDiscovery {
    pub pair: StarPair,
    pub discovery: Discovery
}

impl Deref for StarDiscovery {
    type Target = Discovery;

    fn deref(&self) -> &Self::Target {
        &self.discovery
    }
}

impl StarDiscovery {
   pub fn new( pair: StarPair, discovery: Discovery ) -> Self {
       Self {
           pair,
           discovery
       }
   }
}

impl Ord for StarDiscovery {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.discovery.hops != other.discovery.hops {
            self.discovery.hops.cmp(&other.discovery.hops)
        } else {
            self.pair.cmp(&other.pair)
        }
    }
}

impl PartialOrd for StarDiscovery {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.discovery.hops != other.discovery.hops {
            self.discovery.hops.partial_cmp(&other.discovery.hops)
        } else {
            self.pair.partial_cmp(&other.pair)
        }
    }
}





#[derive(DirectedHandler)]
pub struct StarDriver<E>  {
    pub status: DriverStatus,
    pub star_skel: StarSkel<E>,
    pub driver_skel: DriverSkel
}

impl <E> StarDriver<E> {
    pub fn new( star_skel:StarSkel<E>, driver_skel: DriverSkel ) -> Self {
        Self {
            status: DriverStatus::Started,
            star_skel,
            driver_skel
        }
    }

    async fn search_for_stars(&self, search: Search) -> Result<Vec<Discovery>,MsgErr> {
        let mut ripple = DirectedProto::new();
        ripple.kind(DirectedKind::Ripple);
        ripple.method(SysMethod::Search.into());
        ripple.bounce_backs = Option::new(BounceBacks::Count(self.skel.adjacents.len()));
        ripple.body(Substance::Sys(Sys::Search(search)));
        ripple.to(Recipients::Stars);
        let echoes:Vec<ReflectedWave> = self.transmitter.direct(ripple).await?;

        let mut rtn = vec![];
        for echo in echoes {
            if let Substance::Sys(Sys::Discoveries(discoveries)) = &echo.core().body {
                for discovery in discoveries {
                    rtn.push(discovery.clone());
                }
            } else {
                self.skel.logger.warn("unexpected reflected core substance from search echo");
            }
        }

        Ok(rtn)
    }
}



#[async_trait]
impl <E> Driver for StarDriver<E> {

    fn kind(&self) -> &Kind {
        &Kind::Star(self.star_skel.kind.clone())
    }

    async fn status(&self) -> DriverStatus {
        self.status.clone()
    }

    async fn lifecycle(&mut self, event: DriverLifecycleCall) -> Result<DriverStatus,MsgErr> {
        match event {
            DriverLifecycleCall::Init => {
                if self.status == DriverStatus::Ready {
                    return Ok(self.status.clone())
                }

                self.status = DriverStatus::Initializing;

                let mut discoveries = vec![];
                for discovery in self.search_for_stars(Search::Kinds).await? {
                    let discovery = StarDiscovery::new( StarPair::new( self.star_skel.key.clone(), discovery.star_key.clone() ), discovery );
                    discoveries.push(discovery);
                }
                discoveries.sort();

            }
            DriverLifecycleCall::Shutdown => {}
        }

        Ok(self.status.clone())
    }

    fn ex(&self, point: &Point, state: Option<Arc<RwLock<dyn State>>>) -> Box<dyn Core> {

    }

    async fn assign(&mut self, ctx: InCtx<'_,Assign>) -> Result<Option<Arc<RwLock<dyn State>>>, MsgErr> {
        Err("only allowed one Star per StarDriver".into())
    }
}

#[routes]
impl <E> StarDriver<E> {



}

#[derive(DirectedHandler)]
pub struct StarCore<E> {
   pub skel: StarSkel<E>
}

impl <E> Core for StarCore<E> {

}

#[routes]
impl <E> StarCore<E> {

    pub fn new( skel: StarSkel<E> ) -> Self {
        Self {
            skel
        }
    }

    #[route(Sys<Assign>)]
    pub async fn handle_assign( &self, ctx: InCtx<'_,Assign> ) -> Result<ReflectedCore,MsgErr>{
        self.driver_skel.assign(ctx.input.clone()).await?;
        Ok(ReflectedCore::ok())
    }

}

#[derive(Clone)]
pub struct StarWrangles {
    pub wrangles: Arc<DashMap<StarSub, RoundRobinWrangleSelector>>
}

impl StarWrangles {

  pub fn new() -> Self {
      Self {
          wrangles: Arc::new(DashMap::new())
      }
  }

  pub fn add( &self, discoveries: Vec<StarDiscovery> ) {
      let mut map = HashMap::new();
      for discovery in discoveries {
          match map.get_mut(&discovery.star_kind) {
              None => {
                  map.insert( discovery.star_kind.clone(), vec![discovery]);
              }
              Some(discoveries) => {
                  discoveries.push(discovery)
              }
          }
      }

      for (kind,discoveries) in map {
          let wrangler = RoundRobinWrangleSelector::new(kind, discoveries );
          self.wrangles.insert(wrangler.kind.clone(), wrangler );
      }
  }

  pub fn verify( &self, kinds: &[&StarSub] ) -> Result<(),MsgErr> {
      for kind in kinds {
          if !self.wrangles.contains_key(*kind) {
              Err(format!("star must be able to wrangle at least one {}", kind.to_string()).into())
          }
      }
      Ok(())
  }

  pub async fn wrangle( &self, kind: &StarSub ) -> Result<&StarKey,MsgErr>{
      self.wrangles.get(kind).ok_or(format!("could not find wrangles for kind {}", kind.to_string()))?.value().wrangle()
  }


}

pub struct RoundRobinWrangleSelector {
    pub kind: StarSub,
    pub stars: Vec<StarDiscovery>,
    pub index: Mutex<usize>,
    pub step_index: usize
}

impl RoundRobinWrangleSelector {

   pub fn new( kind: StarSub, mut stars: Vec<StarDiscovery> ) -> Self {
       stars.sort();
       let mut step_index = 0;
       let mut hops = -1;
       for discovery in &stars {
           if hops < 0 {
               hops = discovery.hops;
           }
           else if discovery.hops > hops {
               break;
           }
           step_index += 1;
       }
       Self {
           kind,
           stars,
           index: Mutex::new(0),
           step_index
       }
   }


   pub async fn wrangle( &self ) -> Result<&StarKey,MsgErr>{

       if self.stars.is_empty() {
           return Err(format!("cannot find wrangle for kind: {}",self.kind.to_string()).into());
       }

       let index = {
           let mut lock = self.index.lock().await;
           let index:usize = *lock;
           lock += 1;
           index
       };

       let index = index % self.step_index;

       if let Some(discovery) = self.stars.get(index) {
           Ok(&discovery.value().key)
       } else {
           Err(format!("cannot find wrangle for kind: {}",self.kind.to_string()).into())
       }

   }
}


