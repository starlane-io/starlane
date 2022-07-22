#![cfg(test)]

use super::*;
use chrono::{DateTime, Utc};
use cosmic_api::id::id::{Layer, ToPoint, ToPort, Uuid};
use cosmic_api::msg::MsgMethod;
use cosmic_api::wave::{DirectedKind, DirectedProto, HyperWave};
use cosmic_api::{NoDiceArtifactFetcher, HYPERUSER};
use dashmap::DashMap;
use std::io::Error;
use std::sync::atomic;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use tokio::join;
use tokio::sync::{Mutex, oneshot};
use tokio::sync::mpsc::{Receiver, Sender};

lazy_static! {
    pub static ref LESS: Point = Point::from_str("space:users:less").expect("point");
    pub static ref FAE: Point = Point::from_str("space:users:fae").expect("point");
}

#[derive(Clone)]
pub struct TestPlatform {
    pub ctx: TestRegistryContext,
}

impl TestPlatform {
    pub fn new() -> Self {
        Self { ctx: TestRegistryContext::new() }
    }
}

#[async_trait]
impl Platform for TestPlatform {
    type Err = TestErr;
    type RegistryContext = TestRegistryContext;


    fn machine_template(&self) -> MachineTemplate {
        MachineTemplate::default()
    }

    fn machine_name(&self) -> MachineName {
        "test".to_string()
    }

    fn properties_config<K: ToBaseKind>(&self, base: &K) -> &'static PropertiesConfig {
        todo!()
    }

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder {
        DriversBuilder::new()
    }

    async fn global_registry(&self) -> Result<Registry<Self>,Self::Err> {
        Ok(Arc::new(TestRegistryApi::new(self.ctx.clone())))
    }

    async fn star_registry(
        &self,
        star: &StarKey
    ) -> Result<Registry<Self>, Self::Err> {
        todo!()
    }

    fn artifact_hub(&self) -> ArtifactApi {
        ArtifactApi::new(Arc::new(NoDiceArtifactFetcher::new()))
    }

    fn start_services(&self, entry_router: &mut HyperGateSelector) {}


}

#[derive(Clone)]
pub struct TestRegistryContext {
    pub sequence: Arc<AtomicU64>,
    pub particles: Arc<DashMap<Point, ParticleRecord>>,
}

impl TestRegistryContext {
    pub fn new() -> Self {
        Self {
            sequence: Arc::new(AtomicU64::new(0u64)),
            particles: Arc::new(DashMap::new()),
        }
    }
}

pub struct TestRegistryApi {
    ctx: TestRegistryContext,
}

impl TestRegistryApi {
    fn new(ctx: TestRegistryContext) -> Self {
        Self { ctx }
    }

    fn ctx(&self) -> &TestRegistryContext {
        &self.ctx
    }
}

#[async_trait]
impl RegistryApi<TestPlatform> for TestRegistryApi {
    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<Details, TestErr> {
        todo!()
    }

    async fn assign<'a>(&'a self, point: &'a Point, location: &'a Point) -> Result<(), TestErr> {
        todo!()
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), TestErr> {
        Ok(())
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), TestErr> {
        Ok(())
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, TestErr> {
        Ok(self.ctx.sequence.fetch_add(1, atomic::Ordering::Relaxed))
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, TestErr> {
        todo!()
    }

    async fn locate<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, TestErr> {
println!("registry locating:::> {}",point.to_string());
        Ok(self
            .ctx
            .particles
            .get(&point)
            .ok_or(TestErr::new("not found"))?
            .value()
            .clone())
    }

    async fn query<'a>(
        &'a self,
        point: &'a Point,
        query: &'a Query,
    ) -> Result<QueryResult, TestErr> {
        todo!()
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, TestErr> {
        todo!()
    }

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, TestErr> {
        todo!()
    }

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, TestErr> {
        todo!()
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), TestErr> {
        todo!()
    }

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, TestErr> {
        todo!()
    }

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), TestErr> {
        todo!()
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, TestErr> {
        todo!()
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), TestErr> {
        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct TestErr {
    pub message: String,
}

impl TestErr {
    pub fn new<S: ToString>(message: S) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl ToString for TestErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl Into<MsgErr> for TestErr {
    fn into(self) -> MsgErr {
        MsgErr::from_500(self.to_string())
    }
}

impl From<MsgErr> for TestErr {
    fn from(err: MsgErr) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl From<io::Error> for TestErr {
    fn from(err: Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

impl PlatErr for TestErr {
    fn to_cosmic_err(&self) -> MsgErr {
        MsgErr::from_500(self.to_string())
    }

    fn new<S>(message: S) -> Self
    where
        S: ToString,
    {
        Self {
            message: message.to_string(),
        }
    }

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString,
    {
        Self {
            message: message.to_string(),
        }
    }

    fn status(&self) -> u16 {
        500u16
    }
}

fn create(ctx: &TestRegistryContext, particle: Point, location: Point) {

println!("ADDING PARTICLE: {}",particle.to_string());
    ctx.particles.insert(
        particle.clone(),
        ParticleRecord::new(
            Details::new(
                Stub {
                    point: particle,
                    kind: Kind::Control,
                    status: Status::Ready,
                },
                Properties::new(),
            ),
            location
        ),
    );
}

#[test]
fn it_works() -> Result<(), TestErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let platform = TestPlatform::new();
        let machine_api = platform.machine();
        machine_api.wait_ready().await;

        let star_api = machine_api.get_machine_star().await.unwrap();
        let stub = star_api.stub().await.unwrap();
        let location = stub.key.clone().to_point();
        create(&platform.ctx, LESS.clone(), location.clone() );
        create(&platform.ctx, FAE.clone(), location.clone() );

        let record = platform.global_registry().await.unwrap().locate(&LESS).await.expect("IS LESS THERE?");
println!("location for LESS: {}", record.location.to_string());

        let skel = star_api.get_skel().await.unwrap();

        let mut to_fabric_rx = skel.diagnostic_interceptors.to_fabric.subscribe();
        let mut from_hyperway_rx = skel.diagnostic_interceptors.from_hyperway.subscribe();


        // send a 'nice' wave from Fae to Less
        let mut wave = DirectedProto::new();
        wave.kind(DirectedKind::Ping);
        wave.from(FAE.clone().to_port());
        wave.to(LESS.clone().to_port());
        wave.method(MsgMethod::new("DieTacEng").unwrap());
        let wave = wave.build().unwrap();
        let wave = wave.to_ultra();

        let (check_to_fabric_tx, check_to_fabric_rx):(oneshot::Sender<Result<(),()>>,oneshot::Receiver<Result<(),()>>) = oneshot::channel();
        let (check_from_hyperway_tx,check_from_hyperway_rx):(oneshot::Sender<Result<(),()>>,oneshot::Receiver<Result<(),()>>) = oneshot::channel();

        let wave_id = wave.id();
        {
            tokio::spawn(async move {
                while let Ok(hop) = from_hyperway_rx.recv().await {
                    let transport = hop.unwrap_from_hop().unwrap();
                    let wave = transport.unwrap_from_transport().unwrap();
                    if wave.id() == wave_id {
                        println!("intercepted from_hyperway event");
                        check_from_hyperway_tx.send(Ok(()));
                        break;
                    } else {
                        println!("RECEIVED WAVE: {}", wave.id().to_string())
                    }
                }
            });
        }

        let wave_id = wave.id();
        {
            tokio::spawn(async move {
                while let Ok(wave) = to_fabric_rx.recv().await {
                    if wave.id() == wave_id {
                        println!("intercepted to_fabric event!");
                        check_to_fabric_tx.send(Ok(()));
                        break;
                    } else {
                        println!("RECEIVED WAVE: {}", wave.id().to_string())
                    }
                }
            });
        }


        // send straight out of the star (circumvent layer traversal)
        star_api.to_fabric(wave).await;

        tokio::time::timeout(Duration::from_secs(5), check_from_hyperway_rx).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), check_to_fabric_rx).await.unwrap();

        Ok(())

    })

}