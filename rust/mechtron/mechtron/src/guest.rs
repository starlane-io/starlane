use crate::err::GuestErr;
use crate::err::MechErr;
use crate::{MechtronFactories, Platform};
use cosmic_macros::handler_sync;
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::HyperSubstance;
use cosmic_universe::loc::{Layer, Point, ToSurface};
use cosmic_universe::log::{
    LogSource, NoAppender, PointLogger, RootLogger, SynchTransmittingLogAppender,
};
use cosmic_universe::particle::Details;
use cosmic_universe::wave::core::CoreBounce;
use cosmic_universe::wave::exchange::synch::{
    DirectedHandler, DirectedHandlerProxy, DirectedHandlerShell, ExchangeRouter, InCtx,
    ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx,
};
use cosmic_universe::wave::exchange::SetStrategy;
use cosmic_universe::wave::{Agent, DirectedWave, ReflectedAggregate, ToRecipients, UltraWave};
use dashmap::DashMap;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;

#[derive(Clone)]
pub struct GuestSkel<P>
where
    P: Platform,
{
    details: Details,
    mechtrons: Arc<DashMap<Point, Details>>,
    factories: Arc<MechtronFactories<P>>,
    logger: PointLogger,
    platform: P,
}

impl<P> GuestSkel<P>
where
    P: Platform,
{
    pub fn new(
        details: Details,
        factories: Arc<MechtronFactories<P>>,
        logger: PointLogger,
        platform: P,
    ) -> Self {
        let mechtrons = Arc::new(DashMap::new());
        Self {
            details,
            mechtrons,
            factories,
            logger,
            platform,
        }
    }
}

#[derive(Clone)]
pub struct GuestCtx {
    pub transmitter: ProtoTransmitterBuilder,
}

impl GuestCtx {
    pub fn new(transmitter: ProtoTransmitterBuilder) -> Self {
        Self { transmitter }
    }

    pub fn builder(&self) -> ProtoTransmitterBuilder {
        self.transmitter.clone()
    }

    pub fn transmitter(&self) -> ProtoTransmitter {
        self.transmitter.clone().build()
    }
}

pub struct Guest<P>
where
    P: Platform + 'static,
{
    skel: GuestSkel<P>,
    ctx: GuestCtx,
}

impl<P> Guest<P>
where
    P: Platform + 'static,
{
    pub fn new(details: Details, platform: P) -> Result<Self, GuestErr>
    where
        P: Sized,
        GuestErr: From<<P as Platform>::Err>,
    {
        let router: GuestRouter<P> = GuestRouter::new();
        let router = Arc::new(router);
        let mut transmitter = ProtoTransmitterBuilder::new(router);
        transmitter.agent = SetStrategy::Override(Agent::Point(details.stub.point.clone()));
        transmitter.from = SetStrategy::Override(details.stub.point.clone().to_surface());
        transmitter.to = SetStrategy::Override(Point::global_logger().to_surface().to_recipients());
        let appender = SynchTransmittingLogAppender::new(transmitter.clone());
        let root_logger = RootLogger::new(LogSource::Core, Arc::new(appender));
        let logger = root_logger.point(details.stub.point.clone());
        logger.info("Guest created");

        let ctx = GuestCtx::new(transmitter);

        let factories = Arc::new(platform.factories()?);

        let skel = GuestSkel::new(details, factories, logger, platform);

        Ok(Self { skel, ctx })
    }
}

impl<G> crate::Guest for Guest<G>
where
    G: Platform,
{
    fn handler(&self) -> DirectedHandlerShell<DirectedHandlerProxy> {
        DirectedHandlerShell::new(
            DirectedHandlerProxy::new(GuestHandler::new(self.skel.clone(), self.ctx.clone())),
            self.ctx.builder(),
            self.skel.details.stub.point.to_surface(),
            self.skel.logger.logger.clone(),
        )
    }
}

pub struct GuestRouter<P>
where
    P: crate::Platform,
{
    phantom: PhantomData<P>,
}

impl<P> GuestRouter<P>
where
    P: crate::Platform,
{
    pub fn new() -> Self {
        Self {
            phantom: PhantomData::default(),
        }
    }
}

impl<P> ExchangeRouter for GuestRouter<P>
where
    P: crate::Platform,
{
    fn route(&self, wave: UltraWave) {
        crate::membrane::mechtron_exchange_wave_host::<P>(wave);
    }

    fn exchange(&self, direct: DirectedWave) -> Result<ReflectedAggregate, UniErr> {
        crate::membrane::mechtron_exchange_wave_host::<P>(direct.to_ultra())
            .map_err(|e| e.to_uni_err())
    }
}

pub struct GuestHandler<P>
where
    P: Platform,
{
    skel: GuestSkel<P>,
    ctx: GuestCtx,
}

impl<P> GuestHandler<P>
where
    P: Platform,
{
    pub fn new(skel: GuestSkel<P>, ctx: GuestCtx) -> Self {
        Self { skel, ctx }
    }
}

#[handler_sync]
impl<P> GuestHandler<P>
where
    P: Platform,
{
    #[route("Hyp<Host>")]
    pub fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), UniErr> {
        if let HyperSubstance::Host(host) = ctx.input {
            self.skel.logger.info("Received Host command!");
            Ok(())
        } else {
            Err("expecting Host ".into())
        }
    }
}
