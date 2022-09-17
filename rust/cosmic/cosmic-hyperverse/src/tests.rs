#![cfg(test)]

use super::*;
use crate::base::BaseDriverFactory;
use crate::control::{ControlClient, ControlCliSession, ControlDriverFactory};
use chrono::{DateTime, Utc};
use cosmic_universe::command::common::StateSrc;
use cosmic_universe::log::{LogSource, PointLogger, RootLogger, StdOutAppender};
use cosmic_universe::ext::ExtMethod;
use cosmic_universe::hyper::{Assign, AssignmentKind, HyperSubstance, InterchangeKind, Knock};
use cosmic_universe::wave::{
    Agent, CmdMethod, DirectedKind, DirectedProto, Exchanger, HyperWave, HypMethod, Method,
    Pong, ProtoTransmitterBuilder, Wave,
};
use cosmic_universe::HYPERUSER;
use cosmic_hyperlane::{AnonHyperAuthenticator, HyperClient, HyperConnectionDetails, HyperConnectionErr, HyperGate, HyperwayEndpoint, HyperwayStub, LocalHyperwayGateJumper};
use dashmap::DashMap;
use std::io::Error;
use std::sync::atomic;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use tokio::join;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{Mutex, oneshot};
use tokio::time::error::Elapsed;
use cosmic_universe::artifact::NoDiceArtifactFetcher;
use cosmic_universe::command::request::create::{Create, PointSegTemplate, PointTemplate, Strategy, Template};
use cosmic_universe::id::{Layer, StarHandle, ToPoint, ToPort, TraversalDirection, Uuid};
use cosmic_universe::mount::MountKind;
//use crate::control::ControlDriverFactory;
use crate::driver::{DriverAvail, DriverFactory};
use crate::root::RootDriverFactory;
use crate::space::SpaceDriverFactory;
use crate::star::HyperStarApi;

lazy_static! {
    pub static ref LESS: Point = Point::from_str("space:users:less").expect("point");
    pub static ref FAE: Point = Point::from_str("space:users:fae").expect("point");
}

lazy_static! {
    pub static ref PROPERTIES_CONFIG: PropertiesConfig = PropertiesConfig::new();
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
        &PROPERTIES_CONFIG
    }

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder<Self> {
        let mut builder = DriversBuilder::new(kind.clone());

        // only allow external Base wrangling external to Super
        if *kind == StarSub::Super {
            builder.add_post(Arc::new(BaseDriverFactory::new(DriverAvail::External)));
        } else {
            builder.add_post(Arc::new(BaseDriverFactory::new(DriverAvail::Internal)));
        }

        match kind {
            StarSub::Central => {
                builder.add_post(Arc::new(RootDriverFactory::new()));
            }
            StarSub::Super => {
                builder.add_post(Arc::new(SpaceDriverFactory::new()));
            }
            StarSub::Nexus => {}
            StarSub::Maelstrom => {}
            StarSub::Scribe => {}
            StarSub::Jump => {
                builder.add_post(Arc::new(ControlDriverFactory::new()));
            }
            StarSub::Fold => {}
            StarSub::Machine => {
                builder.add_post(Arc::new(ControlDriverFactory::new()));
            }
        }

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

    fn start_services(&self, gate: &Arc<dyn HyperGate>) {}
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
                status: Status::Pending,
            },
            properties: Default::default(),
        };
        let record = ParticleRecord {
            details: details.clone(),
            location: None,
        };
        self.ctx
            .particles
            .insert(registration.point.clone(), record);
        Ok(details)
    }

    fn assign<'a>(&'a self, point: &'a Point) -> oneshot::Sender<Point> {
        let (rtn, mut rtn_rx) = oneshot::channel();
        let ctx = self.ctx.clone();
        let point = point.clone();
        tokio::spawn(async move {
            match rtn_rx.await {
                Ok(location) => {
                    let mut record = ctx.particles.get_mut(&point).unwrap();

                    let location: Point = location;
                    record.value_mut().location = Some(location);
                }
                Err(_) => {
                    // hopefully logged elsewhere
                }
            }
        });

        rtn
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
        Ok(Default::default())
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, TestErr> {
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

impl Into<UniErr> for TestErr {
    fn into(self) -> UniErr {
        UniErr::from_500(self.to_string())
    }
}

impl From<oneshot::error::RecvError> for TestErr {
    fn from(err: RecvError) -> Self {
        TestErr {
            message: err.to_string(),
        }
    }
}

impl From<Elapsed> for TestErr {
    fn from(err: Elapsed) -> Self {
        TestErr {
            message: err.to_string(),
        }
    }
}


impl From<String> for TestErr {
    fn from(err: String) -> Self {
        TestErr { message: err }
    }
}

impl From<&'static str> for TestErr {
    fn from(err: &'static str) -> Self {
        TestErr {
            message: err.to_string(),
        }
    }
}
impl From<UniErr> for TestErr {
    fn from(err: UniErr) -> Self {
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
    fn to_cosmic_err(&self) -> UniErr {
        UniErr::from_500(self.to_string())
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
    star_api: HyperStarApi<TestPlatform>,
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
    wave.to(star_api.get_skel().await?.point.clone().to_port());
    wave.from(HYPERUSER.clone());
    wave.agent(Agent::HyperUser);
    wave.method(HypMethod::Assign);
    wave.body(Substance::Hyper(HyperSubstance::Assign(Assign::new(
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
        wave.from(FAE.clone().to_port());
        wave.to(LESS.clone().to_port());
        wave.method(ExtMethod::new("DieTacEng").unwrap());
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

pub struct MachineApiExtFactory<P>
    where
        P: Platform,
{
    machine_api: MachineApi<P>,
    logger: PointLogger,
}

#[async_trait]
impl<P> HyperwayEndpointFactory for MachineApiExtFactory<P>
    where
        P: Platform,
{
    async fn create(&self, status_tx:mpsc::Sender<HyperConnectionDetails>) -> Result<HyperwayEndpoint, UniErr> {
        let knock = Knock {
            kind: InterchangeKind::DefaultControl,
            auth: Box::new(Substance::Empty),
            remote: None,
        };
        self.logger
            .result_ctx("machine_api.knock()", self.machine_api.knock(knock).await)
    }
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
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("test-client").unwrap());

        tokio::time::timeout(Duration::from_secs(10), machine_api.wait_ready())
            .await
            .unwrap();

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let exchanger = Exchanger::new( Point::from_str("client").unwrap().to_port(), Timeouts::default());
        let client = HyperClient::new_with_exchanger(Box::new(factory), Some(exchanger), logger).unwrap();
        let transmitter = client.transmitter_builder().await?;
        let greet = client.get_greeting().expect("expected greeting");
        let transmitter = transmitter.build();

        {
            let transmitter = transmitter.clone();
            let mut rx = client.rx();
            tokio::spawn(async move {
                while let Ok(wave) = rx.recv().await {
                    if wave.is_directed() {
                        let directed = wave.to_directed().unwrap();
                        if directed.core().method == Method::Cmd(CmdMethod::Bounce) {
                            let reflection = directed.reflection().unwrap();
                            let reflect = reflection.make(ReflectedCore::ok(), greet.port.clone());
                            let wave = reflect.to_ultra();
                            transmitter.route(wave).await;
                        }
                    }
                }
            });
        }

        let mut bounce = DirectedProto::cmd(
            greet.transport.clone().with_layer(Layer::Shell),
            CmdMethod::Bounce,
        );
        let reflect: Wave<Pong> = transmitter.direct(bounce).await?;

        assert!(reflect.core.status.is_success());

        client.close().await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    })
}

#[test]
fn test_star_wrangle() -> Result<(), TestErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = TestPlatform::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("test-client").unwrap());

        tokio::time::timeout(Duration::from_secs(1), machine_api.wait_ready())
            .await
            .unwrap();

        let star_api = machine_api.get_machine_star().await?;

        let wrangles = tokio::time::timeout(Duration::from_secs(55), star_api.wrangle()).await??;

        println!("wrangles: {}", wrangles.wrangles.len());

        for kind in wrangles.wrangles.iter()
        {
            println!("\tkind: {}", kind.kind.to_string() );
        }

        Ok(())
    })
}


#[test]
fn test_golden_path() -> Result<(), TestErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = TestPlatform::new();
        let machine_api = platform.machine();

        tokio::time::timeout(Duration::from_secs(1), machine_api.wait_ready())
            .await
            .unwrap();

        let fold = StarKey::new( &"central".to_string(), &StarHandle::new( "fold", 0));
        let star_api = machine_api.get_star(fold).await?;

        // first test if we can bounce nexus which fold should be directly connected too
        let nexus = StarKey::new( &"central".to_string(), &StarHandle::new( "nexus", 0));
        tokio::time::timeout(Duration::from_secs(5), star_api.bounce(nexus)).await??;
        println!("Ok");

        // this one should require a search operation in order to find
        tokio::time::timeout(Duration::from_secs(5), star_api.bounce(StarKey::central())).await??;

        println!("Ok");

        Ok(())

    })
}

#[test]
fn test_provision_and_assign() -> Result<(), TestErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = TestPlatform::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("test-client").unwrap());

        tokio::time::timeout(Duration::from_secs(1), machine_api.wait_ready())
            .await
            .unwrap();

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let exchanger = Exchanger::new( Point::from_str("client").unwrap().to_port(), Timeouts::default());
        let client = HyperClient::new_with_exchanger(Box::new(factory), Some(exchanger), logger).unwrap();
        let transmitter = client.transmitter_builder().await?;
        let transmitter = transmitter.build();

        let mut proto = DirectedProto::ping();
        proto.method(CmdMethod::Bounce);
        proto.to(Point::root().to_port());
        let reflect: Wave<Pong> = transmitter.direct(proto).await?;
        println!("{}",reflect.core.status.to_string());
        assert!(reflect.core.is_ok());

        let create = Create {
            template: Template::new(PointTemplate { parent: Point::root(), child_segment_template: PointSegTemplate::Exact("my-domain.com".to_string()) }, Kind::Space.to_template() ),
            properties: Default::default(),
            strategy: Strategy::Override,
            state: StateSrc::None,
        };
        let proto : DirectedProto = create.into();
        let reflect: Wave<Pong> = transmitter.direct(proto).await?;
        println!("{}",reflect.core.status.to_string());
        assert!(reflect.core.is_ok());

        tokio::time::sleep(Duration::from_secs(5)).await;

        let point = Point::from_str("my-domain.com")?;
        let mut proto = DirectedProto::ping();
        proto.method(CmdMethod::Bounce);
        proto.to(point.to_port());
        let reflect: Wave<Pong> = transmitter.direct(proto).await?;
        println!("{}",reflect.core.status.to_string());
        assert!(reflect.core.is_ok());

        Ok(())

    })
}

#[test]
fn test_control_cli() -> Result<(), TestErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        // let (final_tx, final_rx) = oneshot::channel();

        let platform = TestPlatform::new();
        let machine_api = platform.machine();
        let logger = RootLogger::new(LogSource::Core, Arc::new(StdOutAppender()));
        let logger = logger.point(Point::from_str("test-client").unwrap());

        tokio::time::timeout(Duration::from_secs(1), machine_api.wait_ready())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_secs(5)).await;

        let factory = MachineApiExtFactory {
            machine_api,
            logger: logger.clone(),
        };

        let client = ControlClient::new(Box::new(factory))?;
        client.wait_for_ready(Duration::from_secs(5)).await?;

        let cli = client.new_cli_session().await?;

        let core = cli.exec("create localhost<Space>").await?;

        println!("{}",core.to_err().to_string());
        assert!(core.is_ok());

        Ok(())

    })
}




