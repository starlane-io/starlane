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
use cosmic_universe::VERSION;

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

}




#[cfg(test)]
mod tests {
    use std::fs;
    use std::str::FromStr;
    use super::*;

    #[test]
    fn wasm() {
        let factory = MechtronHostFactory::new();
        let point = Point::from_str("guest").unwrap();
        let data = Arc::new(fs::read("../../wasm/my-app/my_app.wasm").unwrap());
        let host = factory.create(point, data).unwrap();
        let details = Details::default();
        host.init(details).unwrap();
    }
}
