use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize, Serializer};
use tokio::sync::{mpsc, oneshot};

use crate::app::{AppSpecific, ConfigSrc};
use crate::error::Error;
use crate::message::Fail;
use crate::names::Name;
use crate::permissions::Authentication;
use crate::resource::{AppKey, Labels, ResourceSelector};

pub struct AppSelectCommand {
    pub selector: ResourceSelector,
    pub tx: oneshot::Sender<Result<Vec<AppKey>, Fail>>,
}

pub enum CreateAppControllerFail {
    PermissionDenied,
    SpacesDoNotMatch,
    UnexpectedResponse,
    Timeout,
    Error(Error),
}
