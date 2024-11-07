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
use serde::{Deserialize, Serialize};
use crate::database::Database;
use crate::registry::err::RegErr;
use crate::registry::postgres::embed::PgEmbedSettings;
use crate::registry::postgres::PostgresConnectInfo;

pub type Registry = Arc<dyn RegistryApi>;

#[async_trait]
pub trait RegistryApi: Send + Sync
{
    async fn scorch<'a>(&'a self) -> Result<(), RegErr>;

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr>;

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr>;

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr>;

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr>;

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr>;

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr>;

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr>;

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr>;

    async fn query<'a>(&'a self, point: &'a Point, query: &'a Query)
        -> Result<QueryResult, RegErr>;

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr>;

//    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr>;

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
        let point = select.pattern.query_root();

        let hierarchy = self
            .query(&point, &Query::PointHierarchy)
            .await?
            .try_into()?;

        let sub_select_hops = select.pattern.sub_select_hops();
        let sub_select = select
            .clone()
            .sub_select(point.clone(), sub_select_hops, hierarchy);
        let mut list = self.sub_select(&sub_select).await?;
        if select.pattern.matches_root() {
            list.push(Stub {
                point: Point::root(),
                kind: Kind::Root,
                status: Status::Ready,
            });
        }

        let list = sub_select.into_payload.to_primitive(list)?;

        Ok(list)
    }


    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr>;

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr>;

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr>;

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr>;

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr>;

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr>;
}

pub struct RegistryWrapper
{
    registry: Registry,
}

impl RegistryWrapper
{
    pub fn new(registry: Registry) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl RegistryApi for RegistryWrapper
{
    async fn scorch<'a>(&'a self) -> Result<(), RegErr> {
        self.registry.scorch().await
    }

    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<(), RegErr> {
        self.registry.register(registration).await
    }

    async fn assign_star<'a>(&'a self, point: &'a Point, star: &'a Point) -> Result<(), RegErr> {
        self.registry.assign_star(point, star).await
    }

    async fn assign_host<'a>(&'a self, point: &'a Point, host: &'a Point) -> Result<(), RegErr> {
        self.registry.assign_host(point, host).await
    }

    async fn set_status<'a>(&'a self, point: &'a Point, status: &'a Status) -> Result<(), RegErr> {
        self.registry.set_status(point, status).await
    }

    async fn set_properties<'a>(
        &'a self,
        point: &'a Point,
        properties: &'a SetProperties,
    ) -> Result<(), RegErr> {
        self.registry.set_properties(point, properties).await
    }

    async fn sequence<'a>(&'a self, point: &'a Point) -> Result<u64, RegErr> {
        self.registry.sequence(point).await
    }

    async fn get_properties<'a>(&'a self, point: &'a Point) -> Result<Properties, RegErr> {
        self.registry.get_properties(point).await
    }

    async fn record<'a>(&'a self, point: &'a Point) -> Result<ParticleRecord, RegErr> {
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
    ) -> Result<QueryResult, RegErr> {
        self.registry.query(point, query).await
    }

    async fn delete<'a>(&'a self, delete: &'a Delete) -> Result<SubstanceList, RegErr> {
        self.registry.delete(delete).await
    }

    async fn select<'a>(&'a self, select: &'a mut Select) -> Result<SubstanceList, RegErr> {
        self.registry.select(select).await
    }

    async fn sub_select<'a>(&'a self, sub_select: &'a SubSelect) -> Result<Vec<Stub>, RegErr> {
        self.registry.sub_select(sub_select).await
    }

    async fn grant<'a>(&'a self, access_grant: &'a AccessGrant) -> Result<(), RegErr> {
        self.registry.grant(access_grant).await
    }

    async fn access<'a>(&'a self, to: &'a Point, on: &'a Point) -> Result<Access, RegErr> {
        self.registry.access(to, on).await
    }

    async fn chown<'a>(
        &'a self,
        on: &'a Selector,
        owner: &'a Point,
        by: &'a Point,
    ) -> Result<(), RegErr> {
        self.registry.chown(on, owner, by).await
    }

    async fn list_access<'a>(
        &'a self,
        to: &'a Option<&'a Point>,
        on: &'a Selector,
    ) -> Result<Vec<IndexedAccessGrant>, RegErr> {
        self.registry.list_access(to, on).await
    }

    async fn remove_access<'a>(&'a self, id: i32, to: &'a Point) -> Result<(), RegErr> {
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

pub enum RegistryConfig {
    #[cfg(feature = "postgres")]
    Postgres(PgRegistryConfig),
}

#[cfg(feature = "postgres")]
#[derive(Clone, Serialize, Deserialize)]
pub enum PgRegistryConfig {
    #[cfg(feature = "postgres-embedded")]
    Embedded(Database<PgEmbedSettings>),
    External(Database<PostgresConnectInfo>),
}

impl PgRegistryConfig {
    pub fn database(&self) -> String {
        match self {
            PgRegistryConfig::Embedded(d) => d.database.clone(),
            PgRegistryConfig::External(d) => d.database.clone(),
        }
    }
}

impl Into<Database<PostgresConnectInfo>> for PgRegistryConfig {
    fn into(self) -> Database<PostgresConnectInfo> {
        match self {
            PgRegistryConfig::Embedded(p) => p.into(),
            PgRegistryConfig::External(p) => p,
        }
    }
}

impl TryInto<Database<PgEmbedSettings>> for PgRegistryConfig {
    type Error = RegErr;

    fn try_into(self) -> Result<Database<PgEmbedSettings>, Self::Error> {
        match self {
            PgRegistryConfig::Embedded(registry) => Ok(registry),
            _ => Err(RegErr::Msg(
                "cannot get PgEmbedSettings from None Embedded Registry config".to_string(),
            )),
        }
    }
}

