#![allow(warnings)]

pub mod err;
pub mod guest;
mod membrane;
mod uni;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate cosmic_macros;


extern crate alloc;
extern crate core;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use core::str::FromStr;
use cosmic_macros::handler;
use cosmic_macros::route;
use cosmic_macros::DirectedHandler;
use cosmic_universe::err::UniErr;
use cosmic_universe::hyper::HyperSubstance;
use cosmic_universe::loc::{Layer, Point, ToSurface, Uuid};
use cosmic_universe::log::{LogSource, NoAppender, PointLogger, RootLogger};
use cosmic_universe::parse::SkewerCase;
use cosmic_universe::particle::{Details, Stub};
use cosmic_universe::wasm::Timestamp;
use cosmic_universe::wave::exchange::SetStrategy;
use cosmic_universe::wave::{Agent, DirectedWave, ReflectedAggregate, ReflectedWave, UltraWave};
use cosmic_universe::{loc, VERSION};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::mpsc::Sender;
use std::sync::{mpsc, MutexGuard};

use cosmic_universe::wave::Bounce;

use cosmic_universe::wave::exchange::synch::{DirectedHandler, DirectedHandlerProxy, DirectedHandlerShell, ExchangeRouter, InCtx, ProtoTransmitter, ProtoTransmitterBuilder};
use std::sync::RwLock;
use cosmic_universe::artifact::ArtifactApi;

use crate::err::{GuestErr, MechErr};
use crate::membrane::{mechtron_frame_to_host,  mechtron_timestamp, mechtron_uuid};

#[no_mangle]
extern "C" {
    pub fn mechtron_guest(details: Details) -> Result<Arc<dyn Guest>,GuestErr>;
}

pub trait Guest: Send+Sync {
     fn handler(&self, point: &Point) -> Result<DirectedHandlerShell, GuestErr>;
     fn logger(&self) -> &PointLogger;
}

pub trait Platform: Clone+Send+Sync where Self::Err : MechErr {
    type Err;
    fn factories(&self) -> Result<MechtronFactories<Self>, Self::Err> where Self:Sized{
        Ok(MechtronFactories::new())
    }
}


pub struct MechtronFactories<P> where P: Platform {
    factories: HashMap<String, Box<dyn MechtronFactory<P>>>,
    phantom: PhantomData<P>
}

impl <P> MechtronFactories<P> where P: Platform {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            phantom: Default::default()
        }
    }
    pub fn add<F>(&mut self, factory: F)
    where
        F: MechtronFactory<P>,
    {
        SkewerCase::from_str(factory.name().as_str() ).expect("Mechtron Name must be valid kebab (skewer) case (all lower case alphanumeric and dashes with leading letter)");
        self.factories.insert(factory.name(), Box::new(factory));
    }

    pub fn get<S>(&self, name: S) -> Option<&Box<dyn MechtronFactory<P>>> where S: ToString {
        self.factories.get(&name.to_string() )
    }
}

pub trait MechtronFactory<P>: Sync + Send + 'static where P: Platform {
    fn name(&self) -> String;
    fn lifecycle(&self, details: &Details, logger: PointLogger) -> Result<Box<dyn MechtronLifecycle<P>>, P::Err>;
    fn handler(&self, details: &Details, transmitter: ProtoTransmitter, logger: PointLogger ) -> Result<Box<dyn DirectedHandler>, P::Err>;
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}

/// The MechtronSkel holds the common static elements of the Mechtron together
/// Since a Mechtron is always an instance created to handle a single
/// Directed Wave or Init, the Skel is cloned and passed to each
/// Mechtron instance.
///
#[derive(Clone)]
pub struct MechtronSkel<P> where P: Platform {
    pub details: Details,
    pub logger: PointLogger,
    phantom: PhantomData<P>
}

impl <P> MechtronSkel<P> where P: Platform{
    pub fn new( details: Details, logger: PointLogger, phantom: PhantomData<P> ) -> Self {
        let logger = logger.point(details.stub.point.clone());
        Self {
            details,
            logger,
            phantom
        }
    }
}

/// The Mechtron Context, it holds a transmitter for sending Waves
/// which can be used outside of a Directed/Reflected Wave Handler interaction
#[derive(Clone)]
pub struct MechtronCtx {
    pub transmitter: ProtoTransmitter,
}

impl MechtronCtx {
    pub fn new( transmitter: ProtoTransmitter ) -> Self {
        Self {
            transmitter
        }
    }
}

/// MechtronSphere is the interface used by Guest
/// to make important calls to the Mechtron
pub trait MechtronLifecycle<P>: DirectedHandler + Sync + Send where P: Platform {

    fn create(&self, _ctx: MechtronCtx ) -> Result<(), P::Err> {
        Ok(())
    }

}

/// Create a Mechtron by implementing this trait.
/// Mechtrons are created per request and disposed of afterwards...
/// Implementers of this trait should only hold references to
/// Mechtron::Skel, Mechtron::Ctx & Mechtron::State at most.
pub trait Mechtron<P>: MechtronLifecycle<P> + Sync + Send + 'static where P: Platform {
    /// it is recommended to implement MechtronSkel or some derivative
    /// of MechtronSkel. Skel holds info about the Mechtron (like it's Point,
    /// exact Kind & Properties)  The Skel may also provide access to other
    /// services within the Guest. If your Mechtron doesn't use the Skel
    /// then implement ```type Skel=()```
    type Skel;

    /// it is recommended to implement MechtronCtx or some derivative of MechtronCtx.
    /// Ctx provides the ProtoTransmitter which can be used outside of a
    /// Directed/Reflected Wave interaction.  If you don't need Ctx then
    /// implement ```type Ctx=()```
    type Ctx;

    /// Is any static data (templates, config files) that does not change
    /// and may need to be reused
    type Cache;

    /// State is the aspect of the Mechtron that is changeable.  It is recommended
    /// to wrap State in a tokio Mutex or RwLock if used.  If you are implementing
    /// a statelens mechtron then implement ```type State=();```
    type State;

    /// This method is called by a companion MechtronFactory implementation
    /// to bring this Mechtron back to life to handle an Init or a Directed Wave
    fn restore(skel: Self::Skel, ctx: Self::Ctx, cache: Self::Cache, state: Self::State) -> Self;

    /// create the Cache for this Mechtron (templates, configs & static content)
    /// the cache should hold any static content that is expected to be unchanging
    fn cache(ctx: Self::Ctx) -> Self::Cache;
}


