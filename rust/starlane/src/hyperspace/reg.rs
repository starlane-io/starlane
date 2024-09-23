use crate::platform::Platform;
use starlane::space::command::common::{SetProperties, SetRegistry};
use starlane::space::command::direct::create::Strategy;
use starlane::space::command::direct::delete::Delete;
use starlane::space::command::direct::query::{Query, QueryResult};
use starlane::space::command::direct::select::{Select, SubSelect};
use starlane::space::hyper::{ParticleLocation, ParticleRecord};
use starlane::space::kind::Kind;
use starlane::space::particle::{Details, Properties, Status, Stub};
use starlane::space::point::Point;
use starlane::space::security::{Access, AccessGrant, IndexedAccessGrant};
use starlane::space::selector::Selector;
use starlane::space::substance::SubstanceList;
use std::sync::Arc;

pub type Registry<P> = Arc<dyn RegistryApi<P>>;

#[async_trait]
pub trait RegistryApi<P>: Send + Sync
where
    P: Platform,
{
    async fn nuke<'a>(&'a self) -> Result<(), P::Err>;

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), P::Err>;

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), P::Err>;

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), P::Err>;

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), P::Err>;

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), P::Err>;

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, P::Err>;

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, P::Err>;

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, P::Err>;

    async fn query<'a>(&'a self, point: &'a Point, query: &'a Query)
        -> Result<QueryResult, P::Err>;

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, P::Err>;

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, P::Err>;

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, P::Err>;

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), P::Err>;

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, P::Err>;

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), P::Err>;

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, P::Err>;

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), P::Err>;
}

pub struct RegistryWrapper<P>
where
    P: Platform,
{
    registry: Registry<P>,
}

impl<P> RegistryWrapper<P>
where
    P: Platform,
{
    pub fn new(registry: Registry<P>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl<P> RegistryApi<P> for RegistryWrapper<P>
where
    P: Platform,
{
    async fn nuke<'a>(&'a self) -> Result<(), P::Err> {
        self.registry.nuke().await
    }

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), P::Err> {
        self.registry.register(registration).await
    }

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), P::Err> {
        self.registry.assign_star(point, star).await
    }

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), P::Err> {
        self.registry.assign_host(point, host).await
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), P::Err> {
        self.registry.set_status(point, status).await
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), P::Err> {
        self.registry.set_properties(point, properties).await
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, P::Err> {
        self.registry.sequence(point).await
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, P::Err> {
        self.registry.get_properties(point).await
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, P::Err> {
        if point.is_global() {
            let location = ParticleLocation::new(Some(Point::local_star()), None);
            let record = ParticleRecord {
                details: Details {
                    stub: Stub {
                        point: point.clone(),
                        kind: Kind::Global,
                        status: Status::Ready,
                    },
                    properties: Properties::default(),
                },
                location,
            };

            Ok(record)
        } else {
            self.registry.record(point).await
        }
    }

    async fn query<'a>(
        &'a self,
        point: &'a Point,
        query: &'a Query,
    ) -> Result<QueryResult, P::Err> {
        self.registry.query(point, query).await
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, P::Err> {
        self.registry.delete(delete).await
    }

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, P::Err> {
        self.registry.select(select).await
    }

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, P::Err> {
        self.registry.sub_select(sub_select).await
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), P::Err> {
        self.registry.grant(access_grant).await
    }

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, P::Err> {
        self.registry.access(to, on).await
    }

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), P::Err> {
        self.registry.chown(on, owner, by).await
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, P::Err> {
        self.registry.list_access(to, on).await
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), P::Err> {
        self.registry.remove_access(id, to).await
    }
}

#[derive(Clone)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
    pub strategy: Strategy,
    pub status: Status,
}
