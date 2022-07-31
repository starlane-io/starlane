#![cfg(test)]

use super::*;
use chrono::{DateTime, Utc};
use cosmic_api::command::command::common::StateSrc;
use cosmic_api::id::id::{Layer, ToPoint, ToPort, Uuid};
use cosmic_api::id::TraversalDirection;
use cosmic_api::msg::MsgMethod;
use cosmic_api::sys::{Assign, AssignmentKind, Sys};
use cosmic_api::wave::{Agent, CmdMethod, DirectedKind, DirectedProto, HyperWave, SysMethod};
use cosmic_api::{MountKind, NoDiceArtifactFetcher, HYPERUSER};
use cosmic_hyperlane::{AnonHyperAuthenticator, LocalHyperwayGateJumper};
use dashmap::DashMap;
use std::io::Error;
use std::sync::atomic;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use tokio::join;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{oneshot, Mutex};
//use crate::control::ControlDriverFactory;
use crate::driver::DriverFactory;
use crate::star::StarApi;

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
        Self {
            ctx: TestRegistryContext::new(),
        }
    }
}

#[async_trait]
impl Platform for TestPlatform {
    type Err = TestErr;
    type RegistryContext = TestRegistryContext;
    type StarAuth = AnonHyperAuthenticator;
    type RemoteStarConnectionFactory = LocalHyperwayGateJumper;

    fn star_auth(&self, star: &StarKey) -> Result<Self::StarAuth, Self::Err> {
        Ok(AnonHyperAuthenticator::new())
    }

    fn remote_connection_factory_for_star(
        &self,
        star: &StarKey,
    ) -> Result<Self::RemoteStarConnectionFactory, Self::Err> {
        todo!()
    }

    fn machine_template(&self) -> MachineTemplate {
        MachineTemplate::default()
    }

    fn machine_name(&self) -> MachineName {
        "test".to_string()
    }

    fn properties_config<K: ToBaseKind>(&self, base: &K) -> &'static PropertiesConfig {
        todo!()
    }

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder<Self> {
        let mut builder = DriversBuilder::new(kind.clone());
        // builder.add( Box::new(ControlDriverFactory::new()));
        builder
    }

    async fn global_registry(&self) -> Result<Registry<Self>, Self::Err> {
        Ok(Arc::new(TestRegistryApi::new(self.ctx.clone())))
    }

    async fn star_registry(&self, star: &StarKey) -> Result<Registry<Self>, Self::Err> {
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
        let details = Details {
            stub: Stub {
                point: registration.point.clone(),
                kind: registration.kind.clone(),
                status: Status::Unknown,
            },
            properties: Default::default(),
        };
        let record = ParticleRecord {
            details: details.clone(),
            location: Point::root(),
        };
        self.ctx
            .particles
            .insert(registration.point.clone(), record);
        Ok(details)
    }

    async fn assign<'a>(&'a self, point: &'a Point, location: &'a Point) -> Result<(), TestErr> {
        let mut record = self
            .ctx
            .particles
            .get_mut(point)
            .ok_or(TestErr::new(format!("not found: {}", point.to_string())))?;
        record.value_mut().location = point.clone();
        Ok(())
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), TestErr> {
        let mut record = self
            .ctx
            .particles
            .get_mut(point)
            .ok_or(TestErr::new(format!("not found: {}", point.to_string())))?;
        record.value_mut().details.stub.status = status.clone();
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
        Ok(Access::Super)
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

impl From<oneshot::error::RecvError> for TestErr {
    fn from(err: RecvError) -> Self {
        TestErr {
            message: err.to_string(),
        }
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

async fn create(
    ctx: &TestRegistryContext,
    particle: Point,
    location: Point,
    star_api: StarApi<TestPlatform>,
) -> Result<(), TestErr> {
    println!("ADDING PARTICLE: {}", particle.to_string());
    let details = Details::new(
        Stub {
            point: particle.clone(),
            kind: Kind::Control,
            status: Status::Ready,
        },
        Properties::new(),
    );
    ctx.particles.insert(
        particle.clone(),
        ParticleRecord::new(details.clone(), location),
    );

    let mut wave = DirectedProto::ping();
    wave.kind(DirectedKind::Ping);
    wave.to(star_api.get_skel().await?.point.clone().to_port());
    wave.from(HYPERUSER.clone());
    wave.agent(Agent::HyperUser);
    wave.method(SysMethod::Assign);
    wave.body(Substance::Sys(Sys::Assign(Assign::new(
        AssignmentKind::Create,
        details,
        StateSrc::None,
    ))));
    let wave = wave.build()?;
    let wave = wave.to_ultra();
    star_api.to_gravity(wave).await;
    Ok(())
}

#[test]
fn test_gravity_routing() -> Result<(), TestErr> {
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

        //        let record = platform.global_registry().await.unwrap().locate(&LESS).await.expect("IS LESS THERE?");

        let skel = star_api.get_skel().await.unwrap();

        let mut assign_rx = skel.diagnostic_interceptors.assignment.subscribe();
        let (assign_rtn_tx, assign_rtn_rx) = oneshot::channel();

        tokio::spawn(async move {
            assign_rx.recv().await;
            assign_rx.recv().await;
            assign_rtn_tx.send(());
        });

        create(
            &platform.ctx,
            LESS.clone(),
            location.clone(),
            star_api.clone(),
        )
        .await?;
        create(
            &platform.ctx,
            FAE.clone(),
            location.clone(),
            star_api.clone(),
        )
        .await?;

        tokio::time::timeout(Duration::from_secs(5), assign_rtn_rx).await;

        panic!("far enough");

        let mut to_fabric_rx = skel.diagnostic_interceptors.to_gravity.subscribe();
        let mut from_hyperway_rx = skel.diagnostic_interceptors.from_hyperway.subscribe();

        // send a 'nice' wave from Fae to Less
        let mut wave = DirectedProto::ping();
        wave.kind(DirectedKind::Ping);
        wave.from(FAE.clone().to_port());
        wave.to(LESS.clone().to_port());
        wave.method(MsgMethod::new("DieTacEng").unwrap());
        let wave = wave.build().unwrap();
        let wave = wave.to_ultra();

        let (check_to_fabric_tx, check_to_fabric_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();
        let (check_from_hyperway_tx, check_from_hyperway_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();

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
        star_api.to_gravity(wave).await;

        tokio::time::timeout(Duration::from_secs(5), check_from_hyperway_rx)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), check_to_fabric_rx)
            .await
            .unwrap()
            .unwrap()
            .unwrap();

        Ok(())
    })
}
#[test]
fn test_layer_traversal() -> Result<(), TestErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let (check_to_gravity_tx, check_to_gravity_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();
        let (check_from_hyperway_tx, check_from_hyperway_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();
        let (check_start_traversal_wave_tx, check_start_traversal_wave_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();
        let (check_start_traversal_tx, check_start_traversal_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();
        let (check_transport_endpoint_tx, check_transport_endpoint_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();

        let (direct_tx, direct_rx) = oneshot::channel();

        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(5), check_from_hyperway_rx)
                .await
                .expect("check_from_hyperway")
                .unwrap()
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), check_to_gravity_rx)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), check_start_traversal_wave_rx)
                .await
                .expect("check_start_traversal_wave")
                .unwrap()
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), check_start_traversal_rx)
                .await
                .expect("check_start_traversal")
                .unwrap()
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), check_transport_endpoint_rx)
                .await
                .expect("check_transport_endpoint")
                .unwrap()
                .unwrap();

            direct_tx.send(());
        });

        let platform = TestPlatform::new();
        let machine_api = platform.machine();
        machine_api.wait_ready().await;

        let star_api = machine_api.get_machine_star().await.unwrap();
        let stub = star_api.stub().await.unwrap();
        let location = stub.key.clone().to_point();

        //        let record = platform.global_registry().await.unwrap().locate(&LESS).await.expect("IS LESS THERE?");

        let skel = star_api.get_skel().await.unwrap();

        let mut assign_rx = skel.diagnostic_interceptors.assignment.subscribe();
        let (assign_rtn_tx, assign_rtn_rx) = oneshot::channel();

        tokio::spawn(async move {
            assign_rx.recv().await;
            assign_rx.recv().await;
            assign_rtn_tx.send(());
        });

        create(
            &platform.ctx,
            LESS.clone(),
            location.clone(),
            star_api.clone(),
        )
        .await?;
        create(
            &platform.ctx,
            FAE.clone(),
            location.clone(),
            star_api.clone(),
        )
        .await?;

        tokio::time::timeout(Duration::from_secs(5), assign_rtn_rx)
            .await
            .unwrap();

        let mut to_gravity_rx = skel.diagnostic_interceptors.to_gravity.subscribe();
        let mut from_hyperway_rx = skel.diagnostic_interceptors.from_hyperway.subscribe();
        let mut start_layer_traversal = skel
            .diagnostic_interceptors
            .start_layer_traversal
            .subscribe();
        let mut start_layer_traversal_wave = skel
            .diagnostic_interceptors
            .start_layer_traversal_wave
            .subscribe();
        let mut transport_endpoint = skel.diagnostic_interceptors.transport_endpoint.subscribe();
        let mut reflected_endpoint = skel.diagnostic_interceptors.reflected_endpoint.subscribe();

        // send a 'nice' wave from Fae to Less
        let mut wave = DirectedProto::ping();
        wave.kind(DirectedKind::Ping);
        wave.from(FAE.clone().to_port());
        wave.to(LESS.clone().to_port());
        wave.method(CmdMethod::Bounce);
        let wave = wave.build().unwrap();
        let wave = wave.to_ultra();

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
                while let Ok(wave) = to_gravity_rx.recv().await {
                    if wave.id() == wave_id {
                        println!("intercepted to_fabric event!");
                        check_to_gravity_tx.send(Ok(()));
                        break;
                    } else {
                        println!("to_gravity RECEIVED WAVE: {}", wave.id().to_string())
                    }
                }
            });
        }

        let wave_id = wave.id();
        {
            tokio::spawn(async move {
                while let Ok(transport) = start_layer_traversal_wave.recv().await {
                    if let Ok(wave) = transport.clone().unwrap_from_transport() {
                        if wave.id() == wave_id {
                            println!("intercepted start_layer_traversal_wave !");
                            check_start_traversal_wave_tx.send(Ok(()));
                            break;
                        } else {
                            println!(
                                "start_layer_traversal_wave RECEIVED WAVE: {}",
                                wave.id().to_string()
                            )
                        }
                    } else {
                        println!(
                            "start_layer_traversal_wave RECEIVED TRANSPORT: {}",
                            transport.id().to_string()
                        )
                    }
                }
            });
        }

        let wave_id = wave.id();
        {
            tokio::spawn(async move {
                while let Ok(traversal) = start_layer_traversal.recv().await {
                    let transport = traversal.payload;
                    match transport.clone().unwrap_from_transport() {
                        Ok(wave) => {
                            if wave.id() == wave_id {
                                println!("intercepted start_layer_traversal!");
                                if traversal.dir != TraversalDirection::Core {
                                    println!("Bad Traversal Direction");
                                    check_start_traversal_tx.send(Err(()));
                                } else if traversal.dest.is_some() {
                                    println!("Bad Traversal Dest ");
                                    check_start_traversal_tx.send(Err(()));
                                } else if traversal.layer != Layer::Field {
                                    println!("Bad Traversal Layer");
                                    check_start_traversal_tx.send(Err(()));
                                } else {
                                    println!("traversal layer {}", traversal.layer.to_string());
                                    check_start_traversal_tx.send(Ok(()));
                                }
                                break;
                            } else {
                                println!(
                                    "intercepted start_layer_traversal RECEIVED WAVE: {}",
                                    wave.id().to_string()
                                )
                            }
                        }
                        Err(_) => {
                            println!(
                                "intercepted start_layer_traversal RECEIVED TRANSPORT: {}",
                                transport.id().to_string()
                            )
                        }
                    }
                }
            });
        }

        let wave_id = wave.id();
        {
            tokio::spawn(async move {
                while let Ok(transport) = transport_endpoint.recv().await {
                    if let Ok(wave) = transport.clone().unwrap_from_transport() {
                        if wave.id() == wave_id {
                            println!("intercepted transport_endpoint!");
                            check_transport_endpoint_tx.send(Ok(()));
                            break;
                        } else {
                            println!(
                                "transport_endpoint RECEIVED WAVE: {}",
                                wave.id().to_string()
                            )
                        }
                    } else {
                        println!(
                            "transport_endpoint RECEIVED TRANSPORT: {}",
                            transport.id().to_string()
                        )
                    }
                }
            });
        }

        // send straight out of the star (circumvent layer traversal)
        star_api.to_gravity(wave).await;

        let mut to_gravity_rx = skel.diagnostic_interceptors.to_gravity.subscribe();
        let wave = tokio::time::timeout(Duration::from_secs(5), reflected_endpoint.recv())
            .await
            .expect("reflected_endpoint")
            .expect("reflected_endpoint");

        let (check_to_gravity_tx, check_to_gravity_rx): (
            oneshot::Sender<Result<(), ()>>,
            oneshot::Receiver<Result<(), ()>>,
        ) = oneshot::channel();
        let wave_id = wave.id();
        {
            tokio::spawn(async move {
                while let Ok(wave) = to_gravity_rx.recv().await {
                    if wave.id() == wave_id {
                        println!("intercepted to_gravity reflection event");
                        check_to_gravity_tx.send(Ok(()));
                        break;
                    } else {
                        println!("RECEIVED WAVE: {}", wave.id().to_string())
                    }
                }
            });
        }

        tokio::time::timeout(Duration::from_secs(5), check_to_gravity_rx)
            .await
            .expect("check_to_gravity_rx")
            .expect("check_to_gravity_rx");

        Ok(())
    })
}

#[test]
fn test_control() -> Result<(), TestErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = TestPlatform::new();
        let machine_api = platform.machine();
        tokio::time::timeout(Duration::from_secs(10), machine_api.wait_ready())
            .await
            .unwrap();
        //tokio::time::timeout( Duration::from_secs(15), machine_api.wait_ready()).await;
        //tokio::time::timeout( Duration::from_secs(15), machine_api.wait_ready()).await;
        //        machine_api.wait_ready().await;

        let star_api = machine_api.get_machine_star().await.unwrap();

        // tokio::time::sleep(Duration::from_secs(8)).await;
        /*
        let less = star_api
            .create_mount(Agent::HyperUser, MountKind::Control)
            .await
            .unwrap();
        let fae = star_api
            .create_mount(Agent::HyperUser, MountKind::Control)
            .await
            .unwrap();

         */

        // final_rx.await;

        //let stub = star_api.stub().await.unwrap();
        Ok(())
    })
}
