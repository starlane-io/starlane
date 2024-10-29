use tokio::fs;
use crate::env::{STARLANE_DATA_DIR, STARLANE_REGISTRY_PASSWORD, STARLANE_REGISTRY_USER};
use crate::registry::err::RegErr;
use crate::{Database, PgRegistryConfig, StarlaneConfig};
use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;
use postgresql_embedded::{PostgreSQL, Settings};
use sqlx::PgPool;
use starlane::space::parse::set;

pub struct Postgres {
    config: Database<PgEmbedSettings>,
    postgres: PostgreSQL
}

impl Postgres {

    pub fn url(&self) -> String {
        self.postgres.settings().url("postgres")
    }


    fn postgresql(config: &Database<PgEmbedSettings> ) -> Settings
    {
        let mut settings = Settings::default();
        settings.temporary = false;
        settings.port = 5432;
println!("data_dir: {}", settings.data_dir.display());
println!("installation_dir: {}", settings.installation_dir.display());
println!("settings: {:?}", settings);
        settings
    }

    /*

    fn postgresql(config: &Database<PgEmbedSettings> ) -> Settings
    {
        let mut settings = Settings::default();
        settings.data_dir = format!("{}/registry",config.database_dir).to_string().into();
        settings.password_file = format!("{}/.password",config.database_dir).to_string().into();
        settings.temporary = !config.persistent;
        settings.username = config.user.clone();
        settings.password = config.password.clone();
        settings
    }

     */


    pub async fn install( config: &Database<PgEmbedSettings>) -> Result<(), RegErr> {

        let settings = Self::postgresql(config);
        println!("creating directories {}", settings.data_dir.display());
        println!("temporary? {}",settings.temporary);
        fs::create_dir_all(&settings.data_dir).await?;

        println!("installing postgres...");

        let mut postgres = PostgreSQL::new(settings);

        println!("running setup...");

        postgres.setup().await?;
        println!("staring ...");
        postgres.start().await?;

        println!("Started...");
        println!("checking db: {}", config.database);
        if !postgres.database_exists(&config.database).await? {
            println!("\n\nDB create...");
            postgres.create_database(&config.database).await?;
        }
        println!();
        println!();
        println!();
        println!();
        println!("{}",postgres.settings().url(&config.database) );

        tokio::time::sleep(Duration::from_secs(30)).await;

        println!("stopping...");

        postgres.stop().await?;

        println!("done");
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
        println!("setup..");
        self.postgres.setup().await?;
        println!("starting...");
        self.postgres.start().await?;
        println!("*started() : {:?}",self.postgres.settings());
        let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
        let blah = self;
        tokio::spawn(
            async move {
                while let Some(_) = rx.recv().await {
                    blah.url();
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
