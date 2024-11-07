
use crate::registry::err::RegErr;
use crate::shutdown::{add_shutdown_hook, panic_shutdown};
use derive_builder::Builder;
use port_check::is_local_ipv4_port_free;
use postgresql_embedded::{PostgreSQL, Settings};
use rustyline::completion::Candidate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use crate::database::Database;
use crate::platform::PlatformConfig;
use crate::reg::PgRegistryConfig;
use crate::registry::postgres::PostgresConnectInfo;

pub struct Postgres {
    config: Database<PgEmbedSettings>,
    postgres: PostgreSQL,
}

impl Postgres {
    pub fn url(&self) -> String {
        self.postgres.settings().url("postgres")
    }

    /*
    fn postgresql(config: &Database<PgEmbedSettings> ) -> Settings
    {
        let mut settings = Settings::default();
        settings.temporary = false;
        settings.port = 5432;
        settings
    }

     */

    fn embedded_postgresql(config: &dyn PlatformConfig) -> Result<Settings,RegErr> {

        match config.registry() {
            PgRegistryConfig::Embedded(pg_config) => {
                let mut settings = Settings::default();
                settings.data_dir = format!("{}/registry", pg_config.database_dir(config.home()).display())
                    .to_string()
                    .into();
                settings.password_file = format!("{}/.password", pg_config.database_dir(&config.home()).display())
                    .to_string()
                    .into();
                settings.port = pg_config.port;
                settings.temporary = !pg_config.persistent;
                settings.username = pg_config.username.clone();
                settings.password = pg_config.password.clone();
                Ok(settings)
            }
            PgRegistryConfig::External(_) => {
                Err(RegErr::ExpectedEmbeddedRegistry)
            }
        }
    }

    pub async fn install(config: &dyn PlatformConfig) -> Result<(), RegErr> {
        let database = match config.registry() {
            PgRegistryConfig::Embedded(database) => {
                let settings = Self::embedded_postgresql(config)?;
                fs::create_dir_all(&settings.data_dir).await?;

                let mut postgres = PostgreSQL::new(settings.clone());

                postgres.setup().await?;

                if !is_local_ipv4_port_free(settings.port.clone()) {
                    let err = format!("postgres registry port '{}' is being used by another process", settings.port);
                    panic_shutdown(err.clone());
                    Err(RegErr::Msg(err.to_string()))?;
                }

                postgres.start().await?;

                let _postgres = postgres.clone();

                add_shutdown_hook(Box::pin(async move {
                    _postgres.stop().await.unwrap_or_default();
                }));

                if !postgres.database_exists(&database.database).await? {
                    postgres.create_database(&database.database).await?;
                }

                database.clone().into()
            }
            PgRegistryConfig::External(database) => {
                database.clone()
            }
        };


        Ok(())
    }

    pub async fn new(config: &dyn PlatformConfig) -> Result<Self, RegErr> {
        let mut postgres = PostgreSQL::new(Self::embedded_postgresql(config)?);

        let config = config.registry().clone().try_into()?;
        Ok(Self { postgres, config })
    }

    /// as long as the Sender is alive
    ///
    pub async fn start(mut self) -> Result<tokio::sync::mpsc::Sender<()>, RegErr> {
        self.postgres.setup().await?;

        if !is_local_ipv4_port_free(self.postgres.settings().port) {
            panic_shutdown(format!(
                "embedded postgres registry port '{}' is already in use",
                self.postgres.settings().port
            ));
        }
        self.postgres.start().await?;

        let postgres = self.postgres.clone();

        add_shutdown_hook(Box::pin(async move {
            println!("shutdown postgres...");
            postgres.stop().await.unwrap();
            println!("postgres halted");
        }));

        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        let blah = self;
        tokio::spawn(async move {
            while let Some(_) = rx.recv().await {
                blah.url();
            }
        });

        Ok(tx)
    }
}

impl Drop for Postgres {
    fn drop(&mut self) {
        let handler = tokio::runtime::Handle::current();
        let mut postgres = self.postgres.clone();
        handler.spawn(async move {
            postgres.stop().await.unwrap_or_default();
        });
    }
}

#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct PgEmbedSettings {
    pub port: u16,
    pub username: String,
    pub password: String,
    pub auth_method: PgEmbedAuthMethod,
    pub persistent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_dir: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<Duration>,
}

impl PgEmbedSettings {
    pub fn database_dir<S>(&self, context_dir: S) -> PathBuf
    where
        S: AsRef<str>,
    {
        match &self.database_dir {
            None => format!("{}/data/postgres", context_dir.as_ref())
                .to_string()
                .into(),
            Some(database_dir) => {
                if database_dir.is_absolute() {
                    format!("{}", database_dir.display()).to_string().into()
                } else {
                    format!("{}/{}", context_dir.as_ref(), database_dir.display())
                        .to_string()
                        .into()
                }
            }
        }
    }
}

impl Into<Database<PostgresConnectInfo>> for Database<PgEmbedSettings> {
    fn into(self) -> Database<PostgresConnectInfo> {
        Database {
            database: self.database.clone(),
            schema: self.schema.clone(),
            settings: PostgresConnectInfo {
            url: self.to_uri(),
            user: self.username.clone(),
            password: self.password.clone(),
        }
    }
    }
}



impl Default for PgEmbedSettings {
    fn default() -> Self {
        Self {
            database_dir: None,
            port: 5432,
            username: "postgres".to_string(),
            password: "password".to_string(),
            auth_method: Default::default(),
            persistent: true,
            timeout: Some(Duration::from_secs(5)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum PgEmbedAuthMethod {
    Plain,
    MD5,
    ScramSha256,
}

impl Default for PgEmbedAuthMethod {
    fn default() -> Self {
        PgEmbedAuthMethod::Plain
    }
}


