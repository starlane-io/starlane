use serde::{Deserialize, Serialize};
fn default_schema() -> String {
    "public".to_string()
}

#[derive(Debug,Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct PostgresProvider {
    #[serde(default="default_schema")]
    schema: String,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<String>,
}

#[derive(Debug,Clone,Eq,PartialEq,Serialize,Deserialize)]
pub struct RegistryProvider {
    #[serde(default="default_schema")]
    schema: String,
}