use crate::mesh::serde::fail;

use serde::{Serialize,Deserialize};

#[derive(Debug,Clone,Serialize,Deserialize)]
pub enum Fail {
    Fail(fail::Fail),
    Starlane(StarlaneFailure)
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub enum StarlaneFailure {
  Error(String)
}