use std::path::PathBuf;
use std::time::Duration;
use derive_builder::Builder;
use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V13};
use pg_embed::postgres::{PgEmbed, PgSettings};
use serde::{Deserialize, Serialize};
use crate::env::{STARLANE_DATA_DIR, STARLANE_REGISTRY_PASSWORD, STARLANE_REGISTRY_USER};
use crate::{RegistryConfig, StarlaneConfig};
use crate::registry::err::RegErr;

pub struct Postgres {

}

impl Postgres {
    pub async fn new(config: PgEmbedSettings) -> Result<Self,RegErr> {
        let pg_settings: PgSettings = config.into();
        let fetch_settings = PgFetchSettings{
            version: PG_V13,
            ..Default::default()
        };

        let mut pg = PgEmbed::new(pg_settings, fetch_settings).await?;

        // Download, unpack, create password file and database cluster
        pg.setup().await;

        // start postgresql database
        pg.start_db().await;

        // create a new database
        // to enable migrations view the [Usage] section for details
        if pg.database_exists(config.d)
        pg.create_database("database_name").await;



    }



}


#[derive(Builder,Clone,Serialize,Deserialize)]
pub struct PgEmbedSettings {
    database_dir: String,
    port: u16,
    user: String,
    password: String,
    auth_method: PgEmbedAuthMethod,
    persistent: bool,
    timeout: Option<Duration>,
    migration_dir: Option<String>,
}

impl Into<PgSettings> for PgEmbedSettings {
    fn into(self) -> PgSettings {
        PgSettings{
            database_dir: self.database_dir.into(),
            port: self.port,
            user: self.user,
            password: self.password,
            auth_method: self.auth_method.into(),
            persistent: self.persistent,
            timeout: self.timeout,
            migration_dir: self.migration_dir.into(),
        }
    }
}

impl Default for PgEmbedSettings {
    fn default() -> Self {

        Self {
            database_dir: format!("{}/registry",STARLANE_DATA_DIR.to_string()).to_string(),
            port: 5432,
            user: STARLANE_REGISTRY_USER.to_string(),
            password: STARLANE_REGISTRY_PASSWORD.to_string(),
            auth_method: Default::default(),
            persistent: false,
            timeout: Some(Duration::from_secs(30)),
            migration_dir: None,
        }
    }
}


#[derive(Clone,Serialize,Deserialize)]
pub enum PgEmbedAuthMethod {
    Plain,
    MD5,
    ScramSha256,
}

impl Into<PgAuthMethod> for PgEmbedAuthMethod {
    fn into(self) -> PgAuthMethod {
        match self {
            PgEmbedAuthMethod::Plain => PgAuthMethod::Plain,
            PgEmbedAuthMethod::MD5 => PgAuthMethod::MD5,
            PgEmbedAuthMethod::ScramSha256 => PgAuthMethod::ScramSha256
        }
    }
}

impl Default for PgEmbedAuthMethod {
    fn default() -> Self {
        PgEmbedAuthMethod::Plain
    }
}