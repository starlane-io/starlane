use cosmic_universe::particle::Details;
use std::collections::HashMap;
use std::marker::PhantomData;
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::HyperSubstance;
use cosmic_universe::loc::{Layer, Point, ToSurface};
use cosmic_universe::log::{LogSource, NoAppender, PointLogger, RootLogger};
use cosmic_universe::wave::exchange::synch::{DirectedHandler, DirectedHandlerProxy, DirectedHandlerShell, ExchangeRouter, InCtx, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx};
use std::sync::Arc;
use cosmic_universe::wave::{Agent, DirectedWave, ReflectedAggregate, UltraWave};
use cosmic_universe::wave::core::CoreBounce;
use cosmic_universe::wave::exchange::SetStrategy;
use crate::MechtronFactories;
use crate::err::GuestErr;
use crate::err::MechErr;

pub struct MechtronRouter<G> where G: crate::Guest {
    phantom: PhantomData<G>
}

impl <G> MechtronRouter<G> where G: crate::Guest {
    pub fn new() -> Self { Self {
        phantom: PhantomData::default()
    } }
}


impl <G> ExchangeRouter for MechtronRouter<G> where G: crate::Guest {
    fn route(&self, wave: UltraWave) {
        crate::membrane::mechtron_exchange_wave_host::<G>(wave);
    }

    fn exchange(&self, direct: DirectedWave) -> Result<ReflectedAggregate, UniErr> {
        crate::membrane::mechtron_exchange_wave_host::<G>(direct.to_ultra()).map_err(|e|e.to_uni_err())
    }
}

pub struct Guest {
    details: Details,
    mechtrons: HashMap<Point, Details>,
    factories: MechtronFactories,
    logger: PointLogger,
}

impl crate::Guest for Guest {
    type Err = GuestErr;
}

impl Guest {
    pub fn new(details: Details, factories: MechtronFactories) -> Self {
        let root_logger = RootLogger::new(LogSource::Core, Arc::new(NoAppender::new()));
        let logger = root_logger.point(details.stub.point.clone());

        Self {
            details,
            mechtrons: HashMap::new(),
            factories,
            logger,
        }
    }

    pub fn handler(&self) -> DirectedHandlerShell<DirectedHandlerProxy> {
        let surface = self.details.stub.point.to_surface().with_layer(Layer::Core);
        let router: MechtronRouter<Self> = MechtronRouter::new();
        let router = Arc::new(router);
        let mut transmitter = ProtoTransmitterBuilder::new(router);
        transmitter.from = SetStrategy::Override(surface.clone());
        transmitter.agent = SetStrategy::Override(Agent::Point(surface.point.clone()));
        DirectedHandlerShell::new( DirectedHandlerProxy::new( GuestHandler::new() ), transmitter, self.details.stub.point.to_surface(), self.logger.logger.clone() )
    }
}

pub struct GuestHandler;

impl GuestHandler {
    pub fn new() -> Self {
        Self {}
    }
}

impl DirectedHandler for GuestHandler {
    fn handle(&self, ctx: RootInCtx) -> CoreBounce {
        CoreBounce::Absorbed
    }
}

/*
#[handler]
impl GuestHandler {
    #[route("Hyp<Assign>")]
    pub fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), UniErr> {
        if let HyperSubstance::Assign(assign) = ctx.input {
            Ok(())
        } else {
            Err("expecting Assign".into())
        }
    }
}

 */
