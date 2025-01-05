use std::sync::Arc;
use tokio::sync::watch::Receiver;
use starlane_space::progress::Progress;
use std::marker::PhantomData;

pub struct FoundationTx<F>
where
    F: foundation::Foundation,
{
    phantom: PhantomData<F>,
    config: base::config::default::FoundationConfig,
    call_tx: tokio::sync::mpsc::Sender<Method<F>>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl<F> FoundationTx<F>
where
    F: foundation::Foundation,
{
    fn new(
        config: base::config::default::FoundationConfig,
        call_tx: tokio::sync::mpsc::Sender<Method<F>>,
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

#[async_trait]
impl<F> foundation::Foundation for FoundationTx<F>
where
    F: foundation::Foundation,
{
    type Config = F::Config;
    type Dependency = ();
    type Provider = ();

    fn kind(&self) -> FoundationKind {
        self.config.kind().clone()
    }

    fn config(&self) -> Self::Config {
        self.config.clone()
    }

    fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.status.clone()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Method::Probe { progress, rtn })
            .map_err(BaseErr::msg)?;
        rx.await.map_err(BaseErr::msg)?
    }

    async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Method::Install { progress, rtn })
            .map_err(BaseErr::msg)?;
        rx.await.map_err(BaseErr::msg)?
    }

    fn dependency(
        &self,
        kind: &DependencyKind,
    ) -> Result<Option<Self::Dependency>, BaseErr> {
        let kind = kind.clone();
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Method::MakeReady { kind, rtn })
            .map_err(BaseErr::msg)?;
        Ok(rx.blocking_recv().map_err(BaseErr::msg)??)
    }

    fn registry(&self) -> Result<registry::Registry, BaseErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Method::Registry(rtn))
            .map_err(BaseErr::msg)?;
        rx.blocking_recv().map_err(BaseErr::msg)?
    }
}