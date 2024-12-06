use crate::base;
use crate::base::config::ProviderConfig;
use crate::base::config::{Config, DependencyConfig, FoundationConfig};
use crate::base::err::BaseErr;
use crate::base::foundation::kind::FoundationKind;
use crate::base::foundation::status::{Phase, PhaseDetail, Status};
use crate::base::foundation::util::SerMap;
use crate::base::kind::{DependencyKind, Kind, ProviderKind};
use crate::base::{foundation, registry};
use crate::space::parse::CamelCase;
use crate::space::point::Point;
use crate::space::progress::Progress;
use md5::digest::FixedOutput;
use starlane_primitive_macros::logger;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use wasmer_wasix::virtual_fs::Upcastable;

enum Call<F>
where
    F: foundation::Foundation,
{
    Synchronize {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<Status, BaseErr>>,
    },
    Dependency {
        kind: DependencyKind,
        rtn: tokio::sync::oneshot::Sender<Result<Option<F::Dependency>, BaseErr>>,
    },

    Install {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), BaseErr>>,
    },
    Registry(tokio::sync::oneshot::Sender<Result<registry::Registry, BaseErr>>),
    DepCall(DepWrapper<F>),
    _Phantom(PhantomData<F>),
}

struct FoundationTx<F>
where
    F: foundation::Foundation,
{
    phantom: PhantomData<F>,
    config: base::config::default::FoundationConfig,
    call_tx: tokio::sync::mpsc::Sender<Call<F>>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl<F> FoundationTx<F>
where
    F: foundation::Foundation,
{
    fn new(
        config: base::config::default::FoundationConfig,
        call_tx: tokio::sync::mpsc::Sender<Call<F>>,
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
            .try_send(Call::Synchronize { progress, rtn })
            .map_err(BaseErr::msg)?;
        rx.await.map_err(BaseErr::msg)?
    }

    async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Call::Install { progress, rtn })
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
            .try_send(Call::Dependency { kind, rtn })
            .map_err(BaseErr::msg)?;
        Ok(rx.blocking_recv().map_err(BaseErr::msg)??)
    }

    fn registry(&self) -> Result<registry::Registry, BaseErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Call::Registry(rtn))
            .map_err(BaseErr::msg)?;
        rx.blocking_recv().map_err(BaseErr::msg)?
    }
}

type DepWrapper<D> = Wrapper<DependencyKind, DepCall<D>>;
type ProvWrapper<P> = Wrapper<ProviderKind, ProviderCall<P>>;

struct Wrapper<K, C> {
    kind: K,
    call: C,
}
impl<K, C> Wrapper<K, C> {
    fn new(kind: K, call: C) -> Self {
        Self { kind, call }
    }
}

enum DepCall<D>
where
    D: foundation::Dependency,
{
    Download {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), BaseErr>>,
    },
    Install {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), BaseErr>>,
    },
    Initialize {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), BaseErr>>,
    },
    Start {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<foundation::LiveService<DependencyKind>, BaseErr>>,
    },
    Provider {
        kind: ProviderKind,
        rtn: tokio::sync::oneshot::Sender<Result<D::Provider, BaseErr>>,
    },
    ProviderCall(ProvWrapper<D::Provider>),
    _Phantom(PhantomData<D>),
}

struct DependencyTx<F>
where
    F: foundation::Foundation,
{
    config: <<F as foundation::Foundation>::Dependency as foundation::Dependency>::Config,
    call_tx: tokio::sync::mpsc::Sender<DepCall<F>>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl<F> DependencyTx<F>
where
    F: foundation::Foundation,
{
    fn new(
        config: F::Dependency::Config,
        call_tx: tokio::sync::mpsc::Sender<DepCall<F>>,
        status: Arc<tokio::sync::watch::Receiver<Status>>,
    ) -> F::Dependency {
        Self {
            config,
            call_tx,
            status,
        }
    }
}

#[async_trait]
impl<F> foundation::Dependency for DependencyTx<F>
where
    F: foundation::Foundation,
{
    type Config = F::Config;
    type Provider = F::Provider;

    fn kind(&self) -> &DependencyKind {
        self.config.kind()
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

    async fn download(&self, progress: Progress) -> Result<(), BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Download { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Install { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn initialize(&self, progress: Progress) -> Result<(), BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Initialize { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn start(
        &self,
        progress: Progress,
    ) -> Result<LiveService<DependencyKind>, BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Start { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    fn provider(&self, kind: &ProviderKind) -> Result<Option<Self::Provider>, BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let kind = kind.clone();
        let call = DepCall::Provider { kind, rtn };
        self.call_tx.try_send(call).map_err(BaseErr::msg)?;
        rtn_rx.blocking_recv().map_err(BaseErr::msg)?
    }
}

enum ProviderCall<P>
where
    P: Provider,
{
    Initialize {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), BaseErr>>,
    },
    Start {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<LiveService<CamelCase>, BaseErr>>,
    },
    _Phantom(PhantomData<P>),
}

struct ProviderTx<P>
where
    P: Provider,
{
    config: P::Config,
    call_tx: tokio::sync::mpsc::Sender<ProviderCall<P>>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl<P> ProviderTx<P>
where
    P: Provider,
{
    fn new(
        config: Arc<dyn ProviderConfig>,
        call_tx: tokio::sync::mpsc::Sender<ProviderCall<P>>,
        status: Arc<tokio::sync::watch::Receiver<Status>>,
    ) -> Self {
        Self {
            config,
            call_tx,
            status,
        }
    }
}

#[async_trait]
impl<P> Provider for ProviderTx<P>
where
    P: Provider,
{
    type Config = P::Config;

    fn kind(&self) -> &ProviderKind {
        &self.config.kind()
    }

    fn config(&self) -> Arc<dyn ProviderConfig> {
        self.config.clone()
    }

    fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.status.clone()
    }

    async fn initialize(&self, progress: Progress) -> Result<(), BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = ProviderCall::Initialize { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn start(&self, progress: Progress) -> Result<LiveService<CamelCase>, BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = ProviderCall::Start { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }
}

struct Runner<F>
where
    F: Foundation,
{
    call_rx: tokio::sync::mpsc::Receiver<Call<F>>,
    call_tx: tokio::sync::mpsc::Sender<Call<F>>,
    foundation: F,
    runners: HashMap<DependencyKind, DependencyRunner<Self>>,
}

impl<F> Runner<F>
where
    F: Foundation,
{
    fn new(foundation: F) -> impl Foundation {
        let (call_tx, call_rx) = tokio::sync::mpsc::channel(64);
        let config = foundation.config().clone();
        let proxy = FoundationTx::new(config, call_tx.clone(), foundation.status_watcher());
        let runner = Self {
            foundation,
            call_rx,
            call_tx,
            runners: Default::default(),
        };
        runner.start();
        proxy
    }

    fn start(self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    fn dependency(
        &mut self,
        kind: DependencyKind,
    ) -> Result<Option<&DependencyRunner<F>>, BaseErr> {
        if !self.runners.contains_key(&kind) {
            match self.foundation.dependency(&kind) {
                Ok(None) => return Ok(None),
                Err(err) => return Err(err),
                Ok(Some(dep)) => {
                    let runner = DependencyRunner::new(dep, self.call_tx.clone());
                    self.runners.insert(kind.clone(), runner);
                }
            }
        }

        let runner = self.runners.get(&kind).unwrap();

        Ok(Some(runner))
    }

    fn proxy(
        &mut self,
        kind: DependencyKind,
    ) -> Result<Option<Box<F::Dependency>>, BaseErr> {
        let runner = self
            .dependency(kind.clone())?
            .ok_or(BaseErr::dep_not_available(kind))?;
        Ok(Some(runner.proxy()))
    }

    async fn run(mut self) -> Result<(), BaseErr> {
        let logger = logger!(Point::global_foundation());
        while let Some(call) = self.call_rx.recv().await {
            match call {
                Call::Synchronize { progress, rtn } => {
                    rtn.send(self.foundation.synchronize(progress).await)
                        .unwrap_or_default();
                }
                Call::Dependency { kind, rtn } => {
                    rtn.send(self.proxy(kind)).unwrap_or_default();
                }
                Call::Install { progress, rtn } => {
                    rtn.send(self.foundation.install(progress).await)
                        .unwrap_or_default();
                }
                Call::Registry(rtn) => {
                    rtn.send(self.foundation.registry()).unwrap_or_default();
                }
                Call::DepCall(wrap) => match self.runners.get_mut(&wrap.kind) {
                    None => {}
                    Some(runner) => runner.handle(wrap.call).await,
                },
                /// should never be called...
                Call::_Phantom(_) => {}
            }
        }
        Ok(())
    }
}

struct DependencyRunner<F>
where
    F: Foundation,
{
    dependency: F::Dependency,
    runners: HashMap<ProviderKind, ProviderRunner<F>>,
    call_tx: tokio::sync::mpsc::Sender<Call<F::Dependency>>,
}

impl<F> DependencyRunner<F>
where
    F: Foundation,
{
    fn new(dependency: F::Dependency, call_tx: tokio::sync::mpsc::Sender<Call<F>>) -> Self {
        Self {
            dependency,
            runners: Default::default(),
            call_tx,
        }
    }

    fn proxy(&self) -> F::Dependency {
        let kind = self.kind().clone();
        let (dep_call_tx, mut dep_call_rx) = tokio::sync::mpsc::channel(64);
        let foundation_call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            while let Some(dep_call) = dep_call_rx.recv().await {
                let prov_wrapper = DepWrapper::new(kind.clone(), dep_call);
                let call = Call::DepCall(prov_wrapper);
                foundation_call_tx.send(call).await.unwrap_or_default();
            }
        });
        Box::new(DependencyTx::new(
            self.dependency.config(),
            dep_call_tx,
            self.dependency.status_watcher(),
        ))
    }

    fn kind(&self) -> &DependencyKind {
        &self.dependency.kind()
    }

    fn provider(
        &mut self,
        kind: ProviderKind,
    ) -> Result<Option<&mut ProviderRunner<F>>, BaseErr> {
        if !self.runners.contains_key(&kind) {
            match self.dependency.provider(&kind) {
                Ok(None) => return Ok(None),
                Err(err) => return Err(err),
                Ok(Some(dep)) => {
                    let runner = ProviderRunner::new(dep, self.call_tx.clone());
                    self.runners.insert(kind.clone(), runner);
                }
            }
        }

        /// we can because we have already confirmed that kind is set via [`HashMap::contains_key()`]
        let runner = self.runners.get_mut(&kind).unwrap();

        Ok(Some(runner))
    }
    fn provider_proxy(
        &mut self,
        kind: ProviderKind,
    ) -> Result<Option<Box<F::Provider>>, BaseErr> {
        let runner = self
            .provider(kind.clone())?
            .ok_or(BaseErr::provider_not_available(kind))?;
        Ok(Some(runner.proxy()))
    }

    async fn handle(&mut self, call: DepCall<F>) {
        match call {
            DepCall::Download { progress, rtn } => {
                rtn.send(self.dependency.download(progress).await)
                    .unwrap_or_default();
            }
            DepCall::Install { progress, rtn } => {
                rtn.send(self.dependency.install(progress).await)
                    .unwrap_or_default();
            }
            DepCall::Initialize { progress, rtn } => {
                rtn.send(self.dependency.initialize(progress).await)
                    .unwrap_or_default();
            }
            DepCall::Start { progress, rtn } => {
                rtn.send(self.dependency.start(progress).await)
                    .unwrap_or_default();
            }
            DepCall::Provider { kind, rtn } => {
                rtn.send(self.provider_proxy(kind)).unwrap_or_default();
            }
            DepCall::ProviderCall(wrap) => {
                if let Some(provider) = self.provider(wrap.kind).unwrap_or_default() {
                    provider.handle(wrap.call).await;
                }
            }
            DepCall::_Phantom(_) => {}
        }
    }
}

struct ProviderRunner<F>
where
    F: Foundation,
{
    provider: F::Provider,
    foundation_call_tx: tokio::sync::mpsc::Sender<Call<F>>,
}

impl<F> ProviderRunner<F>
where
    F: Foundation,
{
    fn new(
        provider: F::Provider,
        foundation_call_tx: tokio::sync::mpsc::Sender<Call<F>>,
    ) -> Self {
        Self {
            provider,
            foundation_call_tx,
        }
    }

    fn proxy(&self) -> Box<dyn Provider<Config=F::Provider::Config>> {
        let kind = self.kind().clone();
        let (prov_call_tx, mut prov_call_rx) = tokio::sync::mpsc::channel(64);
        let foundation_call_tx = self.foundation_call_tx.clone();
        tokio::spawn(async move {
            while let Some(prov_call) = prov_call_rx.recv().await {
                let prov_wrapper = ProvWrapper::new(kind.clone(), prov_call);
                let dep_call = DepCall::ProviderCall(prov_wrapper);
                let dep_wrapper = DepWrapper::new(kind.dep.clone(), dep_call);
                let call = Call::DepCall(dep_wrapper);
                foundation_call_tx.send(call).await.unwrap_or_default();
            }
        });
        Box::new(ProviderTx::new(
            self.provider.config().clone(),
            prov_call_tx,
            self.provider.status_watcher(),
        ))
    }

    fn kind(&self) -> &ProviderKind {
        self.provider.kind()
    }

    async fn handle(&mut self, call: ProviderCall<F>) {
        match call {
            ProviderCall::Initialize { progress, rtn } => {
                rtn.send(self.provider.initialize(progress).await)
                    .unwrap_or_default();
            }
            ProviderCall::Start { progress, rtn } => {
                rtn.send(self.provider.start(progress).await)
                    .unwrap_or_default();
            }
            ProviderCall::_Phantom(_) => {}
        }
    }
}