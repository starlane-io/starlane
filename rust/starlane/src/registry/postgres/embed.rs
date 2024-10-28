use crate::env::{STARLANE_DATA_DIR, STARLANE_REGISTRY_PASSWORD, STARLANE_REGISTRY_USER};
use crate::registry::err::RegErr;
use crate::{Database, PgRegistryConfig, StarlaneConfig};
use derive_builder::Builder;
use pg_embed::pg_enums::PgAuthMethod;
use pg_embed::pg_fetch::{PgFetchSettings, PG_V13, PG_V15};
use pg_embed::postgres::{PgEmbed, PgSettings};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;

pub struct Postgres {
    pg_embed: PgEmbed,
}

impl Postgres {

    pub async fn install(config: &Database<PgEmbedSettings>) -> Result<(), RegErr> {
        fs::create_dir_all(&config.database_dir).await?;

        println!("setup");
        fs::create_dir_all(&config.database_dir).await?;

        let pg_settings: PgSettings = config.settings.clone().into();
        let fetch_settings = PgFetchSettings {
            version: PG_V15,
            ..Default::default()
        };

        let mut pg = PgEmbed::new(pg_settings, fetch_settings).await?;
        // Download, unpack, create password file and database cluster
        pg.setup().await?;

        println!("starting db");
        // start postgresql database
        pg.start_db().await?;

        println!("check exist db");
        // create a new database
        // to enable migrations view the [Usage] section for details
        if !pg.database_exists(config.database.as_str()).await? {
            println!("dreating db");
            pg.create_database(config.database.as_str()).await?;
        }

        println!("shutting down db");
        pg.stop_db().await?;

        Ok(())
    }

    pub async fn new(config: Database<PgEmbedSettings>) -> Result<Self, RegErr> {
        let pg_settings: PgSettings = config.settings.clone().into();
        let fetch_settings = PgFetchSettings {
            version: PG_V15,
            ..Default::default()
        };

        let mut pg = PgEmbed::new(pg_settings, fetch_settings).await?;

        println!("setup");
        // Download, unpack, create password file and database cluster
        pg.setup().await?;

        println!("starting db");
        // start postgresql database
        pg.start_db().await?;

        println!("check exist db");
        // create a new database
        // to enable migrations view the [Usage] section for details
        if !pg.database_exists(config.database.as_str()).await? {
            println!("dreating db");
            pg.create_database(config.database.as_str()).await?;
        }

        println!("pg created");

        Ok(Self { pg_embed: pg })
    }

    /// as long as the Sender is alive
    pub async fn start(mut self) -> Result<tokio::sync::mpsc::Sender<()>, RegErr> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        tokio::spawn(
            async move {
                while let Some(_) = rx.recv().await {
                    // do nothing but linger
                }
            }
        );

//        self.pg_embed.start_db().await?;

        println!("pg running");

        Ok(tx)
    }

}

#[derive(Builder, Clone, Serialize, Deserialize,Eq,PartialEq,Hash)]
pub struct PgEmbedSettings {
    pub port: u16,
    pub user: String,
    pub password: String,
    pub auth_method: PgEmbedAuthMethod,
    pub persistent: bool,
    pub database_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

impl Into<PgSettings> for PgEmbedSettings {
    fn into(self) -> PgSettings {
        PgSettings {
            database_dir: self.database_dir.into(),
            port: self.port,
            user: self.user,
            password: self.password,
            auth_method: self.auth_method.into(),
            persistent: self.persistent,
            timeout: self.timeout,
            migration_dir: None,
            /*
            migration_dir: match self.migration_dir {
                None => None,
                Some(path) => Some(path.into())
            }

             */
        }
    }
}

impl Default for PgEmbedSettings {
    fn default() -> Self {
        Self {
            //database_dir: format!("{}/registry", STARLANE_DATA_DIR.to_string()).to_string(),
            database_dir: "./registry".to_string(),
            port: 5432,
            user: STARLANE_REGISTRY_USER.to_string(),
            password: STARLANE_REGISTRY_PASSWORD.to_string(),
            auth_method: Default::default(),
            persistent: false,
            timeout: Some(Duration::from_secs(30)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize,Eq,PartialEq,Hash)]
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
            PgEmbedAuthMethod::ScramSha256 => PgAuthMethod::ScramSha256,
        }
    }
}

impl Default for PgEmbedAuthMethod {
    fn default() -> Self {
        PgEmbedAuthMethod::Plain
    }
}
