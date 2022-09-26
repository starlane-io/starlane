use crate::err::GuestErr;
use crate::err::MechErr;
use crate::{MechtronFactories, MechtronSkel, Platform};
use cosmic_macros::handler_sync;
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::HyperSubstance;
use cosmic_universe::kind::Kind::Mechtron;
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
    P: Platform +'static,
{
    details: Details,
    mechtrons: Arc<DashMap<Point, HostedMechtron>>,
    factories: Arc<MechtronFactories<P>>,
    logger: PointLogger,
    transmitter: ProtoTransmitterBuilder,
    platform: P,
}

impl<P> GuestSkel<P>
where
    P: Platform + 'static,
{
    pub fn new(
        details: Details,
        factories: Arc<MechtronFactories<P>>,
        platform: P,
    ) -> Self {
        let router: GuestRouter<P> = GuestRouter::new();
        let router = Arc::new(router);
        let mut transmitter = ProtoTransmitterBuilder::new(router);
        transmitter.agent = SetStrategy::Override(Agent::Point(details.stub.point.clone()));
        transmitter.from = SetStrategy::Override(details.stub.point.clone().to_surface());
        transmitter.to = SetStrategy::Override(Point::global_logger().to_surface().to_recipients());
        let appender = SynchTransmittingLogAppender::new(transmitter.clone());
        let root_logger = RootLogger::new(LogSource::Core, Arc::new(appender));
        let logger = root_logger.point(details.stub.point.clone());

        let mechtrons = Arc::new(DashMap::new());
        Self {
            details,
            mechtrons,
            factories,
            logger,
            platform,
            transmitter,
        }
    }

    fn hosted( &self, point: &Point) -> Result<HostedMechtron,GuestErr> {
         Ok(self
            .mechtrons
            .get(point)
            .ok_or::<GuestErr>(
                format!(
                    "mechtron associated with point: {} is not hosted by guest {}",
                    point.to_string(),
                    self.details.stub.point.to_string()
                )
                .into(),
            )?
            .value()
            .clone())

    }

    fn mechtron_skel(&self, point: &Point) -> Result<MechtronSkel<P>, GuestErr> {

        let hosted = self.hosted(point)?;
        let mut transmitter = self.builder();
        transmitter.from = SetStrategy::Override(hosted.details.stub.point.to_surface());
        transmitter.agent = SetStrategy::Fill(Agent::Point(hosted.details.stub.point.clone()));
        let logger = self.logger.point(hosted.details.stub.point.clone());
        let phantom: PhantomData<P> = PhantomData::default();
        let skel = MechtronSkel::new(
            hosted.details.clone(),
            logger,
            transmitter.clone().build(),
            phantom,
        );

        Ok(skel)
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

        let factories = Arc::new(platform.factories()?);
        let skel = GuestSkel::new(details, factories,  platform);
        skel.logger.info("Guest created");

        Ok(Self { skel })
    }
}

impl<P> crate::Guest for Guest<P>
where
    P: Platform,
{
    fn handler(&self, point: &Point) -> Result<DirectedHandlerShell, GuestErr> {
        if *point == self.skel.details.stub.point {
            Ok(DirectedHandlerShell::new(
                Box::new(GuestHandler::new(self.skel.clone())),
                self.skel.builder(),
                self.skel.details.stub.point.to_surface(),
                self.skel.logger.logger.clone(),
            ))
        } else {
            let hosted = self.skel.hosted(point)?;
            let factory = self.skel.factories.get(&hosted.name).ok_or(format!(
                "cannot find factory assicated with name: {}",
                hosted.name
            ))?;

            let skel = self.skel.mechtron_skel(point)?;
            let mechtron = factory
                .handler(skel)
                .map_err(|e| GuestErr::from(e.to_string()))?;

            Ok(DirectedHandlerShell::new(
                mechtron,
                 self.skel.builder(),
                hosted.details.stub.point.to_surface(),
                self.skel.logger.logger.clone(),
            ))
        }
    }

    fn logger(&self) -> &PointLogger {
        &self.skel.logger
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
    P: Platform + 'static,
{
    skel: GuestSkel<P>,
}

impl<P> GuestHandler<P>
where
    P: Platform + 'static,
{
    pub fn new(skel: GuestSkel<P> ) -> Self {
        Self { skel }
    }
}

#[handler_sync]
impl<P> GuestHandler<P>
where
    P: Platform + 'static,
{
    #[route("Hyp<Host>")]
    pub fn assign(&self, ctx: InCtx<'_, HyperSubstance>) -> Result<(), P::Err> {
        self.skel.logger.info("Received Host command!");
        if let HyperSubstance::Host(host) = ctx.input {
            let factory = self
                .skel
                .logger
                .result(self.skel.factories.get(&host.name).ok_or(format!(
                    "Guest does not have a mechtron with name: {}",
                    host.name
                )))?;
            self.skel.logger.info("Creating...");
             self.skel.mechtrons.insert(
                host.details.stub.point.clone(),
                HostedMechtron::new(host.details.clone(), host.name.clone()),
            );

            let skel = self.skel.mechtron_skel(&host.details.stub.point)?;
            let mechtron = factory.lifecycle(skel)?;
            self.skel.logger.info("Got MechtronLifecycle...");
            let skel = self.skel.mechtron_skel(&host.details.stub.point)?;
            mechtron.create(skel )?;

            Ok(())
        } else {
            Err("expecting Host ".into())
        }
    }
}

#[derive(Clone)]
pub struct HostedMechtron {
    pub details: Details,
    pub name: String,
}

impl HostedMechtron {
    pub fn new(details: Details, name: String) -> Self {
        Self { details, name }
    }
}
