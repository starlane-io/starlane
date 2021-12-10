use tokio::sync::oneshot;

use crate::error::Error;
use crate::resource::selector::ResourceSelector;
use crate::fail::Fail;

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
