use tokio::sync::oneshot;

use crate::error::Error;
use crate::fail::Fail;


pub enum CreateAppControllerFail {
    PermissionDenied,
    SpacesDoNotMatch,
    UnexpectedResponse,
    Timeout,
    Error(Error),
}
