use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::watch::Receiver;
use starlane_hyperspace::provider::ProviderKind;
use starlane_space::progress::Progress;
use starlane_space::status::{Status, StatusDetail};
use crate::err::BaseErr;
use crate::{registry, Foundation};
use crate::kind::FoundationKind;

pub(crate) struct FoundationSafety<F>
where
    F: Foundation,
{
    foundation: Box<F>,
}

impl<F> CreateProxy for FoundationSafety<F>
where
    F: Foundation,
{
    type Proxy = F;

    fn proxy(&self) -> Result<Self::Proxy, BaseErr> {
        todo!()
    }
}



#[async_trait]
impl<F> Foundation for FoundationSafety<F>
where
    F: Foundation,
{
    type Config = F::Config;

    type Provider = F::Provider;

    fn kind(&self) -> FoundationKind {
        self.foundation.kind()
    }

    fn config(&self) -> Arc<Self::Config> {
        self.foundation.config()
    }

    fn status(&self) -> Status {
        self.status()
    }

    async fn status_detail(&self) -> Result<StatusDetail, BaseErr> {
        todo!()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.foundation.status_watcher()
    }

    async fn probe(&self, progress: Progress) -> Result<Status, BaseErr> {
        self.foundation.synchronize(progress).await
    }

    async fn ready(&self, progress: Progress) -> Result<(), BaseErr> {

        if self.status() == Status::Ready{
            Err(BaseErr::unknown_state("install"))
        } else {
            self.foundation.install(progress).await
        }
    }

    fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<Self::Provider>>, BaseErr> {
        if self.status() == Status::Unknown {
            Err(BaseErr::unknown_state(kind))
        } else {
            self.foundation.provider(kind)
        }
    }


    /*
    fn registry(&self) -> Result<registry::Registry, BaseErr>
    {
        if self.status() == Status::Unknown {
            Err(BaseErr::unknown_state(&ProviderKind::Registry))
        } else {
            Ok(self.foundation.provider(&ProviderKind::Registry))
        }

    }

     */
}