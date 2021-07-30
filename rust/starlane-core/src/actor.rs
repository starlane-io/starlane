

use std::fmt;


use std::str::FromStr;


use serde::{Deserialize, Serialize};





use crate::error::Error;



use crate::names::Name;



pub type ActorSpecific = Name;
pub type GatheringSpecific = Name;

#[derive(Debug,Eq, PartialEq, Hash, Clone, Serialize, Deserialize)]
pub enum ActorKind {
    Stateful,
    Stateless,
}

impl ActorKind {
    // it looks a little pointless but helps get around a compiler problem with static_lazy values
    pub fn as_kind(&self) -> Self {
        self.clone()
    }
}

impl fmt::Display for ActorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ActorKind::Stateful => "Stateful".to_string(),
                ActorKind::Stateless => "Stateless".to_string(),
            }
        )
    }
}

impl FromStr for ActorKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Stateful" => Ok(ActorKind::Stateful),
            "Stateless" => Ok(ActorKind::Stateless),
            _ => Err(format!("could not find ActorKind: {}", s).into()),
        }
    }
}
