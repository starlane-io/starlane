pub mod err;
mod membrane;

use crate::err::HostErr;
use cosmic_universe::err::UniErr;
use cosmic_universe::loc::Point;
use cosmic_universe::particle::Details;
use cosmic_universe::substance::Bin;
use cosmic_universe::wasm::Timestamp;
use cosmic_universe::wave::UltraWave;
use cosmic_universe::{loc, VERSION};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use threadpool::ThreadPool;
use wasm_membrane_host::membrane::WasmMembrane;
use wasmer::Function;
use wasmer::{imports, Cranelift, Module, Store, Universal};
use wasmer_compiler_singlepass::Singlepass;

pub trait HostPlatform: Clone+Send+Sync
where
    Self::Err: HostErr,
{
    type Err;
}

pub struct MechtronHostFactory<P>
where
    P: HostPlatform,
{
    store: Store,
    ctx: MechtronHostCtx,
    platform: P,
}

impl<P> MechtronHostFactory<P>
where
    P: HostPlatform,
{
    pub fn new(platform: P ) -> Self {
        let compiler = Singlepass::new();
        let store = Store::new(&Universal::new(compiler).engine());
        let ctx = MechtronHostCtx {
            pool: Arc::new(Mutex::new(ThreadPool::new(10))),
        };
        Self {
            ctx,
            store,
            platform
        }
    }

    pub fn create(&self, point: Point, data: Bin) -> Result<MechtronHost<P>, P::Err> {
        let module = Arc::new(Module::new(&self.store, data.as_ref())?);
        let membrane = WasmMembrane::new(module, point.to_string())?;

        MechtronHost::new(point, membrane, self.ctx.clone(),self.platform.clone())
    }
}

#[derive(Clone)]
pub struct MechtronHostCtx {
    pool: Arc<Mutex<ThreadPool>>,
}

pub struct MechtronHost<P>
where
    P: HostPlatform,
{
    pub ctx: MechtronHostCtx,
    pub point: Point,
    pub membrane: Arc<WasmMembrane>,
    pub platform: P
}

impl<P> MechtronHost<P>
where
    P: HostPlatform,
{
    pub fn new(
        point: Point,
        membrane: Arc<WasmMembrane>,
        ctx: MechtronHostCtx,
        platform: P
    ) -> Result<Self, P::Err> {
        Ok(Self {
            ctx,
            point,
            membrane,
            platform
        })
    }

    pub fn init(&self, details: Details) -> Result<(), P::Err> {
        self.membrane.init()?;
        let version = self.membrane.write_string(VERSION.to_string())?;
        let details: Vec<u8> = bincode::serialize(&details)?;
        let details = self.membrane.write_buffer(&details)?;
        let ok = self
            .membrane
            .instance
            .exports
            .get_native_function::<(i32, i32), i32>("mechtron_guest_init")
            .unwrap()
            .call(version, details)?;
        if ok == 0 {
            Ok(())
        } else {
            Err("Mehctron init error".into())
        }
    }

    pub fn route(&self, wave: UltraWave) -> Result<Option<UltraWave>, P::Err> {
        let wave: Vec<u8> = bincode::serialize(&wave)?;
        let wave = self.membrane.write_buffer(&wave)?;

        let reflect = self
            .membrane
            .instance
            .exports
            .get_native_function::<i32, i32>("mechtron_frame_to_guest")
            .unwrap()
            .call(wave)?;

        if reflect == 0 {
            Ok(None)
        } else {
            let reflect = self.membrane.consume_buffer(reflect)?;
            let reflect = reflect.as_slice();
            let reflect: UltraWave = bincode::deserialize(reflect)?;
            Ok(Some(reflect))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_universe::kind::Sub;
    use cosmic_universe::loc::ToSurface;
    use cosmic_universe::wave::{DirectedProto, WaveId, WaveKind};
    use std::str::FromStr;
    use std::{fs, thread};
    use crate::err::DefaultHostErr;

    #[no_mangle]
    extern "C" fn cosmic_uuid() -> loc::Uuid {
        loc::Uuid::from(uuid::Uuid::new_v4().to_string()).unwrap()
    }

    #[no_mangle]
    extern "C" fn cosmic_timestamp() -> Timestamp {
        Timestamp::new(chrono::Utc::now().timestamp_millis())
    }

    #[derive(Clone)]
    pub struct TestPlatform {

    }

    impl TestPlatform {
        pub fn new() -> Self { Self {} }
    }

    impl HostPlatform for TestPlatform {
        type Err = DefaultHostErr;
    }

    #[test]
    fn wasm() {
        let factory = MechtronHostFactory::new(TestPlatform::new());
        let point = Point::from_str("guest").unwrap();
        let data = Arc::new(fs::read("../../wasm/my-app/my_app.wasm").unwrap());
        let host = factory.create(point, data).unwrap();
        let details = Details::default();
        host.init(details).unwrap();

        let mut wave = DirectedProto::ping();
        wave.to(Point::local_endpoint().to_surface());
        wave.from(Point::local_endpoint().to_surface());

        let wave = wave.build().unwrap();
        let wave = wave.to_ultra();
        host.route(wave).unwrap();
    }
}
