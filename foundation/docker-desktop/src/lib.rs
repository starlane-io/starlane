use async_trait::async_trait;
use bollard::Docker;
pub use starlane_base as base;
use starlane_hyperspace::base::err::BaseErr;
use starlane_hyperspace::base::provider::{Provider,  ProviderKind};
use starlane_hyperspace::base::{BaseSub, Foundation};
use starlane_hyperspace::base::config::ProviderConfig;
use starlane_space::progress::Progress;
use starlane_space::status::{StatusDetail, StatusProbe, StatusWatcher};

mod concrete {}


pub fn create_docker_foundation() {
}

pub struct DockerDaemonProvider;

#[async_trait]
impl Provider for DockerDaemonProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::DockerDaemon
    }

    async fn start(&self) -> StatusDetail {
        todo!()
    }
}

impl Default for DockerDaemonProvider {
    fn default() -> Self {
       Self
    }
}

impl BaseSub for DockerDaemonProvider {}


pub struct DockerDaemonProviderFactory;

impl Default for DockerDaemonProviderFactory {
    fn default() -> Self {
        Self
    }
}


#[async_trait]
impl StatusProbe for DockerDaemonProvider {
    async fn probe(&self) -> StatusDetail {
        todo!()
    }
}





#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
