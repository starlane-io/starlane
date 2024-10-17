use std::sync::Arc;
use sqlx::Error;
use strum::ParseError;
use thiserror::Error;
use starlane::space::err::{HyperSpatialError, ParseErrs, SpaceErr, SpatialError};
use starlane::space::point::Point;

#[derive(Error, Debug,Clone)]
pub enum RegErr {
  #[error(transparent)]
  Parse(#[from] ParseErrs),
  #[error("duplicate error")]
  Dupe,
  #[error("postgres error: {0}")]
  SqlxErr(#[from] Arc<sqlx::Error>),
  #[error(transparent)]
  SpaceErr(#[from] SpaceErr),
  #[error("postgres registry db connection pool '{0}' not found")]
  PoolNotFound(String),
 #[error("expected parent for point `{0}'")]
  ExpectedParent(Point),
  #[error("Registry does not handle GetOp::State operations")]
  NoGetOpStateOperations,
  #[error("Database Setup Failed")]
  DatabaseSetupFail,
  #[error("Point '{point}' registry error: {message}")]
  Point { point: Point, message: String },
  #[error("{0}")]
  Msg(String),
}



impl SpatialError for RegErr {

}

impl HyperSpatialError for RegErr {

}



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



impl From<sqlx::Error> for RegErr {
    fn from(value: Error) -> Self {
        RegErr::SqlxErr(Arc::new(value))
    }
}

impl RegErr {
    pub fn dupe() -> Self {
        Self::Dupe
    }

    pub fn point<S>( point: Point, message: S ) -> RegErr where S: ToString {
        let message = message.to_string();
        RegErr::Point {point, message }
    }

    pub fn pool_not_found<S:ToString>( key: S ) -> Self {
        Self::PoolNotFound(key.to_string())
    }

    pub fn expected_parent(point: &Point) -> Self {
        Self::ExpectedParent(point.clone())
    }
}
