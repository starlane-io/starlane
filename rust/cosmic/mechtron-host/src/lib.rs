pub mod err;

use std::sync::{Arc, Mutex};
use wasmer::{Module, Cranelift, Store, Universal, imports};
use threadpool::ThreadPool;
use wasmer_compiler_singlepass::Singlepass;
use cosmic_universe::err::UniErr;
use cosmic_universe::loc::Point;
use cosmic_universe::substance::Bin;
use wasm_membrane_host::membrane::WasmMembrane;
use wasmer::Function;
use cosmic_universe::particle::Details;
use cosmic_universe::{loc, VERSION};
use cosmic_universe::wasm::Timestamp;
use cosmic_universe::wave::UltraWave;

use crate::err::HostErr;

pub struct MechtronHostFactory {
   store: Store,
   ctx: MechtronHostCtx
}

impl MechtronHostFactory {
    pub fn new() -> Self {
        let compiler = Singlepass::new();
        let store = Store::new(&Universal::new(compiler).engine() );
        let ctx = MechtronHostCtx {
            pool: Arc::new( Mutex::new( ThreadPool::new(10)))
        };
        Self {
            ctx,
            store
        }
    }

    pub fn create(&self, point: Point, data: Bin) -> Result<MechtronHost, HostErr> {
        let module = Arc::new(Module::new(&self.store, data.as_ref())?);
        let membrane = WasmMembrane::new(module, point.to_string())?;

        MechtronHost::new( point, membrane, self.ctx.clone() )
    }
}

#[derive(Clone)]
pub struct MechtronHostCtx {
    pool: Arc<Mutex<ThreadPool>>
}

pub struct MechtronHost {
  pub ctx:  MechtronHostCtx,
  pub point: Point,
  pub membrane: Arc<WasmMembrane>
}

impl MechtronHost {
    pub fn new(point: Point, membrane: Arc<WasmMembrane>, ctx: MechtronHostCtx ) -> Result<Self, HostErr> {
        Ok(Self {
            ctx,
            point,
            membrane
        })
    }

    pub fn init(&self, details: Details) -> Result<(),HostErr> {
        self.membrane.init()?;
        let version = self.membrane.write_string( VERSION.to_string() )?;
        let details: Vec<u8> = bincode::serialize(&details)?;
        let details = self.membrane.write_buffer(&details )?;
        let ok = self
            .membrane.instance
            .exports
            .get_native_function::<(i32,i32), i32>("mechtron_guest_init")
            .unwrap()
            .call(version,details)?;
        if ok == 0 {
            Ok(())
        } else {
            Err("Mehctron init error".into())
        }
    }

     pub fn route(&self, wave: UltraWave) -> Result<(),HostErr> {
        let wave: Vec<u8> = bincode::serialize(&wave)?;
        let wave = self.membrane.write_buffer(&wave)?;

        self
            .membrane.instance
            .exports
            .get_native_function::<i32,() >("mechtron_frame_to_guest")
            .unwrap()
            .call(wave )?;


         Ok(())
    }

}
#[no_mangle]
extern "C" fn cosmic_uuid() -> loc::Uuid{
    loc::Uuid::from(uuid::Uuid::new_v4().to_string()).unwrap()
}

#[no_mangle]
extern "C" fn cosmic_timestamp() -> Timestamp {
    Timestamp::new(chrono::Utc::now().timestamp_millis())
}




#[cfg(test)]
mod tests {
    use std::{fs, thread};
    use std::str::FromStr;
    use cosmic_universe::kind::Sub;
    use cosmic_universe::loc::ToSurface;
    use cosmic_universe::wave::{DirectedProto, WaveId, WaveKind};
    use super::*;

    #[test]
    fn wasm() {
        let factory = MechtronHostFactory::new();
        let point = Point::from_str("guest").unwrap();
        let data = Arc::new(fs::read("../../wasm/my-app/my_app.wasm").unwrap());
        let host = factory.create(point, data).unwrap();
        let details = Details::default();
        host.init(details).unwrap();

        let mut wave = DirectedProto::ping();
        wave.to(Point::local_endpoint().to_surface());
        wave.from(Point::local_endpoint().to_surface());

        let wave = wave.build().unwrap();;
        let wave = wave.to_ultra();
        host.route(wave).unwrap();

    }
}
