use crate::registry::postgres::embed::PgEmbedSettings;
use crate::registry::postgres::{PostgresConnectInfo, PostgresDbKey};
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Database<S> {
    pub database: String,
    pub schema: String,
    pub settings: S,
}

impl<Info> Database<Info> {
    pub fn new<D, S>(database: D, schema: S, settings: Info) -> Database<Info>
    where
        D: ToString,
        S: ToString,
    {
        let database = database.to_string();
        let schema = schema.to_string();
        Database {
            database,
            settings,
            schema,
        }
    }
}

pub struct LiveDatabase {
    pub database: Database<PostgresConnectInfo>,
    pub(crate) handle: tokio::sync::mpsc::Sender<()>,
}

impl LiveDatabase {
    pub fn new(
        database: Database<PostgresConnectInfo>,
        handle: tokio::sync::mpsc::Sender<()>,
    ) -> Self {
        Self { database, handle }
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

impl Database<PgEmbedSettings> {
    pub fn from_embed<D, S>(
        database: D,
        schema: S,
        settings: PgEmbedSettings,
    ) -> Database<PgEmbedSettings>
    where
        D: ToString,
        S: ToString,
    {
        Self::new(database, schema, settings)
    }

    pub fn to_key(&self) -> PostgresDbKey {
        PostgresDbKey {
            url: "localhost".to_string(),
            user: self.settings.username.clone(),
            database: self.database.clone(),
        }
    }

    pub fn to_uri(&self) -> String {
        format!(
            "postgres://{}:{}@localhost/{}",
            self.username, self.password, self.database
        )
    }
}

impl<S> Deref for Database<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}
