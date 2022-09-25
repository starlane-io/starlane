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
use wasmer::Function;
use wasmer::{imports, Cranelift, Module, Store, Universal};
use wasmer_compiler_singlepass::Singlepass;
use cosmic_universe::log::{PointLogger, RootLogger};
use crate::membrane::WasmMembrane;

pub trait HostPlatform: Clone+Send+Sync
where
    Self::Err: HostErr,
{
    type Err;

    fn root_logger(&self) -> RootLogger;
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
    P: HostPlatform+'static,
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

    pub fn create(&self, details: Details, data: Bin) -> Result<MechtronHost<P>, P::Err> {
        let logger = self.platform.root_logger();
        let logger = logger.point(details.stub.point.clone());

        let module = Arc::new(Module::new(&self.store, data.as_ref())?);
        let membrane = WasmMembrane::new(module, details.stub.point.to_string(),self.platform.clone(),logger.clone())?;

        MechtronHost::new(details, membrane, self.ctx.clone(),self.platform.clone(), logger)
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
    pub details: Details,
    pub logger: PointLogger,
    pub ctx: MechtronHostCtx,
    pub membrane: Arc<WasmMembrane<P>>,
    pub platform: P
}

impl<P> MechtronHost<P>
where
    P: HostPlatform,
{
    pub fn new(
        details: Details,
        membrane: Arc<WasmMembrane<P>>,
        ctx: MechtronHostCtx,
        platform: P,
        logger: PointLogger
    ) -> Result<Self, P::Err> {


        Ok(Self {
            ctx,
            details,
            membrane,
            logger,
            platform
        })
    }

    pub fn point(&self) -> &Point {
        &self.details.stub.point
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
            Err(format!("Mehctron init error {} ",ok).into())
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
    use cosmic_universe::kind::{Kind, Sub};
    use cosmic_universe::loc::ToSurface;
    use cosmic_universe::wave::{DirectedProto, WaveId, WaveKind};
    use std::str::FromStr;
    use std::{fs, thread};
    use cosmic_universe::command::common::StateSrc;
    use cosmic_universe::hyper;
    use cosmic_universe::hyper::{Assign, AssignmentKind, HyperSubstance};
    use cosmic_universe::log::{LogSource, StdOutAppender};
    use cosmic_universe::particle::{Status, Stub};
    use cosmic_universe::substance::Substance;
    use cosmic_universe::wave::core::hyp::HypMethod;
    use cosmic_universe::wave::core::Method;
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

        fn root_logger(&self) -> RootLogger {
            RootLogger::new( LogSource::Core, Arc::new(StdOutAppender::new()))
        }
    }

    #[test]
    fn wasm() {

        let mut details:Details = Default::default();
        details.stub.point = Point::from_str("host").unwrap();

        let factory = MechtronHostFactory::new(TestPlatform::new());
        let point = Point::from_str("guest").unwrap();
        let data = Arc::new(fs::read("../../wasm/my-app/my_app.wasm").unwrap());
        let host = factory.create(details, data).unwrap();
        let mut details = Details::default();
        details.stub.point = Point::from_str("host:guest").unwrap();
        let guest = details.stub.point.to_surface();
        host.init(details).unwrap();


        let mechtron = Details {
            stub: Stub {
                point: guest.point.push("mechtron").unwrap(),
                kind: Kind::Mechtron,
                status: Status::Ready
            },
            properties: Default::default()
        };
        let host_cmd = Assign::new( AssignmentKind::Create, mechtron.clone(), StateSrc::None ).to_host();
        let mut wave = DirectedProto::ping();
        wave.to(guest);
        wave.from(host.point().to_surface());
        wave.method(Method::Hyp(HypMethod::Host));
        wave.body(Substance::Hyper(HyperSubstance::Host(host_cmd)));
        let wave = wave.build().unwrap();
        let wave = wave.to_ultra();
        let reflect = host.route(wave).unwrap();
        let reflect = reflect.unwrap();
        let reflect = reflect.to_reflected().unwrap();

        reflect.success_or().unwrap();
    }
}
