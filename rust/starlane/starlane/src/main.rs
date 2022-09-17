#![allow(warnings)]

use chrono::{DateTime, Utc};
use cosmic_universe::command::direct::create::KindTemplate;
use cosmic_universe::error::UniErr;
use cosmic_universe::id::{ArtifactSubKind, FileSubKind, MachineName, Specific, StarKey, StarSub, UserBaseSubKind};
use cosmic_universe::id2::{
    BaseSubKind,
};
use cosmic_universe::property::{
    AnythingPattern, BoolPattern, EmailPattern, PointPattern, PropertiesConfig, PropertyPermit,
    PropertySource, U64Pattern, UsernamePattern,
};
use cosmic_universe::substance::Token;
use cosmic_universe::artifact::NoDiceArtifactFetcher;
use cosmic_artifact::Artifacts;
use cosmic_hyperlane::HyperGateSelector;
use cosmic_hyperverse::driver::DriversBuilder;
use cosmic_hyperverse::machine::{Machine, MachineTemplate};
use cosmic_hyperverse::Platform;
use cosmic_hyperverse::{Registry, RegistryApi};
use cosmic_registry_postgres::{
    PostErr, PostgresDbInfo, PostgresPlatform, PostgresRegistry, PostgresRegistryContext,
    PostgresRegistryContextHandle,
};
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io;
use tokio::runtime::Runtime;
use uuid::Uuid;
use cosmic_universe::artifact::ArtifactApi;
use cosmic_universe::id::{BaseKind, Kind, ToBaseKind};

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;

lazy_static! {
    pub static ref STARLANE_PORT: usize = std::env::var("STARLANE_PORT")
        .unwrap_or("4343".to_string())
        .parse::<usize>()
        .unwrap_or(4343);
    pub static ref STARLANE_DATA_DIR: String =
        std::env::var("STARLANE_DATA_DIR").unwrap_or("data".to_string());
    pub static ref STARLANE_CACHE_DIR: String =
        std::env::var("STARLANE_CACHE_DIR").unwrap_or("cache".to_string());
    pub static ref STARLANE_TOKEN: String =
        std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string());
    pub static ref STARLANE_REGISTRY_URL: String =
        std::env::var("STARLANE_REGISTRY_URL").unwrap_or("localhost".to_string());
    pub static ref STARLANE_REGISTRY_USER: String =
        std::env::var("STARLANE_REGISTRY_USER").unwrap_or("postgres".to_string());
    pub static ref STARLANE_REGISTRY_PASSWORD: String =
        std::env::var("STARLANE_REGISTRY_PASSWORD").unwrap_or("password".to_string());
    pub static ref STARLANE_REGISTRY_DATABASE: String =
        std::env::var("STARLANE_REGISTRY_DATABASE").unwrap_or("postgres".to_string());
}
#[no_mangle]
pub extern "C" fn cosmic_uuid() -> String {
    Uuid::new_v4().to_string()
}

#[no_mangle]
pub extern "C" fn cosmic_timestamp() -> DateTime<Utc> {
    Utc::now()
}

fn main() -> Result<(), PostErr> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let machine = Starlane::new().await.unwrap().machine();
        machine.wait().await
    })
}

#[derive(Clone)]
pub struct Starlane {
    ctx: PostgresRegistryContext,
}

impl Starlane {
    pub async fn new() -> Result<Self, Self::Err> {
        let mut dbs = HashSet::new();
        dbs.insert(Self::lookup_registry_db()?);
        for star in stars {
            dbs.insert(Self::lookup_star_db(&star)?);
        }
        let ctx = PostgresRegistryContext::new(dbs).await?;

        Ok(Self { ctx })
    }
}

impl PostgresPlatform for Starlane {
    fn lookup_registry_db() -> Result<PostgresDbInfo, Self::Err> {
        Ok(PostgresDbInfo::new(
            STARLANE_REGISTRY_URL.to_string(),
            STARLANE_REGISTRY_USER.to_string(),
            STARLANE_REGISTRY_PASSWORD.to_string(),
            STARLANE_REGISTRY_DATABASE.to_string(),
        ))
    }

    fn lookup_star_db(star: &StarKey) -> Result<PostgresDbInfo, Self::Err> {
        Ok(PostgresDbInfo::new_with_schema(
            STARLANE_REGISTRY_URL.to_string(),
            STARLANE_REGISTRY_USER.to_string(),
            STARLANE_REGISTRY_PASSWORD.to_string(),
            STARLANE_REGISTRY_DATABASE.to_string(),
            star.to_sql_name(),
        ))
    }
}

#[async_trait]
impl Platform for Starlane {
    type Err = PostErr;
    type RegistryContext = PostgresRegistryContext;

    fn machine_template(&self) -> MachineTemplate {
        MachineTemplate::default()
    }

    fn machine_name(&self) -> MachineName {
        "standalone".to_string()
    }

    fn properties_config<K: ToBaseKind>(&self, base: &K) -> &'static PropertiesConfig {
        match base.to_base() {
            BaseKind::Space => &UNREQUIRED_BIND_AND_CONFIG_PROERTIES_CONFIG,
            BaseKind::UserBase => &USER_BASE_PROPERTIES_CONFIG,
            BaseKind::User => &USER_PROPERTIES_CONFIG,
            BaseKind::App => &MECHTRON_PROERTIES_CONFIG,
            BaseKind::Mechtron => &MECHTRON_PROERTIES_CONFIG,
            _ => &DEFAULT_PROPERTIES_CONFIG,
        }
    }

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder {
        match kind {
            StarSub::Central => {}
            StarSub::Super => {}
            StarSub::Nexus => {}
            StarSub::Maelstrom => {}
            StarSub::Scribe => {}
            StarSub::Jump => {}
            StarSub::Fold => {}
            StarSub::Machine => {}
        }
        DriversBuilder::new()
    }

    fn token(&self) -> Token {
        Token::new(STARLANE_TOKEN.to_string())
    }

    async fn global_registry(
        &self,
        ctx: Arc<Self::RegistryContext>,
    ) -> Result<Registry<Self>, Self::Err> {
        let ctx = PostgresRegistryContextHandle::new(&self.lookup_registry_db()?, ctx);
        Ok(Arc::new(PostgresRegistry::new(ctx, self.clone()).await?))
    }

    async fn star_registry(
        &self,
        star: &StarKey,
        ctx: Arc<Self::RegistryContext>,
    ) -> Result<Registry<Self>, Self::Err> {
        let ctx = PostgresRegistryContextHandle::new(&self.lookup_star_db(star)?, ctx);
        Ok(Arc::new(PostgresRegistry::new(ctx, self.clone()).await?))
    }

    fn artifact_hub(&self) -> ArtifactApi {
        let fetcher = Arc::new(NoDiceArtifactFetcher {});
        ArtifactApi::new(fetcher)
    }

    fn start_services(&self, entry_router: &mut HyperGateSelector) {}

    fn select_kind(&self, template: &KindTemplate) -> Result<Kind, UniErr> {
        let base: BaseKind = BaseKind::from_str(template.base.to_string().as_str())?;
        match base {
            BaseKind::UserBase => match &template.sub {
                None => {
                    return Err("SubKind must be set for UserBase<?>".into());
                }
                Some(sub) => match sub.as_str() {
                    "OAuth" => {
                        let specific =
                            Specific::from_str("starlane.io:redhat.com:keycloak:community:18.0.0")?;
                        let sub = UserBaseSubKind::OAuth(specific);
                        Ok(Kind::UserBase(sub))
                    }
                    what => return Err(format!("unrecognized UserBase sub: '{}'", what).into()),
                },
            },
            _ => Platform::select_kind(self, template),
        }
    }
}

lazy_static! {
    pub static ref DEFAULT_PROPERTIES_CONFIG: PropertiesConfig = default_properties_config();
    pub static ref USER_PROPERTIES_CONFIG: PropertiesConfig = user_properties_config();
    pub static ref USER_BASE_PROPERTIES_CONFIG: PropertiesConfig = userbase_properties_config();
    pub static ref MECHTRON_PROERTIES_CONFIG: PropertiesConfig = mechtron_properties_config();
    pub static ref UNREQUIRED_BIND_AND_CONFIG_PROERTIES_CONFIG: PropertiesConfig =
        unrequired_bind_and_config_properties_config();
}

fn default_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.build()
}

fn mechtron_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.add(
        "config",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.build()
}

fn unrequired_bind_and_config_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        false,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.add(
        "config",
        Box::new(PointPattern {}),
        false,
        false,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.build()
}

fn user_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        Some("hyperspace:repo:boot:1.0.0:/bind/user.bind".to_string()),
        true,
        vec![],
    );
    builder.add(
        "username",
        Box::new(UsernamePattern {}),
        false,
        false,
        PropertySource::Core,
        None,
        false,
        vec![],
    );
    builder.add(
        "email",
        Box::new(EmailPattern {}),
        false,
        true,
        PropertySource::Core,
        None,
        false,
        vec![PropertyPermit::Read],
    );
    builder.add(
        "password",
        Box::new(AnythingPattern {}),
        false,
        true,
        PropertySource::CoreSecret,
        None,
        false,
        vec![],
    );
    builder.build()
}

fn userbase_properties_config() -> PropertiesConfig {
    let mut builder = PropertiesConfig::builder();
    builder.add(
        "bind",
        Box::new(PointPattern {}),
        true,
        false,
        PropertySource::Shell,
        Some("hyperspace:repo:boot:1.0.0:/bind/userbase.bind".to_string()),
        true,
        vec![],
    );
    builder.add(
        "config",
        Box::new(PointPattern {}),
        false,
        true,
        PropertySource::Shell,
        None,
        false,
        vec![],
    );
    builder.add(
        "registration-email-as-username",
        Box::new(BoolPattern {}),
        false,
        false,
        PropertySource::Shell,
        Some("true".to_string()),
        false,
        vec![],
    );
    builder.add(
        "verify-email",
        Box::new(BoolPattern {}),
        false,
        false,
        PropertySource::Shell,
        Some("false".to_string()),
        false,
        vec![],
    );
    builder.add(
        "sso-session-max-lifespan",
        Box::new(U64Pattern {}),
        false,
        true,
        PropertySource::Core,
        Some("315360000".to_string()),
        false,
        vec![],
    );
    builder.build()
}

pub fn properties_config<K: ToBaseKind>(base: &K) -> &'static PropertiesConfig {
    match base.to_base() {
        BaseKind::Space => &UNREQUIRED_BIND_AND_CONFIG_PROERTIES_CONFIG,
        BaseKind::UserBase => &USER_BASE_PROPERTIES_CONFIG,
        BaseKind::User => &USER_PROPERTIES_CONFIG,
        BaseKind::App => &MECHTRON_PROERTIES_CONFIG,
        BaseKind::Mechtron => &MECHTRON_PROERTIES_CONFIG,
        _ => &DEFAULT_PROPERTIES_CONFIG,
    }
}
