pub mod service;
pub mod database;
mod err;

use starlane_hyperspace::provider::Provider;
use starlane_space::status::StatusEntity;
use std::fmt::Display;
use std::str::FromStr;
use starlane_hyperspace::provider::config::ProviderConfig;
/*
let pool = PgPoolOptions::new()
.max_connections(5)
.connect(db.database.to_uri().as_str())
.await?;
 */

/*
#[derive(Clone)]
pub struct LiveDatabase {
    pub database: Database<PostgresConnectInfo>,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl LiveDatabase {
    pub fn new(database: Database<PostgresConnectInfo>, tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self { database, tx }
    }
}

impl Database<PostgresConnectInfo> {
    pub fn from_con<D, S>(
        database: D,
        schema: S,
        info: PostgresConnectInfo,
    ) -> Database<PostgresConnectInfo>
    where
        D: ToString,
        S: ToString,
    {
        Database::new(database, schema, info)
    }

    pub fn to_key(&self) -> PostgresDbKey {
        PostgresDbKey {
            url: self.url.clone(),
            user: self.user.clone(),
            database: self.database.clone(),
        }
    }

    pub fn to_uri(&self) -> String {
        /*
        format!(
            "postgres://{}:{}@{}/{}",
            self.user, self.password, self.url, self.database
        )

         */
        self.url.clone()
    }
}

 */
