#[cfg(feature = "postgres")]
use sqlx::Error;

use crate::space::err::{HyperSpatialError, ParseErrs, SpaceErr, SpatialError};
use crate::space::point::Point;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RegErr {
    #[error(transparent)]
    Parse(#[from] ParseErrs),
    #[error("duplicate error")]
    Dupe,
    #[error("particle not found: '{0}'")]
    NotFound(Point),

    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),

    #[error("expected parent for point `{0}'")]
    ExpectedParent(Point),
    #[error("Registry does not handle GetOp::State operations")]
    NoGetOpStateOperations,
    #[error("Database Setup Failed")]
    RegistrySetupFailed,
    #[error("Point '{point}' registry error: {message}")]
    Point { point: Point, message: String },
    #[error("{0}")]
    Msg(String),

    #[cfg(feature = "postgres")]
    #[error("postgres error: {0}")]
    SqlxErr(#[from] Arc<sqlx::Error>),

    #[error("postgres registry db connection pool '{0}' not found")]
    PoolNotFound(String),

    #[cfg(feature = "postgresql-embedded")]
    #[error("postgres embed error error: {0}")]
    PgErr(#[from] postgresql_embedded::Error),
    #[error(transparent)]
    IoErr(Arc<std::io::Error>),
    #[error("database has scorch guard enabled.  To change this: 'INSERT INTO reset_mode VALUES ('Scorch')'")]
    NoScorch,
    #[error("expected an embedded postgres registry but received configuration for a remote postgres registry")]
    ExpectedEmbeddedRegistry,
}

impl From<std::io::Error> for RegErr {
    fn from(value: std::io::Error) -> Self {
        Self::IoErr(Arc::new(value))
    }
}

impl SpatialError for RegErr {}

impl HyperSpatialError for RegErr {}

impl From<&str> for RegErr {
    fn from(err: &str) -> Self {
        Self::Msg(err.to_string())
    }
}

impl From<&String> for RegErr {
    fn from(err: &String) -> Self {
        Self::Msg(err.to_string())
    }
}

#[cfg(feature = "postgres")]
impl From<sqlx::Error> for RegErr {
    fn from(value: Error) -> Self {
        RegErr::SqlxErr(Arc::new(value))
    }
}

impl RegErr {
    pub fn dupe() -> Self {
        Self::Dupe
    }

    pub fn point<S>(point: Point, message: S) -> RegErr
    where
        S: ToString,
    {
        let message = message.to_string();
        RegErr::Point { point, message }
    }

    pub fn pool_not_found<S: ToString>(key: S) -> Self {
        Self::PoolNotFound(key.to_string())
    }

    pub fn expected_parent(point: &Point) -> Self {
        Self::ExpectedParent(point.clone())
    }

    pub fn msg<M>(msg: M) -> RegErr
    where
        M: ToString,
    {
        Self::Msg(msg.to_string())
    }
}
