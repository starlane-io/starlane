#![allow(warnings)]
//# ! [feature(unboxed_closures)]
//#[macro_use]
//extern crate wasm_bindgen;
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;
extern crate alloc;
extern crate core;


use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use core::str::FromStr;
use std::collections::HashMap;
use std::sync::{mpsc, MutexGuard};
use std::sync::mpsc::Sender;
use cosmic_universe::err::UniErr;
use cosmic_universe::loc::{Layer, Point, ToSurface, Uuid};
use cosmic_universe::particle::{Details, Stub};
use cosmic_universe::wave::{Agent, UltraWave};
use dashmap::DashMap;
use tokio::runtime::Runtime;
use cosmic_macros::DirectedHandler;
use cosmic_macros::handler;
use cosmic_macros::route;
use cosmic_universe::hyper::HyperSubstance;
use cosmic_universe::log::{LogSource, NoAppender, PointLogger, RootLogger};
use cosmic_universe::parse::SkewerCase;
use cosmic_universe::{loc, VERSION};
use cosmic_universe::wasm::Timestamp;
use cosmic_universe::wave::exchange::{DirectedHandler, DirectedHandlerShell, Exchanger, InCtx, SetStrategy, TxRouter};

use std::sync::Mutex;
use cosmic_universe::wave::exchange::asynch::{ProtoTransmitter, ProtoTransmitterBuilder};

use wasm_membrane_guest::membrane::{log, membrane_consume_buffer, membrane_consume_string, membrane_guest_alloc_buffer, membrane_guest_version, membrane_read_buffer, membrane_read_string, membrane_write_buffer};

lazy_static! {
    static ref TX: Mutex<Option<mpsc::Sender<UltraWave>>>= Mutex::new(None);
    static ref RUNTIME: Runtime =  tokio::runtime::Builder::new_current_thread().build().unwrap();
}

#[no_mangle]
extern "C" {
    pub fn mechtron_frame_to_host(frame: i32);
    pub fn mechtron_uuid() -> i32;
    pub fn mechtron_timestamp() -> i64;
    pub fn mechtron_register(factories: & mut MechtronFactories ) -> Result<(),UniErr>;
}

#[no_mangle]
extern "C" fn cosmic_uuid() -> loc::Uuid{
    loc::Uuid::from_unwrap(membrane_consume_string(unsafe{mechtron_uuid()} ).unwrap())
}

#[no_mangle]
extern "C" fn cosmic_timestamp() -> Timestamp {
    Timestamp::new(unsafe{mechtron_timestamp()})
}



#[no_mangle]
pub fn mechtron_guest_init(version: i32, frame: i32) -> i32{
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
    let (tx,rx) = mpsc::channel();
    TX.lock().unwrap().replace(tx);
    RUNTIME.block_on( async move  {
           // Guest::new(rx, details, factories).await
    });
      0
}

#[no_mangle]
pub fn mechtron_frame_to_guest(frame: i32) {
      let frame = membrane_consume_buffer(frame).unwrap();
      let wave: UltraWave = bincode::deserialize(frame.as_slice()).unwrap();
      match TX.lock().unwrap().as_ref() {
          None => {}
          Some(tx) => {
//              tx.send(wave);
          }
      }
    /*
    RUNTIME.block_on( async move {
      GUEST.read().await.unwrap();
      });
     */

}


pub struct MechtronFactories {
    factories: HashMap<String,Box<dyn MechtronFactory>>
}

impl MechtronFactories {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new()
        }
    }
    pub fn add<F>( &mut self, factory: F ) where F: MechtronFactory {
        SkewerCase::from_str(factory.name().as_str() ).expect("Mechtron Name must be valid kebab (skewer) case (all lower case alphanumeric and dashes with leading letter)");
        self.factories.insert( factory.name(), Box::new(factory));
    }
}

pub struct GuestTx {
    pub tx: tokio::sync::broadcast::Sender<UltraWave>,
    pub rx: tokio::sync::broadcast::Receiver<UltraWave>,
}

impl GuestTx {
    pub fn new() -> Self {
        let (tx,rx):(tokio::sync::broadcast::Sender<UltraWave>, tokio::sync::broadcast::Receiver<UltraWave>) = tokio::sync::broadcast::channel(1024);
        Self {
            tx,
            rx
        }
    }
}

pub struct Guest {
   details: Details,
   mechtrons: DashMap<Point,Details>,
   factories: MechtronFactories,
   rx: mpsc::Receiver<UltraWave>,
   logger: PointLogger,
   handler: DirectedHandlerShell<GuestHandler>,
   exchanger: Exchanger,
   pub runtime: Runtime
}

impl Guest {
    pub async fn new(rx: mpsc::Receiver<UltraWave>, details: Details, factories: MechtronFactories) {
        let root_logger = RootLogger::new( LogSource::Core, Arc::new(NoAppender::new()) );
        let logger = root_logger.point(details.stub.point.clone());
        let handler = GuestHandler { };

        let surface = details.stub.point.to_surface().with_layer(Layer::Core);
        let (out_tx, out_rx) = tokio::sync::mpsc::channel(1024);
        let router = Arc::new(TxRouter::new(out_tx));
        let exchanger = Exchanger::new( surface.clone(), Default::default() );

        let mut transmitter = ProtoTransmitterBuilder::new(router,exchanger.clone());
        transmitter.from =SetStrategy::Override(surface.clone());
        transmitter.agent =SetStrategy::Override(Agent::Point(surface.point.clone()));
        let handler = DirectedHandlerShell::new( handler, transmitter, surface, logger.logger.clone() );
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build().unwrap();


        let guest = Self {
            details,
            mechtrons: DashMap::new(),
            factories,
            rx,
            handler,
            exchanger,
            logger,
            runtime
        };
//        guest.start().await;
        /*
        {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build().unwrap();

            runtime.block_on(async move {
                guest.start().await;
            });

        }
         */
    }

    pub async fn start(mut self) {
        loop {
           // self.rx.recv();
        }
//            self.rx.recv().await;
/*        while let Ok(wave) = self.rx.recv().await {
            if wave.is_directed() {
                let directed = wave.to_directed().unwrap();
                self.handler.handle(directed).await;
            } else {
                let reflected = wave.to_reflected().unwrap();
                self.exchanger.reflected(reflected).await.unwrap();
            }
        }

 */
    }

    pub fn reg_mechtron(&self, details: Details ) {
        self.mechtrons.insert( details.stub.point.clone(), details );
    }

    pub fn route_to_host(wave: UltraWave) {
        tokio::spawn( async move {
            match bincode::serialize(&wave) {
                Ok(data) => {
                    let buffer_id = membrane_write_buffer(data);
                    unsafe {
                        mechtron_frame_to_host(buffer_id);
                    }
                }
                Err(err) => {

                }
            }
        });
    }
}

#[derive(DirectedHandler)]
pub struct GuestHandler {

}

#[handler]
impl GuestHandler {

    #[route("Hyp<Assign>")]
    pub async fn assign( &self, ctx: InCtx<'_,HyperSubstance>) -> Result<(),UniErr> {
        if let HyperSubstance::Assign(assign) = ctx.input
        {
            Ok(())
        }
        else {
            Err("expecting Assign".into())
        }
    }

}

pub trait MechtronFactory: Sync + Send + 'static {
    fn name(&self) -> String;
    fn create(&self, details: Details) -> Result<Box<dyn MechtronLifecycle>, UniErr>;
}

/// The MechtronSkel holds the common static elements of the Mechtron together
/// Since a Mechtron is always an instance created to handle a single
/// Directed Wave or Init, the Skel is cloned and passed to each
/// Mechtron instance.
///
#[derive(Clone)]
pub struct MechtronSkel {
   pub details: Details
}

/// The Mechtron Context, it holds a transmitter for sending Waves
/// which can be used outside of a Directed/Reflected Wave Handler interaction
#[derive(Clone)]
pub struct MechtronCtx {
    pub transmitter: ProtoTransmitter
}

/// MechtronSphere is the interface used by Guest
/// to make important calls to the Mechtron
pub trait MechtronLifecycle: DirectedHandler+Sync+Send {
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
   fn restore(skel: Self::Skel, ctx: Self::Ctx, state: Self::State ) -> Self;
}





#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
