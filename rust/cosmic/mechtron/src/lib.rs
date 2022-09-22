#![allow(warnings)]
//# ! [feature(unboxed_closures)]
//#[macro_use]
//extern crate wasm_bindgen;
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;
extern crate alloc;


use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use cosmic_universe::err::UniErr;
use cosmic_universe::loc::{Layer, Point, ToSurface, Uuid};
use cosmic_universe::particle::{Details, Stub};
use cosmic_universe::wave::{Agent, UltraWave};
use dashmap::DashMap;
use cosmic_macros::DirectedHandler;
use cosmic_macros::handler;
use cosmic_macros::route;
use cosmic_universe::hyper::HyperSubstance;
use cosmic_universe::log::{LogSource, NoAppender, PointLogger, RootLogger};
use cosmic_universe::wave::exchange::{DirectedHandlerShell, Exchanger, InCtx, ProtoTransmitterBuilder, SetStrategy, TxRouter};

use wasm_membrane_guest::membrane::{
    log, membrane_guest_version, membrane_consume_buffer, membrane_read_buffer, membrane_read_string, membrane_write_buffer, membrane_guest_alloc_buffer
};

lazy_static! {
    static ref TX: MechtronGuestTx = MechtronGuestTx::new();
    static ref FACTORIES:  Arc<DashMap<String,Box<dyn MechtronFactory>>> = Arc::new(DashMap::new());
}

#[no_mangle]
extern "C" {
    pub fn mechtron_registration();
    pub fn mechtron_frame_to_host(frame: i32);
}



#[no_mangle]
pub fn mechtron_guest_init(frame: i32) {
      let frame = membrane_consume_buffer(frame).unwrap();
      let details: Details = bincode::deserialize(frame.as_slice()).unwrap();
      MechtronGuest::new(details);
}

#[no_mangle]
pub fn mechtron_frame_to_guest(frame: i32) {
      let frame = membrane_consume_buffer(frame).unwrap();
      let frame: UltraWave = bincode::deserialize(frame.as_slice()).unwrap();
      mechtron_guest_receive(frame);
}

pub fn mechtron_guest_send(wave: UltraWave) -> Result<(),UniErr> {
    let data = bincode::serialize(&wave)?;
    let buffer_id = membrane_write_buffer(data);
    unsafe {
        mechtron_frame_to_host(buffer_id);
    }
    Ok(())
}

pub fn mechtron_guest_receive( wave: UltraWave ) {
    TX.tx.send(wave).unwrap();
}

pub fn mechtron_register_factory( factory: Box<dyn MechtronFactory>) {
    FACTORIES.insert( factory.name(), factory );
}

pub struct MechtronGuestTx {
    pub tx: tokio::sync::broadcast::Sender<UltraWave>,
    pub rx: tokio::sync::broadcast::Receiver<UltraWave>,
}

impl MechtronGuestTx {
    pub fn new() -> Self {
        let (tx,rx):(tokio::sync::broadcast::Sender<UltraWave>, tokio::sync::broadcast::Receiver<UltraWave>) = tokio::sync::broadcast::channel(1024);
        Self {
            tx,
            rx
        }
    }
}

pub struct MechtronGuest {
   details: Details,
   mechtrons: DashMap<Point,Details>,
   tx: tokio::sync::broadcast::Sender<UltraWave>,
   rx: tokio::sync::broadcast::Receiver<UltraWave>,
   logger: PointLogger,
   handler: DirectedHandlerShell<MechtronGuestHandler>,
   exchanger: Exchanger
}

impl MechtronGuest {
    pub fn new(details: Details) {

        let root_logger = RootLogger::new( LogSource::Core, Arc::new(NoAppender::new()) );
        let logger = root_logger.point(details.stub.point.clone());
        let handler = MechtronGuestHandler { };

        let surface = details.stub.point.to_surface().with_layer(Layer::Core);
        let (out_tx, out_rx) = tokio::sync::mpsc::channel(1024);
        let router = Arc::new(TxRouter::new(out_tx));
        let exchanger = Exchanger::new( surface.clone(), Default::default() );

        let mut transmitter = ProtoTransmitterBuilder::new(router,exchanger.clone());
        transmitter.from =SetStrategy::Override(surface.clone());
        transmitter.agent =SetStrategy::Override(Agent::Point(surface.point.clone()));
        let handler = DirectedHandlerShell::new( handler, transmitter, surface, logger.logger.clone() );

        let mut guest = Self {
            details,
            mechtrons: DashMap::new(),
            tx: TX.tx.clone(),
            rx: TX.tx.subscribe(),
            handler,
            exchanger,
            logger
        };
        {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build().unwrap();
            runtime.block_on(async move {
                guest.start().await;
            });
        }
    }

    pub async fn start(mut self) {
        while let Ok(wave) = self.rx.recv().await {
            if wave.is_directed() {
                let directed = wave.to_directed().unwrap();
                self.handler.handle(directed).await;
            } else {
                let reflected = wave.to_reflected().unwrap();
                self.exchanger.reflected(reflected).await.unwrap();
            }
        }
    }

    pub fn reg_mechtron(&self, details: Details ) {
        self.mechtrons.insert( details.stub.point.clone(), details );
    }

}

#[derive(DirectedHandler)]
pub struct MechtronGuestHandler {

}

#[handler]
impl MechtronGuestHandler {

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
    fn create(&self, details: Details) -> Result<Box<dyn Mechtron>, UniErr>;
}


pub struct MechtronSkel {

}


pub trait Mechtron: Sync + Send + 'static {

}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
