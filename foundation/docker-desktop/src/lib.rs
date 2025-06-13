use async_trait::async_trait;
pub use starlane_base as base;
use starlane_hyperspace::base::err::BaseErr;
use starlane_hyperspace::base::{BaseSub, Foundation};
use starlane_hyperspace::base::provider::{Provider, ProviderKind};
use starlane_space::progress::Progress;
use starlane_space::status::{EntityReadier, StatusDetail, StatusResult, StatusWatcher};

mod concrete {}

pub struct DockerDaemonFoundation();

impl DockerDaemonFoundation {
    pub fn new() -> Self {
        Self()
    }
}
impl BaseSub for DockerDaemonFoundation {}

#[async_trait]
impl Foundation for DockerDaemonFoundation {
    async fn status_detail(&self) -> StatusDetail {
        todo!()
    }

    fn status_watcher(&self) -> &StatusWatcher {
        todo!()
    }

    async fn probe(&self) -> StatusResult {
        todo!()
    }

    async fn ready(&self, progress: Progress) -> StatusResult {
        todo!()
    }

    fn provider<P>(&self, kind: &ProviderKind) -> Result<Option<&P>, BaseErr>
    where
        P: Provider + EntityReadier
    {
        todo!()
    }
}


#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {}
}
