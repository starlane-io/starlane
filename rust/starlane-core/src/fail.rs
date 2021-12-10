use crate::mesh::serde::fail;

pub enum Fail {
    Fail(fail::Fail),
    Starlane(StarlaneFailure)
}

pub enum StarlaneFailure {
  Error(String)
}