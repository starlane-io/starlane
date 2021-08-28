use tokio::sync::oneshot;

use starlane_resources::message::Fail;
use starlane_resources::ResourceSelector;

use crate::error::Error;
use crate::resource::AppKey;

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
