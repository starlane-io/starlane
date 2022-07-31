use mesh_portal::version::latest::fail;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Fail {
    Fail(fail::Fail),
    Starlane(StarlaneFailure),
}

impl ToString for Fail {
    fn to_string(&self) -> String {
        "Fail".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StarlaneFailure {
    Error(String),
}

impl ToString for StarlaneFailure {
    fn to_string(&self) -> String {
        match self {
            StarlaneFailure::Error(e) => e.clone(),
        }
    }
}

impl Into<fail::Fail> for Fail {
    fn into(self) -> fail::Fail {
        match self {
            Fail::Fail(fail) => fail,
            Fail::Starlane(error) => fail::Fail::Mesh(fail::mesh::Fail::Error(error.to_string())),
        }
    }
}
