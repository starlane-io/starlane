#![allow(warnings)]

#[macro_use]
extern crate async_recursion;
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate cosmic_macros;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate strum_macros;

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use http::StatusCode;
use tokio::io;
use tokio::runtime::{Handle, Runtime};
use tokio::sync::{mpsc, oneshot};
use tracing::error;
use uuid::Uuid;

use cosmic_hyperlane::{HyperAuthenticator, HyperGate, HyperGateSelector, HyperwayEndpointFactory};
use cosmic_universe::artifact::ArtifactApi;
use cosmic_universe::command::common::{SetProperties, SetRegistry};
use cosmic_universe::command::direct::create::{KindTemplate, Strategy};
use cosmic_universe::command::direct::delete::Delete;
use cosmic_universe::command::direct::query::{Query, QueryResult};
use cosmic_universe::command::direct::select::{Select, SubSelect};
use cosmic_universe::err::UniErr;
use cosmic_universe::fail::Timeout;
use cosmic_universe::hyper::ParticleRecord;
use cosmic_universe::kind::{
    ArtifactSubKind, BaseKind, FileSubKind, Kind, Specific, StarSub, UserBaseSubKind,
};
use cosmic_universe::loc::{
    Layer, MachineName, Point, RouteSeg, StarKey, Surface, ToBaseKind, ToSurface,
};
use cosmic_universe::log::RootLogger;
use cosmic_universe::particle::property::PropertiesConfig;
use cosmic_universe::particle::{Details, Properties, Status, Stub};
use cosmic_universe::security::IndexedAccessGrant;
use cosmic_universe::security::{Access, AccessGrant};
use cosmic_universe::selector::Selector;
use cosmic_universe::settings::Timeouts;
use cosmic_universe::substance::{Substance, SubstanceList, Token};
use cosmic_universe::wave::core::ReflectedCore;
use cosmic_universe::wave::UltraWave;

use crate::driver::{DriverFactory, DriversBuilder};
use crate::machine::{Machine, MachineApi, MachineTemplate};

pub mod driver;
pub mod global;
pub mod layer;
pub mod machine;
pub mod star;

#[cfg(test)]
pub mod tests;

#[cfg(test)]
pub mod test;

#[no_mangle]
pub extern "C" fn cosmic_uuid() -> String {
    Uuid::new_v4().to_string()
}

#[no_mangle]
pub extern "C" fn cosmic_timestamp() -> DateTime<Utc> {
    Utc::now()
}

pub type Registry<P> = Arc<dyn RegistryApi<P>>;

#[async_trait]
pub trait RegistryApi<P>: Send + Sync
where
    P: Hyperverse,
{
    async fn register<'a>(&'a self, registration: &'a Registration) -> Result<Details, P::Err>;

    fn assign<'a>(&'a self, point: &'a Point) -> oneshot::Sender<Point>;

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

pub trait HyperErr:
    Sized
    + Debug
    + Send
    + Sync
    + ToString
    + Clone
    + Into<UniErr>
    + From<UniErr>
    + From<String>
    + From<&'static str>
    + From<tokio::sync::oneshot::error::RecvError>
    + From<std::io::Error>
    + From<zip::result::ZipError>
    + From<Box<bincode::ErrorKind>>
    + From<acid_store::Error>
    + Into<UniErr>
{
    fn to_cosmic_err(&self) -> UniErr;

    fn new<S>(message: S) -> Self
    where
        S: ToString;

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString;

    fn not_found() -> Self {
        Self::not_found_msg("Not Found")
    }

    fn not_found_msg<S>(message: S) -> Self
    where
        S: ToString,
    {
        Self::status_msg(404, message)
    }

    fn status(&self) -> u16;

    fn as_reflected_core(&self) -> ReflectedCore {
        let mut core = ReflectedCore::new();
        core.status =
            StatusCode::from_u16(self.status()).unwrap_or(StatusCode::from_u16(500u16).unwrap());
        core.body = Substance::Empty;
        core
    }
}

#[async_trait]
pub trait Hyperverse: Send + Sync + Sized + Clone
where
    Self::Err: HyperErr,
    Self: 'static,
    Self::RegistryContext: Send + Sync,
    Self::StarAuth: HyperAuthenticator,
    Self::RemoteStarConnectionFactory: HyperwayEndpointFactory,
    Self::Err: HyperErr,
{
    type Err;
    type RegistryContext;
    type StarAuth;
    type RemoteStarConnectionFactory;

    fn machine(&self) -> MachineApi<Self> {
        Machine::new(self.clone())
    }

    fn star_auth(&self, star: &StarKey) -> Result<Self::StarAuth, Self::Err>;
    fn remote_connection_factory_for_star(
        &self,
        star: &StarKey,
    ) -> Result<Self::RemoteStarConnectionFactory, Self::Err>;

    fn machine_template(&self) -> MachineTemplate;
    fn machine_name(&self) -> MachineName;
    fn properties_config<K: ToBaseKind>(&self, base: &K) -> &'static PropertiesConfig;
    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder<Self>;
    async fn global_registry(&self) -> Result<Registry<Self>, Self::Err>;
    async fn star_registry(&self, star: &StarKey) -> Result<Registry<Self>, Self::Err>;
    fn artifact_hub(&self) -> ArtifactApi;
    fn start_services(&self, gate: &Arc<dyn HyperGate>);
    fn logger(&self) -> RootLogger {
        Default::default()
    }

    fn select_kind(&self, template: &KindTemplate) -> Result<Kind, UniErr> {
        let base: BaseKind = BaseKind::from_str(template.base.to_string().as_str())?;
        Ok(match base {
            BaseKind::Root => Kind::Root,
            BaseKind::Space => Kind::Space,
            BaseKind::Base => Kind::Base,
            BaseKind::User => Kind::User,
            BaseKind::App => Kind::App,
            BaseKind::Mechtron => Kind::Mechtron,
            BaseKind::FileSystem => Kind::FileSystem,
            BaseKind::File => match &template.sub {
                None => return Err("expected kind for File".into()),
                Some(kind) => {
                    let file_kind = FileSubKind::from_str(kind.as_str())?;
                    return Ok(Kind::File(file_kind));
                }
            },
            BaseKind::Database => {
                unimplemented!("need to write a SpecificPattern matcher...")
            }
            BaseKind::BundleSeries => Kind::BundleSeries,
            BaseKind::Bundle => Kind::Bundle,
            BaseKind::Artifact => match &template.sub {
                None => {
                    return Err("expected Sub for Artirtact".into());
                }
                Some(sub) => {
                    let artifact_kind = ArtifactSubKind::from_str(sub.as_str())?;
                    return Ok(Kind::Artifact(artifact_kind));
                }
            },
            BaseKind::Control => Kind::Control,
            BaseKind::UserBase => match &template.sub {
                None => {
                    return Err("SubKind must be set for UserBase<?>".into());
                }
                Some(sub) => {
                    let specific =
                        Specific::from_str("starlane.io:redhat.com:keycloak:community:18.0.0")?;
                    let sub = UserBaseSubKind::OAuth(specific);
                    Kind::UserBase(sub)
                }
            },
            BaseKind::Repo => Kind::Repo,
            BaseKind::Portal => Kind::Portal,
            BaseKind::Star => {
                unimplemented!()
            }
            BaseKind::Driver => Kind::Driver,
            BaseKind::Global => Kind::Global,
        })
    }

    fn log<R>(result: Result<R, Self::Err>) -> Result<R, Self::Err> {
        if let Err(err) = result {
            println!("ERR: {}", err.to_string());
            Err(err)
        } else {
            result
        }
    }

    fn log_ctx<R>(ctx: &str, result: Result<R, Self::Err>) -> Result<R, Self::Err> {
        if let Err(err) = result {
            println!("{}: {}", ctx, err.to_string());
            Err(err)
        } else {
            result
        }
    }

    fn log_deep<R, E: ToString>(
        ctx: &str,
        result: Result<Result<R, Self::Err>, E>,
    ) -> Result<Result<R, Self::Err>, E> {
        match &result {
            Ok(Err(err)) => {
                println!("{}: {}", ctx, err.to_string());
            }
            Err(err) => {
                println!("{}: {}", ctx, err.to_string());
            }
            Ok(_) => {}
        }
        result
    }
}

pub struct Settings {
    pub timeouts: Timeouts,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timeouts: Default::default(),
        }
    }
}

#[derive(strum_macros::Display)]
pub enum Anatomy {
    FromHyperlane,
    ToGravity,
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
