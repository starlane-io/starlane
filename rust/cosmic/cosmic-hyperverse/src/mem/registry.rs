use crate::mem::cosmos::MemCosmos;
use crate::err::CosmicErr;
use crate::{Registration, RegistryApi};
use cosmic_universe::command::common::{PropertyMod, SetProperties};
use cosmic_universe::command::direct::delete::Delete;
use cosmic_universe::command::direct::query::{Query, QueryResult};
use cosmic_universe::command::direct::select::{Select, SubSelect};
use cosmic_universe::hyper::{ParticleLocation, ParticleRecord};
use cosmic_universe::loc::Point;
use cosmic_universe::particle::{Details, Properties, Property, Status, Stub};
use cosmic_universe::security::{Access, AccessGrant, IndexedAccessGrant};
use cosmic_universe::selector::Selector;
use cosmic_universe::substance::SubstanceList;
use dashmap::DashMap;
use std::sync::atomic::AtomicU64;
use std::sync::{atomic, Arc};
use dashmap::mapref::one::Ref;
use tokio::sync::oneshot;
use cosmic_universe::parse::get_properties;

impl MemRegCtx {
    pub fn new() -> Self {
        Self {
            sequence: Arc::new(AtomicU64::new(0u64)),
            particles: Arc::new(DashMap::new()),
            properties: Arc::new( DashMap::new() )
        }
    }
}

#[derive(Clone)]
pub struct MemRegCtx {
    pub sequence: Arc<AtomicU64>,
    pub particles: Arc<DashMap<Point, ParticleRecord>>,
    pub properties: Arc<DashMap<Point, Properties>>,
}

pub struct MemRegApi {
    ctx: MemRegCtx,
}

impl MemRegApi {
    pub fn new(ctx: MemRegCtx) -> Self {
        Self { ctx }
    }

    fn ctx(&self) -> &MemRegCtx {
        &self.ctx
    }
}

#[async_trait]
impl RegistryApi<MemCosmos> for MemRegApi {
    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<Details, CosmicErr> {
        self.set_properties(&registration.point, &registration.properties).await?;

        let details = Details {
            stub: Stub {
                point: registration.point.clone(),
                kind: registration.kind.clone(),
                status: Status::Pending,
            },
            properties: self.get_properties(&registration.point).await?
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

    async fn assign<'a>(
        &'a self,
        point: &'a Point,
        location: ParticleLocation,
    ) -> Result<(), CosmicErr> {
        let mut record = self.ctx.particles.get_mut(&point).unwrap();
        record.value_mut().location = Some(location);
        Ok(())
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), CosmicErr> {
        let mut record = self
            .ctx
            .particles
            .get_mut(point)
            .ok_or(CosmicErr::new(format!("not found: {}", point.to_string())))?;
        record.value_mut().details.stub.status = status.clone();
        Ok(())
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), CosmicErr> {
        let mut rtn= Properties::new();
        for (id,property) in properties.iter() {
            match property {
                PropertyMod::Set { key, value, lock } => {
                    let property = Property{
                        key: key.clone(),
                        value: value.clone(),
                        locked: lock.clone()
                    };
                    rtn.insert(id.clone(), property );
                }
                PropertyMod::UnSet(_) => {}
            }
        }
        self.ctx.properties.insert( point.clone(), rtn );
        Ok(())
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, CosmicErr> {
        Ok(self.ctx.sequence.fetch_add(1, atomic::Ordering::Relaxed))
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, CosmicErr> {
        match self.ctx.properties.get( point) {
            None => Ok(Default::default()),
            Some(mul) =>  Ok(mul.value().clone())
        }
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, CosmicErr> {
        let properties = self.get_properties(point).await?;
        let mut record = self
            .ctx
            .particles
            .get(&point)
            .ok_or(CosmicErr::new("not found"))?
            .value()
            .clone();
        record.details.properties = properties;
        Ok(record)
    }

    async fn query<'a>(
        &'a self,
        point: &'a Point,
        query: &'a Query,
    ) -> Result<QueryResult, CosmicErr> {
        todo!()
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, CosmicErr> {
        todo!()
    }

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, CosmicErr> {
        todo!()
    }

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, CosmicErr> {
        todo!()
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), CosmicErr> {
        todo!()
    }

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, CosmicErr> {
        Ok(Access::Super)
    }

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), CosmicErr> {
        todo!()
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, CosmicErr> {
        todo!()
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), CosmicErr> {
        todo!()
    }
}

