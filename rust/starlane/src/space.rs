use crate::keys::{SpaceKey, UserKey, AppKey, SubSpaceKey};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use tokio::sync::{mpsc, oneshot};
use std::sync::Arc;
use crate::permissions::Authentication;
use crate::error::Error;
use crate::resource::{Labels, ResourceSelector};
use crate::names::Name;
use crate::message::Fail;
use crate::app::{AppSpecific, ConfigSrc};


pub struct AppSelectCommand
{
    pub selector: ResourceSelector,
    pub tx: oneshot::Sender<Result<Vec<AppKey>,Fail>>
}


pub enum CreateAppControllerFail
{
    PermissionDenied,
    SpacesDoNotMatch,
    UnexpectedResponse,
    Timeout,
    Error(Error)
}