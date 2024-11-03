use tokio::fs;
use crate::env::{STARLANE_CONTROL_PORT, STARLANE_DATA_DIR, STARLANE_REGISTRY_PASSWORD, STARLANE_REGISTRY_USER};
use crate::registry::err::RegErr;
use crate::{Database, PgRegistryConfig, StarlaneConfig};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use port_check::is_local_ipv4_port_free;
use postgresql_embedded::{PostgreSQL, Settings};
use sqlx::PgPool;
use starlane::space::parse::set;
use crate::shutdown::{add_shutdown_hook, panic_shutdown, shutdown};

pub struct Postgres {
    config: Database<PgEmbedSettings>,
    postgres: PostgreSQL
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


    fn postgresql(config: &Database<PgEmbedSettings> ) -> Settings
    {
        let mut settings = Settings::default();
        settings.data_dir = format!("{}/registry",config.database_dir).to_string().into();
        settings.password_file = format!("{}/.password",config.database_dir).to_string().into();
        settings.port = 5432u16;
        settings.temporary = !config.persistent;
        settings.username = config.user.clone();
        settings.password = config.password.clone();
        settings
    }



    pub async fn install( config: &Database<PgEmbedSettings>) -> Result<(), RegErr> {

        let settings = Self::postgresql(config);
        fs::create_dir_all(&settings.data_dir).await?;

        let mut postgres = PostgreSQL::new(settings);

        postgres.setup().await?;


        if !is_local_ipv4_port_free(config.port) {
            panic_shutdown(format!("postgres registry port '{}' is being used by another process", config.port));
        }


        postgres.start().await?;

        if !postgres.database_exists(&config.database).await? {
            postgres.create_database(&config.database).await?;
        }
       postgres.stop().await?;

        Ok(())
    }



    pub async fn new(config: &Database<PgEmbedSettings>) -> Result<Self, RegErr> {

        let mut postgres = PostgreSQL::new(Self::postgresql(config));

        let config = config.clone();
        Ok(Self { postgres, config })
    }

    /// as long as the Sender is alive
    ///
    pub async fn start(mut self) -> Result<tokio::sync::mpsc::Sender<()>, RegErr> {

        self.postgres.setup().await?;

        if !is_local_ipv4_port_free(self.postgres.settings().port) {
            panic_shutdown(format!("embedded postgres registry port '{}' is already in use", self.postgres.settings().port));
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
        tokio::spawn(
            async move {
                while let Some(_) = rx.recv().await {
                    blah.url();
                }
            }
        );


        Ok(tx)
    }

}

impl Drop for Postgres {
    fn drop(&mut self) {
        let handler = tokio::runtime::Handle::current();
        let mut postgres = self.postgres.clone();
        handler.spawn( async move{
            postgres.stop().await.unwrap_or_default();
        });
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


impl Default for PgEmbedSettings {
    fn default() -> Self {
        Self {
            database_dir: format!("{}/postgres", STARLANE_DATA_DIR.to_string()).to_string(),
            port: 5432,
            user: STARLANE_REGISTRY_USER.to_string(),
            password: STARLANE_REGISTRY_PASSWORD.to_string(),
            auth_method: Default::default(),
            persistent: true,
            timeout: Some(Duration::from_secs(5)),
        }
    }
}

#[derive(Clone, Serialize, Deserialize,Eq,PartialEq,Hash)]
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
