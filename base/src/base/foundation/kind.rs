use derive_name::Name;
use serde_derive::{Deserialize, Deserializer, Serialize};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::str::FromStr;
#[derive(
    Name,
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
    strum_macros::EnumIter,
    strum_macros::IntoStaticStr,
    Serialize,
    Deserialize,
)]
pub enum FoundationKind {
    DockerDaemon,
}

impl Default for FoundationKind {
    fn default() -> Self {
        Self::DockerDaemon
    }
}

//pub type FoundationParser = fn(&Value) -> Result<dyn Foundation, BaseErr>;

