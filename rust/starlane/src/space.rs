use crate::app::{AppCreate, AppSelect};
use crate::keys::{SpaceKey, UserKey};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;

#[derive(Clone,Serialize,Deserialize)]
pub struct SpaceCommandWrapper
{
    org: SpaceKey,
    user: UserKey,
    command: SpaceCommand
}

#[derive(Clone,Serialize,Deserialize)]
pub enum SpaceCommand
{
    AppCreate(AppCreate),
    AppSelect(AppSelect),
    AppDestroy
}


impl fmt::Display for SpaceCommand{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            SpaceCommand::AppCreate(_) => "AppCreate".to_string(),
            SpaceCommand::AppSelect(_) => "AppSelect".to_string(),
            SpaceCommand::AppDestroy => "Destroy".to_string()
        };
        write!(f, "{}",r)
    }
}

