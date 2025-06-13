use serde::{Deserialize, Serialize};
use std::ops::Deref;
use crate::registry::postgres::{PostgresConnectInfo, PostgresDbKey};

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



pub struct PostgresClusterConfig;

impl Database<PostgresClusterConfig> {
    pub fn from_embed<D, S>(
        database: D,
        schema: S,
        settings: PostgresClusterConfig,
    ) -> Database<PostgresClusterConfig>
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
