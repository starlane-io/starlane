#![cfg(test)]

use std::io::Error;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use tokio::sync::Mutex;
use cosmic_api::id::id::Uuid;
use cosmic_api::NoDiceArtifactFetcher;
use super::*;



#[derive(Clone)]
pub struct TestPlatform {

}

impl TestPlatform {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Platform for TestPlatform {
    type Err = TestErr;
    type RegistryContext = TestRegistryContext;

    async fn create_registry_context(&self, stars: HashSet<StarKey>) -> Result<Self::RegistryContext, Self::Err> {
        Ok(TestRegistryContext::new())
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

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder {
        DriversBuilder::new()
    }

    fn token(&self) -> Token {
        Token::new("__token__")
    }

    async fn global_registry(&self, ctx: Arc<Self::RegistryContext>) -> Result<Registry<Self>, Self::Err> {
        Ok(Arc::new(TestRegistryApi::new(ctx)))
    }

    async fn star_registry(&self, star: &StarKey, ctx: Arc<Self::RegistryContext>) -> Result<Registry<Self>, Self::Err> {
        todo!()
    }

    fn artifact_hub(&self) -> ArtifactApi {
        ArtifactApi::new( Arc::new(NoDiceArtifactFetcher::new()) )
    }

    fn start_services(&self, entry_router: &mut InterchangeEntryRouter) {
    }
}

pub struct TestRegistryContext {
   pub sequence: AtomicU64,
   pub particles: DashMap<Point,Details>
}

impl TestRegistryContext {
    pub fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0u64),
            particles: DashMap::new()
        }
    }
}


pub struct TestRegistryApi {
    context: Arc<TestRegistryContext>
}

impl TestRegistryApi {

    pub fn new( context: Arc<TestRegistryContext> ) -> Self {
        Self {
            context
        }
    }
}

#[async_trait]
impl RegistryApi<TestPlatform> for TestRegistryApi where{
    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<Details, TestErr> {
        todo!()
    }

    async fn assign<'a>(&'a self, point: &'a Point, location: &'a Point) -> Result<(), TestErr> {
        todo!()
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), TestErr> {
        Ok(())
    }

    async fn set_properties<'a>(&'a self, point: &'a Point, properties: &'a SetProperties) -> Result<(), TestErr> {
        Ok(())
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, TestErr> {
        todo!()
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, TestErr> {
        todo!()
    }

    async fn locate<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, TestErr> {
        todo!()
    }

    async fn query<'a>(&'a self, point: &'a Point, query: &'a Query) -> Result<QueryResult, TestErr> {
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

    async fn chown<'a>(&'a self, on: &'a Selector, owner: &'a Point, by: &'a Point) -> Result<(), TestErr> {
        todo!()
    }

    async fn list_access<'a>(&'a self, to: &'a Option<&'a Point>, on: &'a Selector) -> Result<Vec<IndexedAccessGrant>, TestErr> {
        todo!()
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), TestErr> {
        todo!()
    }
}


#[derive(Debug,Clone)]
pub struct TestErr {
    pub message: String
}

impl TestErr {
    pub fn new<S:ToString>(message: S) -> Self {
        Self {
            message: message.to_string()
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
            message: err.to_string()
        }
    }
}

impl From<io::Error> for TestErr {
    fn from(err: Error) -> Self {
        Self {
            message: err.to_string()
        }
    }
}

impl PlatErr for TestErr {
    fn to_cosmic_err(&self) -> MsgErr {
        MsgErr::from_500(self.to_string())
    }

    fn new<S>(message: S) -> Self where S: ToString {
        Self {
            message: message.to_string()
        }
    }

    fn status_msg<S>(status: u16, message: S) -> Self where S: ToString {
        Self {
            message: message.to_string()
        }
    }

    fn status(&self) -> u16 {
        500u16
    }
}


#[test]
fn it_works() -> Result<(),TestErr>{

    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    runtime.block_on( async move {
        let platform = TestPlatform::new();
        let machine_api = platform.create();

        {
            let machine_api = machine_api.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(5)).await;
                machine_api.terminate();
            });
        }

        machine_api.wait().await;
    });

    Ok(())

}
