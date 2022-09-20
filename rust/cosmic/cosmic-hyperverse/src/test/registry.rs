use std::sync::{Arc, atomic};
use std::sync::atomic::AtomicU64;
use dashmap::DashMap;
use cosmic_universe::hyper::ParticleRecord;
use cosmic_universe::loc::Point;
use cosmic_universe::particle::{Details, Properties, Status, Stub};
use tokio::sync::oneshot;
use cosmic_universe::command::common::SetProperties;
use cosmic_universe::command::direct::delete::Delete;
use cosmic_universe::command::direct::query::{Query, QueryResult};
use cosmic_universe::command::direct::select::{Select, SubSelect};
use cosmic_universe::security::{Access, AccessGrant, IndexedAccessGrant};
use cosmic_universe::selector::Selector;
use cosmic_universe::substance::SubstanceList;
use crate::{Registration, RegistryApi};
use crate::test::hyperverse::TestHyperverse;
use crate::test::hyperverse::TestErr;

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
    pub fn new(ctx: TestRegistryContext) -> Self {
        Self { ctx }
    }

    fn ctx(&self) -> &TestRegistryContext {
        &self.ctx
    }
}

#[async_trait]
impl RegistryApi<TestHyperverse> for TestRegistryApi {
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

#[derive(Clone)]
pub struct TestRegistryContext {
    pub sequence: Arc<AtomicU64>,
    pub particles: Arc<DashMap<Point, ParticleRecord>>,
}
