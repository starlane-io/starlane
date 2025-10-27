use async_trait::async_trait;
use bollard::Docker;
pub use starlane_base as base;
use starlane_hyperspace::base::err::BaseErr;
use starlane_hyperspace::base::provider::{Provider, ProviderKind};
use starlane_hyperspace::base::{BaseSub, Foundation};
use starlane_space::progress::Progress;
use starlane_space::status::{StatusDetail, StatusWatcher};

mod concrete {}

pub struct DockerDaemonFoundation();

impl DockerDaemonFoundation {
    pub fn new() -> Self {
        Self()
    }
}


#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
