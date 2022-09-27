use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverSkel, DriverStatus, HyperDriverFactory, Item,
    ItemHandler, ItemSphere,
};
use crate::star::{HyperStarSkel, LayerInjectionRouter};
use crate::{Cosmos, HyperErr, Registration, RegistryApi};
use cosmic_universe::artifact::ArtRef;
use cosmic_universe::command::common::StateSrc;
use cosmic_universe::command::direct::create::Strategy;
use cosmic_universe::config::bind::BindConfig;
use cosmic_universe::err::{CoreReflector, UniErr};
use cosmic_universe::hyper::{
    Assign, AssignmentKind, Discoveries, Discovery, HyperSubstance, ParticleLocation, Search,
};
use cosmic_universe::kind::{BaseKind, Kind, StarSub};
use cosmic_universe::loc::{Layer, Point, StarKey, ToPoint, ToSurface, LOCAL_STAR};
use cosmic_universe::log::{Trackable, Tracker};
use cosmic_universe::parse::bind_config;
use cosmic_universe::particle::traversal::TraversalInjection;
use cosmic_universe::particle::Status;
use cosmic_universe::selector::{KindSelector, Pattern, SubKindSelector};
use cosmic_universe::substance::Substance;
use cosmic_universe::util::{log, ValuePattern};
use cosmic_universe::wave::core::http2::StatusCode;
use cosmic_universe::wave::core::hyp::HypMethod;
use cosmic_universe::wave::core::{CoreBounce, DirectedCore, ReflectedCore};
use cosmic_universe::wave::exchange::asynch::{InCtx, ProtoTransmitter, ProtoTransmitterBuilder, Router};
use cosmic_universe::wave::exchange::SetStrategy;
use cosmic_universe::wave::{
    Agent, BounceBacks, DirectedProto, Echoes, Handling, HandlingKind, Pong, Priority, Recipients,
    Retries, UltraWave, WaitTime, Wave,
};
use cosmic_universe::HYPERUSER;
use dashmap::DashMap;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::{Add, Deref};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::error;

lazy_static! {
    static ref STAR_BIND_CONFIG: ArtRef<BindConfig> = ArtRef::new(
        Arc::new(star_bind()),
        Point::from_str("GLOBAL::repo:1.0.0:/bind/star.bind").unwrap()
    );
}

fn star_bind() -> BindConfig {
    log(bind_config(
        r#"
    Bind(version=1.0.0)
    {
       Route<Hyp<Transport>> -> (());
       Route<Hyp<Assign>> -> (()) => &;
       Route<Hyp<Search>> -> (()) => &;
       Route<Hyp<Provision>> -> (()) => &;
    }
    "#,
    ))
    .unwrap()
}

#[derive(Clone, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct StarPair {
    pub a: StarKey,
    pub b: StarKey,
}

impl StarPair {
    pub fn new(a: StarKey, b: StarKey) -> Self {
        if a < b {
            Self { a, b }
        } else {
            Self { a: b, b: a }
        }
    }

    pub fn not(&self, this: &StarKey) -> &StarKey {
        if self.a == *this {
            &self.b
        } else {
            &self.a
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct StarDiscovery {
    pub pair: StarPair,
    pub discovery: Discovery,
}

impl Deref for StarDiscovery {
    type Target = Discovery;

    fn deref(&self) -> &Self::Target {
        &self.discovery
    }
}

impl StarDiscovery {
    pub fn new(pair: StarPair, discovery: Discovery) -> Self {
        Self { pair, discovery }
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

#[derive(Clone)]
pub struct StarDriverFactory<P>
where
    P: Cosmos + 'static,
{
    pub kind: KindSelector,
    pub phantom: PhantomData<P>,
}

impl<P> StarDriverFactory<P>
where
    P: Cosmos + 'static,
{
    pub fn new(sub: StarSub) -> Self {
        let kind = KindSelector {
            base: Pattern::Exact(BaseKind::Star),
            sub: SubKindSelector::Exact(Some(sub.to_camel_case())),
            specific: ValuePattern::Any,
        };
        Self {
            kind,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<P> HyperDriverFactory<P> for StarDriverFactory<P>
where
    P: Cosmos + 'static,
{
    fn kind(&self) -> KindSelector {
        self.kind.clone()
    }

    fn avail(&self) -> DriverAvail {
        DriverAvail::Internal
    }

    async fn create(
        &self,
        star: HyperStarSkel<P>,
        skel: DriverSkel<P>,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver<P>>, P::Err> {
        Ok(Box::new(StarDriver::new(star, skel, ctx)))
    }
}

pub struct StarDriver<P>
where
    P: Cosmos + 'static,
{
    pub star_skel: HyperStarSkel<P>,
    pub driver_skel: DriverSkel<P>,
    pub ctx: DriverCtx,
}

impl<P> StarDriver<P>
where
    P: Cosmos,
{
    pub fn new(star_skel: HyperStarSkel<P>, driver_skel: DriverSkel<P>, ctx: DriverCtx) -> Self {
        Self {
            star_skel,
            driver_skel,
            ctx,
        }
    }
}

#[async_trait]
impl<P> Driver<P> for StarDriver<P>
where
    P: Cosmos,
{
    fn kind(&self) -> Kind {
        Kind::Star(self.star_skel.kind.clone())
    }

    async fn init(&mut self, skel: DriverSkel<P>, _: DriverCtx) -> Result<(), P::Err> {
        let logger = skel.logger.push_mark("init")?;
        logger
            .result(self.driver_skel.status_tx.send(DriverStatus::Init).await)
            .unwrap_or_default();

        let point = self.star_skel.point.clone();
        let registration = Registration {
            point: point.clone(),
            kind: Kind::Star(self.star_skel.kind.clone()),
            registry: Default::default(),
            properties: Default::default(),
            owner: HYPERUSER.clone(),
            strategy: Strategy::Override,
            status: Status::Ready,
        };

        self.star_skel.api.create_states(point.clone()).await?;
        self.star_skel.registry.register(&registration).await?;
        let location = ParticleLocation::new(self.star_skel.point.clone(), None);
        self.star_skel.registry.assign(&point, location).await?;

        logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();

        Ok(())
    }

    async fn item(&self, point: &Point) -> Result<ItemSphere<P>, P::Err> {
        Ok(ItemSphere::Handler(Box::new(Star::restore(
            self.star_skel.clone(),
            self.ctx.clone(),
            (),
        ))))
    }
}

#[derive(DirectedHandler)]
pub struct Star<P>
where
    P: Cosmos + 'static,
{
    pub skel: HyperStarSkel<P>,
    pub ctx: DriverCtx,
}

impl<P> Star<P>
where
    P: Cosmos,
{
    async fn create(&self, assign: &Assign) -> Result<(), P::Err> {
        self.skel
            .state
            .create_shell(assign.details.stub.point.clone());

        /*
        self.skel
            .logger
            .result(self.skel.drivers.assign(assign.clone()).await)?;

         */

        Ok(())
    }
}

#[async_trait]
impl<P> ItemHandler<P> for Star<P>
where
    P: Cosmos,
{
    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        <Star<P> as Item<P>>::bind(self).await
    }

    async fn init(&self) -> Result<Status, UniErr> {
        match self.skel.kind {
            StarSub::Central => {
                let registration = Registration {
                    point: Point::root(),
                    kind: Kind::Root,
                    registry: Default::default(),
                    properties: Default::default(),
                    owner: HYPERUSER.clone(),
                    strategy: Strategy::Ensure,
                    status: Status::Ready,
                };
                self.skel
                    .registry
                    .register(&registration)
                    .await
                    .map_err(|e| e.to_uni_err())?;

                let record = self
                    .skel
                    .registry
                    .record(&Point::root())
                    .await
                    .map_err(|e| e.to_uni_err())?;
                let assign = Assign::new(AssignmentKind::Create, record.details, StateSrc::None);
                self.create(&assign).await.map_err(|e| e.to_uni_err())?;
                let location = ParticleLocation::new(self.skel.point.clone(), None);
                self.skel
                    .registry
                    .assign(&Point::root(), location)
                    .await
                    .map_err(|e| e.to_uni_err())?;

                let registration = Registration {
                    point: Point::global_executor(),
                    kind: Kind::Global,
                    registry: Default::default(),
                    properties: Default::default(),
                    owner: HYPERUSER.clone(),
                    strategy: Strategy::Ensure,
                    status: Status::Ready,
                };
                self.skel
                    .registry
                    .register(&registration)
                    .await
                    .map_err(|e| e.to_uni_err())?;

                let record = self
                    .skel
                    .registry
                    .record(&Point::global_executor())
                    .await
                    .map_err(|e| e.to_uni_err())?;
                let assign = Assign::new(AssignmentKind::Create, record.details, StateSrc::None);
                self.create(&assign).await.map_err(|e| e.to_uni_err())?;
                let location = ParticleLocation::new(LOCAL_STAR.clone(), None);
                self.skel
                    .registry
                    .assign(&Point::global_executor(), location)
                    .await
                    .map_err(|e| e.to_uni_err())?;

                Ok(Status::Ready)
            }
            _ => Ok(Status::Ready),
        }
    }
}

#[async_trait]
impl<P> Item<P> for Star<P>
where
    P: Cosmos + 'static,
{
    type Skel = HyperStarSkel<P>;
    type Ctx = DriverCtx;
    type State = ();

    fn restore(skel: Self::Skel, ctx: Self::Ctx, _: Self::State) -> Self {
        Star { skel, ctx }
    }

    async fn bind(&self) -> Result<ArtRef<BindConfig>, P::Err> {
        Ok(STAR_BIND_CONFIG.clone())
    }
}

#[handler]
impl<P> Star<P>
where
    P: Cosmos,
{
    #[route("Hyp<Provision>")]
    pub async fn provision(
        &self,
        ctx: InCtx<'_, HyperSubstance>,
    ) -> Result<ParticleLocation, P::Err> {
        if let HyperSubstance::Provision(provision) = ctx.input {
            println!("\tprovisioning : {}", provision.point.to_string());
            let record = self.skel.registry.record(&provision.point).await?;
            match self.skel.wrangles.find(&record.details.stub.kind) {
                None => {
println!("\n{} found no provisioning wrangles for {}", self.skel.kind.to_string(), record.details.stub.kind.to_string() );
                    let kind = record.details.stub.kind.clone();
                    if self
                        .skel
                        .drivers
                        .find_external(record.details.stub.kind.clone())
                        .await?
                        .is_some()
                    {
                        println!("\trequesting assignment...");
                        let assign = HyperSubstance::Assign(Assign::new(
                            AssignmentKind::Create,
                            record.details,
                            provision.state.clone(),
                        ));

                        let ctx: InCtx<'_, HyperSubstance> = ctx.push_input_ref(&assign);
                        if self.assign(ctx).await?.is_ok() {
                            Ok(ParticleLocation::new(self.skel.point.clone(), None))
                        } else {
                            Err(
                                format!("could not find assign kind {} to self", kind.to_string())
                                    .into(),
                            )
                        }
                    } else {
                        println!("could not find a place to provision!!!");
                        Err(format!(
                            "could not find a place to provision kind {}",
                            kind.to_string()
                        )
                        .into())
                    }
                }
                Some(selector) => {
                    // hate using a write lock for this...
                    let mut selector = selector.write().await;
                    let key = selector.wrangle().await?;
                    let assign =
                        Assign::new(AssignmentKind::Create, record.details, StateSrc::None);
                    let assign: DirectedCore = assign.into();
                    let mut proto = DirectedProto::ping();
                    proto.core(assign);
                    proto.to(key.to_surface());
println!("\tsending assign request to {}", key.to_surface().to_string() );
                    proto.track = true;
                    let pong: Wave<Pong> = ctx.transmitter.direct(proto).await?;
                    pong.ok_or()?;
println!("\tassignment success!");
                    Ok(ParticleLocation::new(key.to_point().into(), None))
                }
            }
        } else {
            Err("expected Hyp<Provision>".into())
        }
    }

    #[route("Hyp<Assign>")]
    pub async fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<ReflectedCore, P::Err> {
        if let HyperSubstance::Assign(assign) = ctx.input {
            println!(
                "\tassigning to star: {}",
                assign.details.stub.point.to_string()
            );
            #[cfg(test)]
            self.skel
                .diagnostic_interceptors
                .assignment
                .send(assign.clone())
                .unwrap_or_default();

            if self
                .skel
                .drivers
                .find(assign.details.stub.kind.clone())
                .await?
                .is_some()
            {
                self.create(assign).await;

                let driver = self
                    .skel
                    .drivers
                    .local_driver_lookup(assign.details.stub.kind.clone())
                    .await?
                    .ok_or(P::Err::new(format!(
                        "Star does not have  driver for {}",
                        assign.details.stub.kind.to_string()
                    )))?;

                let mut directed = DirectedProto::ping();
                directed.method(HypMethod::Assign);
                directed.from(self.skel.point.to_surface());
                directed.to(driver.to_surface());
println!("\tassign to driver: {}", driver.to_surface().to_string());
                directed.body(HyperSubstance::Assign(assign.clone()).into());
                directed.track = ctx.wave().track();
                let pong: Wave<Pong> = ctx.transmitter.direct(directed).await?;
                pong.ok_or()?;
            } else {
                self.skel.logger.result::<(),UniErr>(
                    Err(UniErr::from_500(format!("Star {} does not have a driver for kind: {}",
                        self.skel.kind.to_string(),
                    assign.details.stub.kind.to_string())).into())
                )?;
            }

            let location = ParticleLocation::new(self.skel.point.clone(), None);
            self.skel
                .registry
                .assign(&assign.details.stub.point, location)
                .await?;

            Ok(ReflectedCore::ok())
        } else {
            Err("expected Hyp<Assign>".into())
        }
    }

    #[route("Hyp<Transport>")]
    pub async fn transport(&self, ctx: InCtx<'_, UltraWave>) {
        self.skel.logger.track(ctx.wave(), || {
            Tracker::new("star:core:transport", "Receive")
        });

        let wave = ctx.input.clone();

        self.skel.logger.track(&wave, || {
            Tracker::new("star:core:transport", "Unwrapped")
        });

//        self.skel.gravity_router.route(wave).await;

        let mut injection = TraversalInjection::new(
            self.skel
                .point
                .clone()
                .to_surface()
                .with_layer(Layer::Gravity),
            wave,
        );
        injection.from_gravity = true;

        self.skel.inject_tx.send(injection).await;

    }

    #[route("Hyp<Search>")]
    pub async fn handle_search_request(&self, ctx: InCtx<'_, HyperSubstance>) -> CoreBounce {
        async fn sub_search_and_reflect<'a, E>(
            star: &Star<E>,
            ctx: &'a InCtx<'a, HyperSubstance>,
            mut history: HashSet<Point>,
            search: Search,
        ) -> Result<ReflectedCore, UniErr>
        where
            E: Cosmos,
        {
            let mut discoveries = if star.skel.kind.is_forwarder() {
                let mut wrangler = Wrangler::new(star.skel.clone(), search);
                history.insert(star.skel.point.clone());
                wrangler.history(history);
                wrangler.wrangle(false).await?
            } else {
                // if not a forwarder, then we don't seek sub wrangles
                Discoveries::new()
            };

            if star.skel.kind.can_be_wrangled() {
                let discovery = Discovery {
                    star_kind: star.skel.kind.clone(),
                    hops: ctx.wave().hops(),
                    star_key: star.skel.key.clone(),
                    kinds: star
                        .skel
                        .drivers
                        .external_kinds()
                        .await?
                        .into_iter()
                        .collect(),
                };
                discoveries.push(discovery);
            }

            let mut core = ReflectedCore::new();
            core.body = Substance::Hyper(HyperSubstance::Discoveries(discoveries));
            core.status = StatusCode::from_u16(200).unwrap();
            Ok(core)
        }

        if let HyperSubstance::Search(search) = ctx.input {
            match search {
                Search::Star(star) => {
                    if self.skel.key == *star {
                        match self.skel.drivers.internal_kinds().await {
                            Ok(kinds) => {
                                let discovery = Discovery {
                                    star_kind: self.skel.kind.clone(),
                                    hops: ctx.wave().hops(),
                                    star_key: self.skel.key.clone(),
                                    kinds: kinds.into_iter().collect(),
                                };
                                let mut discoveries = Discoveries::new();
                                discoveries.push(discovery);

                                let mut core = ReflectedCore::new();
                                core.body =
                                    Substance::Hyper(HyperSubstance::Discoveries(discoveries));
                                core.status = StatusCode::from_u16(200).unwrap();
                                return CoreBounce::Reflected(core);
                            }
                            Err(err) => {
                                return CoreBounce::Reflected(err.as_reflected_core());
                            }
                        }
                    } else {
                        return CoreBounce::Reflected(ReflectedCore::result(
                            sub_search_and_reflect(
                                self,
                                &ctx,
                                ctx.wave().history(),
                                search.clone(),
                            )
                            .await,
                        ));
                    }
                }
                Search::StarKind(kind) => if *kind == self.skel.kind {},
                Search::Kinds => {
                    return CoreBounce::Reflected(ReflectedCore::result(
                        sub_search_and_reflect(self, &ctx, ctx.wave().history(), Search::Kinds)
                            .await,
                    ));
                }
            }
            return CoreBounce::Absorbed;
        } else {
            self.skel
                .logger
                .error(format!("expected Search got : {}", ctx.input.to_string()));
            return CoreBounce::Reflected(ctx.bad_request());
        }
    }
}

#[derive(Clone)]
pub struct StarWrangles {
    pub wrangles: Arc<DashMap<KindSelector, Arc<RwLock<RoundRobinWrangleSelector>>>>,
}

impl StarWrangles {
    pub fn new() -> Self {
        Self {
            wrangles: Arc::new(DashMap::new()),
        }
    }

    pub fn find(&self, kind: &Kind) -> Option<Arc<RwLock<RoundRobinWrangleSelector>>> {
        let mut iter = self.wrangles.iter();
        while let Some(multi) = iter.next() {
            if multi.key().matches(&kind) {
                return Some(multi.value().clone());
            }
        }
        return None;
    }

    pub async fn add(&self, discoveries: Vec<StarDiscovery>) {
        for discovery in discoveries {
            for kind in discovery.kinds.clone() {
                match self.wrangles.get_mut(&kind) {
                    None => {
                        let mut wrangler = RoundRobinWrangleSelector::new(kind.clone());
                        wrangler.stars.push(discovery.clone());
                        wrangler.sort();
                        let mut wrangler = Arc::new(RwLock::new(wrangler));
                        self.wrangles.insert(kind, wrangler);
                    }
                    Some(mut wrangler) => {
                        let mut wrangler = wrangler.value_mut();
                        let mut wrangler = wrangler.write().await;
                        wrangler.stars.push(discovery.clone());
                        wrangler.sort();
                    }
                }
            }
        }
    }

    pub fn verify(&self, kinds: &[&Kind]) -> Result<(), UniErr> {
        for kind in kinds {
            if self.find(*kind).is_none() {
                return Err(format!(
                    "star must be able to wrangle at least one {}",
                    kind.to_string()
                )
                .into());
            }
        }
        Ok(())
    }

    pub async fn wrangle(&self, kind: &Kind) -> Result<StarKey, UniErr> {
        self.find(kind)
            .ok_or(format!(
                "could not find wrangles for kind {}",
                kind.to_string()
            ))?
            .write()
            .await
            .wrangle()
            .await
    }
}

pub struct RoundRobinWrangleSelector {
    pub kind: KindSelector,
    pub stars: Vec<StarDiscovery>,
    pub index: usize,
    pub step_index: usize,
}

impl RoundRobinWrangleSelector {
    pub fn new(kind: KindSelector) -> Self {
        Self {
            kind,
            stars: vec![],
            index: 0,
            step_index: 0,
        }
    }

    pub fn sort(&mut self) {
        self.stars.sort();
        self.step_index = 0;
        let mut hops: i32 = -1;
        for discovery in &self.stars {
            if hops < 0 {
                hops = discovery.hops as i32;
            } else if discovery.hops as i32 > hops {
                break;
            }
            self.step_index += 1;
        }
    }

    pub async fn wrangle(&mut self) -> Result<StarKey, UniErr> {
        if self.stars.is_empty() {
            return Err(format!("cannot find wrangle for kind: {}", self.kind.to_string()).into());
        }

        self.index = self.index + 1;

        let index = self.index % self.step_index;

        if let Some(discovery) = self.stars.get(index) {
            Ok(discovery.discovery.star_key.clone())
        } else {
            Err(format!("cannot find wrangle for kind: {}", self.kind.to_string()).into())
        }
    }
}

pub struct Wrangler<P>
where
    P: Cosmos,
{
    pub skel: HyperStarSkel<P>,
    pub transmitter: ProtoTransmitter,
    pub history: HashSet<Point>,
    pub search: Search,
}

impl<P> Wrangler<P>
where
    P: Cosmos,
{
    pub fn new(skel: HyperStarSkel<P>, search: Search) -> Self {
        let router = LayerInjectionRouter::new(
            skel.clone(),
            skel.point.to_surface().with_layer(Layer::Shell),
        );
        let mut transmitter =
            ProtoTransmitterBuilder::new(Arc::new(router), skel.exchanger.clone());
        transmitter.from = SetStrategy::Override(skel.point.to_surface().with_layer(Layer::Core));
        transmitter.agent = SetStrategy::Override(Agent::HyperUser);
        transmitter.handling = SetStrategy::Override(Handling {
            kind: HandlingKind::Immediate,
            priority: Priority::Hyper,
            retries: Retries::Max,
            wait: WaitTime::High,
        });
        let transmitter = transmitter.build();
        Self {
            skel,
            transmitter,
            history: HashSet::new(),
            search,
        }
    }

    pub fn history(&mut self, mut history: HashSet<Point>) {
        for point in history {
            self.history.insert(point);
        }
    }

    pub async fn wrangle(&self, track: bool) -> Result<Discoveries, UniErr> {
        let mut ripple = DirectedProto::ripple();
        ripple.track = track;
        ripple.method(HypMethod::Search);
        ripple.body(Substance::Hyper(HyperSubstance::Search(
            self.search.clone(),
        )));
        ripple.history(self.history.clone());
        let mut adjacents = self.skel.adjacents.clone();
        adjacents.retain(|point, _| !self.history.contains(point));
        if adjacents.is_empty() {
            return Ok(Discoveries::new());
        }
        ripple.bounce_backs = Some(BounceBacks::Count(adjacents.len()));
        ripple.to(Recipients::Stars);
        let echoes: Echoes = self.transmitter.direct(ripple).await?;
        let mut discoveries = Discoveries::new();
        for echo in echoes {
            if echo.core.status.is_success() {
                if let Substance::Hyper(HyperSubstance::Discoveries(new)) = echo.variant.core.body {
                    for discovery in new.vec.into_iter() {
                        discoveries.push(discovery);
                    }
                } else {
                    self.skel.logger.warn(format!(
                        "unexpected reflected core substance from search echo : {}",
                        echo.core.body.kind().to_string()
                    ));
                }
            } else {
                self.skel.logger.error(format!(
                    "search echo failure {}",
                    echo.core.to_err().to_string()
                ));
            }
        }
        Ok(discoveries)
    }
}
