use crate::reg::{Registration, RegistryApi};
use crate::registry::err::RegErr;
use async_trait::async_trait;
use dashmap::DashMap;
use space::command::common::{PropertyMod, SetProperties};
use space::command::direct::delete::Delete;
use space::command::direct::query::{Query, QueryResult};
use space::command::direct::select::SubSelect;
use space::hyper::{ParticleLocation, ParticleRecord};
use space::particle::{Details, Properties, Property, Status, Stub};
use space::point::Point;
use space::security::{Access, AccessGrant, IndexedAccessGrant};
use space::selector::Selector;
use space::substance::SubstanceList;
use std::sync::atomic::AtomicU64;
use std::sync::{atomic, Arc};

impl MemoryRegistryCtx {
    pub fn new() -> Self {
        Self {
            sequence: Arc::new(AtomicU64::new(0u64)),
            particles: Arc::new(DashMap::new()),
            properties: Arc::new(DashMap::new()),
        }
    }
}

#[derive(Clone)]
pub struct MemoryRegistryCtx {
    pub sequence: Arc<AtomicU64>,
    pub particles: Arc<DashMap<Point, ParticleRecord>>,
    pub properties: Arc<DashMap<Point, Properties>>,
}

pub struct MemoryRegistry {
    ctx: MemoryRegistryCtx,
}

impl MemoryRegistry {
    pub fn new() -> Self {
        let ctx = MemoryRegistryCtx::new();
        Self { ctx }
    }

    fn ctx(&self) -> &MemoryRegistryCtx {
        &self.ctx
    }
}

#[async_trait]
impl RegistryApi for MemoryRegistry {
    async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
        Ok(())
    }

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
        self.set_properties(&registration.point, &registration.properties)
            .await?;

        let details = Details {
            stub: Stub {
                point: registration.point.clone(),
                kind: registration.kind.clone(),
                status: Status::Pending,
            },
            properties: self.get_properties(&registration.point).await?,
        };
        let record = ParticleRecord {
            details: details.clone(),
            location: ParticleLocation::default(),
        };
        self.ctx
            .particles
            .insert(registration.point.clone(), record);
        Ok(())
    }

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr> {
        let mut record = self.ctx.particles.get_mut(&point).unwrap();
        record.value_mut().location.star = Some(star.clone());
        Ok(())
    }

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr> {
        let mut record = self.ctx.particles.get_mut(&point).unwrap();
        record.value_mut().location.host = Some(host.clone());
        Ok(())
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr> {
        let mut record = self
            .ctx
            .particles
            .get_mut(point)
            .ok_or(RegErr::NotFound(point.clone()))?;
        record.value_mut().details.stub.status = status.clone();
        Ok(())
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr> {
        let mut rtn = Properties::new();
        for (id, property) in properties.iter() {
            match property {
                PropertyMod::Set { key, value, lock } => {
                    let property = Property {
                        key: key.clone(),
                        value: value.clone(),
                        locked: lock.clone(),
                    };
                    rtn.insert(id.clone(), property);
                }
                PropertyMod::UnSet(_) => {}
            }
        }
        self.ctx.properties.insert(point.clone(), rtn);
        Ok(())
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
        Ok(self.ctx.sequence.fetch_add(1, atomic::Ordering::Relaxed))
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
        match self.ctx.properties.get(point) {
            None => Ok(Default::default()),
            Some(mul) => Ok(mul.value().clone()),
        }
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
        let properties = self.get_properties(point).await?;
        let mut record = self
            .ctx
            .particles
            .get(&point)
            .ok_or(RegErr::NotFound(point.clone()))?
            .value()
            .clone();
        record.details.properties = properties;
        Ok(record)
    }

    async fn query<'a>(
        &'a self,
        point: &'a Point,
        query: &'a Query,
    ) -> Result<QueryResult, RegErr> {
        todo!()
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
        todo!()
    }

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr> {
        todo!()
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
        todo!()
    }

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
        Ok(Access::Super)
    }

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr> {
        todo!()
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
        todo!()
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
        todo!()
    }
}
