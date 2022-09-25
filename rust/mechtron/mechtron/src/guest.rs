use cosmic_universe::particle::Details;
use std::collections::HashMap;
use std::marker::PhantomData;
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::HyperSubstance;
use cosmic_universe::loc::{Layer, Point, ToSurface};
use cosmic_universe::log::{LogSource, NoAppender, PointLogger, RootLogger, SynchTransmittingLogAppender};
use cosmic_universe::wave::exchange::synch::{DirectedHandler, DirectedHandlerProxy, DirectedHandlerShell, ExchangeRouter, InCtx, ProtoTransmitter, ProtoTransmitterBuilder, RootInCtx};
use std::sync::Arc;
use cosmic_universe::wave::{Agent, DirectedWave, ReflectedAggregate, ToRecipients, UltraWave};
use cosmic_universe::wave::core::CoreBounce;
use cosmic_universe::wave::exchange::SetStrategy;
use crate::{MechtronFactories, Platform};
use crate::err::GuestErr;
use crate::err::MechErr;
use cosmic_macros::handler_sync;

pub struct Guest<P> where P:Platform + 'static {
    details: Details,
    mechtrons: HashMap<Point, Details>,
    factories: MechtronFactories<P>,
    logger: PointLogger,
    platform: P
}

impl <P> Guest<P> where P: Platform + 'static {
    pub fn new(details: Details, platform : P) -> Result<Self,GuestErr> where P: Sized, GuestErr: From<<P as Platform>::Err>{
        let router: GuestRouter<P> = GuestRouter::new();
        let router = Arc::new(router);
        let mut transmitter = ProtoTransmitterBuilder::new(router);
        transmitter.agent = SetStrategy::Override(Agent::Point(details.stub.point.clone()));
        transmitter.from = SetStrategy::Override(details.stub.point.clone().to_surface());
        transmitter.to = SetStrategy::Override(Point::global_logger().to_surface().to_recipients());
        let appender = SynchTransmittingLogAppender::new(transmitter);
        let root_logger = RootLogger::new(LogSource::Core, Arc::new(appender ));
        let logger = root_logger.point(details.stub.point.clone());
        logger.info("Guest created");

        let factories = platform.factories()?;

        Ok(Self {
            details,
            mechtrons: HashMap::new(),
            factories,
            logger,
            platform
        })
    }

}

impl <G> crate::Guest for Guest<G> where G: Platform {
     fn handler(&self) -> DirectedHandlerShell<DirectedHandlerProxy> {
        let surface = self.details.stub.point.to_surface().with_layer(Layer::Core);
        let router: GuestRouter<G> = GuestRouter::new();
        let router = Arc::new(router);
        let mut transmitter = ProtoTransmitterBuilder::new(router);
        transmitter.from = SetStrategy::Override(surface.clone());
        transmitter.agent = SetStrategy::Override(Agent::Point(surface.point.clone()));
        DirectedHandlerShell::new( DirectedHandlerProxy::new( GuestHandler::new() ), transmitter, self.details.stub.point.to_surface(), self.logger.logger.clone() )
    }
}


pub struct GuestRouter<P> where P: crate::Platform {
    phantom: PhantomData<P>
}

impl <P> GuestRouter<P> where P: crate::Platform {
    pub fn new() -> Self { Self {
        phantom: PhantomData::default()
    } }
}


impl <P> ExchangeRouter for GuestRouter<P> where P: crate::Platform {
    fn route(&self, wave: UltraWave) {
        crate::membrane::mechtron_exchange_wave_host::<P>(wave);
    }

    fn exchange(&self, direct: DirectedWave) -> Result<ReflectedAggregate, UniErr> {
        crate::membrane::mechtron_exchange_wave_host::<P>(direct.to_ultra()).map_err(|e|e.to_uni_err())
    }
}



pub struct GuestHandler;

impl GuestHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[handler_sync]
impl GuestHandler {

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
