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

    pub fn init(&self) -> Result<(),HostErr> {
        self.membrane.init()?;
        Ok(())
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
        host.init().unwrap();
    }
}
