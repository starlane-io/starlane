#![allow(warnings)]

pub mod err;
pub mod synch;

//# ! [feature(unboxed_closures)]
//#[macro_use]
//extern crate wasm_bindgen;
#[macro_use]
extern crate lazy_static;

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
use std::sync::mpsc::Sender;
use std::sync::{mpsc, MutexGuard};

use cosmic_universe::wave::Bounce;

use cosmic_universe::wave::exchange::synch::{DirectedHandler, DirectedHandlerProxy, DirectedHandlerShell, ExchangeRouter, InCtx, ProtoTransmitter, ProtoTransmitterBuilder};
use std::sync::RwLock;

use wasm_membrane_guest::membrane::{
    log, membrane_consume_buffer, membrane_consume_string, membrane_guest_alloc_buffer,
    membrane_guest_version, membrane_read_buffer, membrane_read_string, membrane_write_buffer,
};
use crate::err::MechErr;

lazy_static! {
    static ref GUEST: RwLock<Option<synch::Guest>> = RwLock::new(None);
}

#[no_mangle]
extern "C" {
    pub fn mechtron_frame_to_host(frame: i32) -> i32;
    pub fn mechtron_uuid() -> i32;
    pub fn mechtron_timestamp() -> i64;
    pub fn mechtron_register(factories: &mut MechtronFactories) -> Result<(), UniErr>;
}

#[no_mangle]
extern "C" fn cosmic_uuid() -> loc::Uuid {
    loc::Uuid::from_unwrap(membrane_consume_string(unsafe { mechtron_uuid() }).unwrap())
}

#[no_mangle]
extern "C" fn cosmic_timestamp() -> Timestamp {
    Timestamp::new(unsafe { mechtron_timestamp() })
}

#[no_mangle]
pub fn mechtron_guest_init(version: i32, frame: i32) -> i32 {
    let mut factories = MechtronFactories::new();
    unsafe {
        if let Err(_) = mechtron_register(&mut factories) {
            return -1;
        }
    }
    let version = membrane_consume_string(version).unwrap();
    if version != VERSION.to_string() {
        return -2;
    }
    let frame = membrane_consume_buffer(frame).unwrap();
    let details: Details = bincode::deserialize(frame.as_slice()).unwrap();

    {
        let mut write = GUEST.write().unwrap();
        let guest = synch::Guest::new(details, factories);
        write.replace(guest);
    }

    0
}

#[no_mangle]
pub fn mechtron_frame_to_guest(frame: i32) -> i32 {
    let frame = membrane_consume_buffer(frame).unwrap();
    let wave: UltraWave = bincode::deserialize(frame.as_slice()).unwrap();

    if wave.is_directed() {
        let wave = wave.to_directed().unwrap();
        let handler: DirectedHandlerShell<DirectedHandlerProxy> = {
            let read = GUEST.read().unwrap();
            let guest : &synch::Guest = read.as_ref().unwrap();
            guest.handler()
        };

        match handler.handle(wave) {
            Bounce::Absorbed => 0,
            Bounce::Reflected(wave) => {
                let wave = mechtron_write_wave_to_host(wave.to_ultra() ).unwrap();
                wave
            }
        }
    } else {
        // we simply do not deal with ReflectedWaves at this time
        // unless they are in the context of the same thread that made the request
        0
    }
}

pub fn mechtron_write_wave_to_host(wave: UltraWave) -> Result<i32, UniErr> {
    let data = bincode::serialize(&wave)?;
    Ok(membrane_write_buffer(data))
}

pub fn mechtron_exchange_wave_host<G>(wave: UltraWave) -> Result<ReflectedAggregate, G::Err> where G:Guest {
    let data = bincode::serialize(&wave)?;
    let buffer_id = membrane_write_buffer(data);
    let reflect_id = unsafe { mechtron_frame_to_host(buffer_id) };

    if reflect_id == 0 {
        Ok(ReflectedAggregate::None)
    } else {
        let buffer = membrane_consume_buffer(reflect_id)?;
        let agg: ReflectedAggregate = bincode::deserialize(buffer.as_slice())?;
        Ok(agg)
    }
}

pub struct MechtronFactories {
    factories: HashMap<String, Box<dyn MechtronFactory>>,
}

impl MechtronFactories {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
        }
    }
    pub fn add<F>(&mut self, factory: F)
    where
        F: MechtronFactory,
    {
        SkewerCase::from_str(factory.name().as_str() ).expect("Mechtron Name must be valid kebab (skewer) case (all lower case alphanumeric and dashes with leading letter)");
        self.factories.insert(factory.name(), Box::new(factory));
    }
}

pub trait Guest where Self::Err : MechErr {
    type Err;
}

pub trait MechtronFactory: Sync + Send + 'static {
    fn name(&self) -> String;
    fn create(&self, details: Details) -> Result<Box<dyn MechtronLifecycle>, UniErr>;
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
pub struct MechtronSkel {
    pub details: Details,
}

/// The Mechtron Context, it holds a transmitter for sending Waves
/// which can be used outside of a Directed/Reflected Wave Handler interaction
#[derive(Clone)]
pub struct MechtronCtx {
    pub transmitter: ProtoTransmitter,
}

/// MechtronSphere is the interface used by Guest
/// to make important calls to the Mechtron
pub trait MechtronLifecycle: DirectedHandler + Sync + Send {
    fn create(&self);
}

/// Create a Mechtron by implementing this trait.
/// Mechtrons are created per request and disposed of afterwards...
/// Implementers of this trait should only hold references to
/// Mechtron::Skel, Mechtron::Ctx & Mechtron::State at most.
pub trait Mechtron: MechtronLifecycle + Sync + Send + 'static {
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

    /// State is the aspect of the Mechtron that is changeable.  It is recommended
    /// to wrap State in a tokio Mutex or RwLock if used.  If you are implementing
    /// a statelens mechtron then implement ```type State=();```
    type State;

    /// This method is called by a companion MechtronFactory implementation
    /// to bring this Mechtron back to life to handle an Init or a Directed Wave
    fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State) -> Self;
}
