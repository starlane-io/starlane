use crate::driver::{
    Driver, DriverAvail, DriverCtx, DriverErr, DriverSkel, DriverStatus, HyperDriverFactory,
    Particle, ParticleSphere, ParticleSphereInner, ParticleStarErr,
};
use crate::base::Platform;
use crate::registry::Registration;
use crate::star::{HyperStarSkel, LayerInjectionRouter, StarErr};
use async_trait::async_trait;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use starlane_macros::push_mark;
use starlane_macros::{handler, route, DirectedHandler};
use starlane_space::artifact::ArtRef;
use starlane_space::command::common::StateSrc;
use starlane_space::command::direct::create::Strategy;
use starlane_space::config::bind::BindConfig;
use starlane_space::err::{CoreReflector, SpaceErr};
use starlane_space::hyper::{
    Assign, AssignmentKind, Discoveries, Discovery, HyperSubstance, HyperSubstanceKind,
    ParticleLocation, Search,
};
use starlane_space::kind::{BaseKind, Kind, StarSub};
use starlane_space::loc::{Layer, StarKey, ToPoint, ToSurface, LOCAL_STAR};
use starlane_space::log::{Trackable, Tracker};
use starlane_space::parse::bind_config;
use starlane_space::parse::util::{parse_errs, result};
use starlane_space::particle::traversal::TraversalInjection;
use starlane_space::particle::Status;
use starlane_space::point::Point;
use starlane_space::selector::{KindBaseSelector, KindSelector, Pattern, SubKindSelector};
use starlane_space::substance::{Substance, SubstanceKind};
use starlane_space::util::{log, ValueMatcher, ValuePattern};
use starlane_space::wave::core::http2::StatusCode;
use starlane_space::wave::core::hyper::HypMethod;
use starlane_space::wave::core::MethodKind::Hyp;
use starlane_space::wave::core::{CoreBounce, DirectedCore, ReflectedCore};
use starlane_space::wave::exchange::asynch::{InCtx, ProtoTransmitter, ProtoTransmitterBuilder};
use starlane_space::wave::exchange::SetStrategy;
use starlane_space::wave::{
    Agent, BounceBacks, DirectedProto, Echoes, Handling, HandlingKind, PongCore, Priority,
    Recipients, Retries, WaitTime, Wave, WaveVariantDef,
};
use starlane_space::HYPERUSER;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::RwLock;

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
pub struct StarDriverFactory {
    pub kind: Kind,
    pub selector: KindSelector,
}

impl StarDriverFactory {
    pub fn new(kind: StarSub) -> Self {
        let selector = KindSelector {
            base: KindBaseSelector::Exact(BaseKind::Star),
            sub: SubKindSelector::Exact(kind.to_camel_case()),
            specific: ValuePattern::Always,
        };
        let kind = Kind::Star(kind);
        Self { kind, selector }
    }
}

#[async_trait]
impl HyperDriverFactory for StarDriverFactory {
    fn kind(&self) -> Kind {
        self.kind.clone()
    }

    fn selector(&self) -> KindSelector {
        self.selector.clone()
    }

    fn avail(&self) -> DriverAvail {
        DriverAvail::Internal
    }

    async fn create(
        &self,
        star: HyperStarSkel,
        skel: DriverSkel,
        ctx: DriverCtx,
    ) -> Result<Box<dyn Driver>, DriverErr> {
        Ok(Box::new(StarDriver::new(star, skel, ctx)))
    }
}

pub struct StarDriver {
    pub star_skel: HyperStarSkel,
    pub driver_skel: DriverSkel,
    pub ctx: DriverCtx,
}

impl StarDriver {
    pub fn new(star_skel: HyperStarSkel, driver_skel: DriverSkel, ctx: DriverCtx) -> Self {
        Self {
            star_skel,
            driver_skel,
            ctx,
        }
    }
}

#[async_trait]
impl Driver for StarDriver {
    fn kind(&self) -> Kind {
        Kind::Star(self.star_skel.kind.clone())
    }

    async fn init(&mut self, skel: DriverSkel, _: DriverCtx) -> Result<(), DriverErr> {
        let logger = push_mark!(skel.logger);
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
            strategy: Strategy::Ensure,
            status: Status::Ready,
        };

        self.star_skel.api.create_states(point.clone()).await?;
        self.star_skel.registry.register(&registration).await?;
        self.star_skel
            .registry
            .assign_star(&point, &self.star_skel.point)
            .await?;

        logger
            .result(skel.status_tx.send(DriverStatus::Ready).await)
            .unwrap_or_default();

        Ok(())
    }

    async fn particle(&self, point: &Point) -> Result<ParticleSphere, DriverErr> {
        let star = Star::restore(self.star_skel.clone(), self.ctx.clone(), ());
        Ok(star.sphere()?)
    }
}

#[derive(DirectedHandler)]
pub struct Star {
    pub skel: HyperStarSkel,
    pub ctx: DriverCtx,
}

impl Star {
    async fn create(&self, assign: &Assign) -> Result<(), <Self as Particle>::Err> {
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

impl Star {}

#[async_trait]
impl Particle for Star {
    type Skel = HyperStarSkel;
    type Ctx = DriverCtx;
    type State = ();
    type Err = ParticleStarErr;

    fn restore(skel: Self::Skel, ctx: Self::Ctx, _: Self::State) -> Self {
        Star { skel, ctx }
    }

    fn sphere(self) -> Result<ParticleSphere, Self::Err> {
        Ok(ParticleSphere::new_handler(self))
    }
}

#[handler]
impl Star {
    #[route("Hyp<Init>")]
    pub async fn init(
        &self,
        ctx: InCtx<'_, HyperSubstance>,
    ) -> Result<Status, <Self as Particle>::Err> {
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
                self.skel.registry.register(&registration).await?;

                let record = self.skel.registry.record(&Point::root()).await?;
                let assign = Assign::new(AssignmentKind::Create, record.details, StateSrc::None);
                self.create(&assign).await?;
                self.skel
                    .registry
                    .assign_star(&Point::root(), &self.skel.point)
                    .await?;

                let registration = Registration {
                    point: Point::global_executor(),
                    kind: Kind::Global,
                    registry: Default::default(),
                    properties: Default::default(),
                    owner: HYPERUSER.clone(),
                    strategy: Strategy::Ensure,
                    status: Status::Ready,
                };
                self.skel.registry.register(&registration).await?;

                let record = self.skel.registry.record(&Point::global_executor()).await?;
                let assign = Assign::new(AssignmentKind::Create, record.details, StateSrc::None);
                self.create(&assign).await?;
                self.skel
                    .registry
                    .assign_star(&Point::global_executor(), &LOCAL_STAR)
                    .await?;

                Ok(Status::Ready)
            }
            _ => Ok(Status::Ready),
        }
    }

    #[route("Hyp<Provision>")]
    pub async fn provision(
        &self,
        ctx: InCtx<'_, HyperSubstance>,
    ) -> Result<ParticleLocation, <Self as Particle>::Err> {
        if let HyperSubstance::Provision(provision) = ctx.input {
            let record = self.skel.registry.record(&provision.point).await?;

            match self.skel.wrangles.find(&record.details.stub.kind) {
                None => {
                    let kind = record.details.stub.kind.clone();
                    if self
                        .skel
                        .drivers
                        .find_external(record.details.stub.kind.clone())
                        .await?
                        .is_some()
                    {
                        let assign = HyperSubstance::Assign(Assign::new(
                            AssignmentKind::Create,
                            record.details,
                            provision.state.clone(),
                        ));

                        let ctx: InCtx<'_, HyperSubstance> = ctx.push_input_ref(&assign);
                        if self.assign(ctx).await?.is_ok() {
                            Ok(ParticleLocation::new(Some(self.skel.point.clone()), None))
                        } else {
                            Err(StarErr::CouldNotAssignToSelf(kind))?
                        }
                    } else {
                        Err(StarErr::CouldNotFindHostToProvision(kind))?
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
                    let pong: WaveVariantDef<PongCore> = ctx.transmitter.direct(proto).await?;
                    pong.ok_or()?;
                    Ok(ParticleLocation::new(key.to_point().into(), None))
                }
            }
        } else {
            Err(SpaceErr::expected_substance(
                SubstanceKind::Hyper(HyperSubstanceKind::Provision),
                ctx.input.kind().into(),
            ))?
        }
    }

    #[route("Hyp<Assign>")]
    pub async fn assign(
        &self,
        ctx: InCtx<'_, HyperSubstance>,
    ) -> Result<ReflectedCore, <Self as Particle>::Err> {
        if let HyperSubstance::Assign(assign) = ctx.input {
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
                    .ok_or(DriverErr::driver_not_found(&assign.details.stub.kind))?;

                let mut directed = DirectedProto::ping();
                directed.method(HypMethod::Assign);
                directed.from(self.skel.point.to_surface());
                directed.to(driver.to_surface());
                directed.body(HyperSubstance::Assign(assign.clone()).into());
                directed.track = ctx.wave().track();
                let pong: WaveVariantDef<PongCore> = ctx.transmitter.direct(directed).await?;

                self.skel.logger.result(pong.ok_or())?;
            } else {
                self.skel
                    .logger
                    .result::<(), SpaceErr>(Err(SpaceErr::server_error(format!(
                        "Star {} does not have a driver for kind: {}",
                        self.skel.kind.to_string(),
                        assign.details.stub.kind.to_string()
                    ))
                    .into()))?;
            }

            self.skel
                .registry
                .assign_star(&assign.details.stub.point, &self.skel.point)
                .await?;

            Ok(ReflectedCore::ok())
        } else {
            Err(SpaceErr::expected_substance(
                SubstanceKind::Hyper(HyperSubstanceKind::Assign),
                ctx.input.kind().into(),
            ))?
        }
    }

    #[route("Hyp<Transport>")]
    pub async fn transport(&self, ctx: InCtx<'_, Wave>) {
        self.skel.logger.track(ctx.wave(), || {
            Tracker::new("star:core:transport", "Receive")
        });

        let wave = ctx.input.clone();

        self.skel
            .logger
            .track(&wave, || Tracker::new("star:core:transport", "Unwrapped"));

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
        async fn sub_search_and_reflect<'a>(
            star: &Star,
            ctx: &'a InCtx<'a, HyperSubstance>,
            mut history: HashSet<Point>,
            search: Search,
        ) -> Result<ReflectedCore, SpaceErr> {
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
                        match self.skel.drivers.external_kinds().await {
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
            if multi.key().is_match(&kind).is_ok() {
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

    pub fn verify(&self, kinds: &[&Kind]) -> Result<(), SpaceErr> {
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

    pub async fn wrangle(&self, kind: &Kind) -> Result<StarKey, SpaceErr> {
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

    pub async fn wrangle(&mut self) -> Result<StarKey, SpaceErr> {
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

pub struct Wrangler {
    pub skel: HyperStarSkel,
    pub transmitter: ProtoTransmitter,
    pub history: HashSet<Point>,
    pub search: Search,
}

impl Wrangler {
    pub fn new(skel: HyperStarSkel, search: Search) -> Self {
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

    pub async fn wrangle(&self, track: bool) -> Result<Discoveries, SpaceErr> {
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
                    // this is not good, but it's not breaking anything, and I cant deal with all the errors right now -- Scott

                    if let Substance::Hyper(sub) = echo.variant.core.body {
                        self.skel.logger.warn(format!(
                            "unexpected reflected core substance from search echo : {}",
                            sub.to_string()
                        ));
                    } else {
                        self.skel.logger.warn(format!(
                            "unexpected reflected core substance from search echo : {}",
                            echo.core.body.kind().to_string()
                        ));
                    }
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
