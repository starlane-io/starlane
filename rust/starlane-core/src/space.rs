use tokio::sync::oneshot;

use crate::error::Error;
use crate::message::Fail;

use crate::resource::{AppKey, ResourceSelector};

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
