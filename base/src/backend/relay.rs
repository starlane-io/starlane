use std::sync::Arc;
use tokio::sync::watch::Receiver;
use starlane_space::progress::Progress;
use std::marker::PhantomData;
use async_trait::async_trait;
use starlane_hyperspace::base::err::BaseErr;
use starlane_hyperspace::base::{BaseSub, Foundation};
use starlane_hyperspace::base::provider::{Provider, ProviderKindDef};
use starlane_space::status::{EntityReadier, Status, StatusDetail, StatusResult, StatusWatcher};
use crate::backend::call::Call;
use crate::backend::provider::Method;
use crate::base;
use crate::foundation::config::FoundationConfig;

pub struct FoundationTx<F>
where
    F: Foundation,
{
    config: Box<dyn FoundationConfig>,
    phantom: PhantomData<F>,
    call_tx: tokio::sync::mpsc::Sender<Call<Method>>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl<F> FoundationTx<F>
where
    F: Foundation,
{
    fn new(
        config: Box<dyn FoundationConfig>,
        call_tx: tokio::sync::mpsc::Sender<Call<Method>>,
        status: Arc<tokio::sync::watch::Receiver<Status>>,
    ) -> Self {
        Self {
            config,
            call_tx,
            status,
            phantom: PhantomData::default(),
        }
    }
}

impl<F> BaseSub for FoundationTx<F> where F: Foundation, {}

#[async_trait]
impl<F> Foundation for FoundationTx<F>
where
    F: Foundation {
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

    fn provider<P>(&self, kind: &ProviderKindDef) -> Result<Option<&P>, BaseErr>
    where
        P: Provider + EntityReadier
    {
        todo!()
    }
}