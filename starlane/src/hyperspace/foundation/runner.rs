use crate::hyperspace::foundation::config::{Config, DependencyConfig, ProviderConfig};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, Kind, ProviderKind};
use crate::hyperspace::foundation::status::{Phase, Status, StatusDetail};
use crate::hyperspace::foundation::{config, Dependency, Foundation, LiveService, Provider};
use crate::hyperspace::reg::Registry;
use crate::space::parse::CamelCase;
use crate::space::point::Point;
use crate::space::progress::Progress;
use starlane_primitive_macros::logger;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::watch::Receiver;
use wasmer_wasix::virtual_fs::Upcastable;

enum Call {
    Synchronize {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<Status, FoundationErr>>,
    },
    Dependency {
        kind: DependencyKind,
        rtn: tokio::sync::oneshot::Sender<Result<Option<Box<dyn Dependency>>, FoundationErr>>,
    },

    Install {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), FoundationErr>>,
    },
    Registry(tokio::sync::oneshot::Sender<Result<Registry, FoundationErr>>),
    DepCall(DepWrapper),
}

struct FoundationProxy {
    config: Box<dyn config::FoundationConfig>,
    call_tx: tokio::sync::mpsc::Sender<Call>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl FoundationProxy {
    fn new(
        config: Box<dyn config::FoundationConfig>,
        call_tx: tokio::sync::mpsc::Sender<Call>,
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
impl Foundation for FoundationProxy {
    fn kind(&self) -> &FoundationKind {
        self.config.kind()
    }

    fn config(&self) -> &Box<dyn config::FoundationConfig> {
        &self.config
    }

    fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.status.clone()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status, FoundationErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Call::Synchronize { progress, rtn })
            .map_err(FoundationErr::msg)?;
        rx.await.map_err(FoundationErr::msg)?
    }

    async fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Call::Install { progress, rtn })
            .map_err(FoundationErr::msg)?;
        rx.await.map_err(FoundationErr::msg)?
    }

    fn dependency(
        &self,
        kind: &DependencyKind,
    ) -> Result<Option<Box<dyn Dependency>>, FoundationErr> {
        let kind = kind.clone();
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Call::Dependency { kind, rtn })
            .map_err(FoundationErr::msg)?;
        Ok(rx.blocking_recv().map_err(FoundationErr::msg)??)
    }

    fn registry(&self) -> Result<Registry, FoundationErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Call::Registry(rtn))
            .map_err(FoundationErr::msg)?;
        rx.blocking_recv().map_err(FoundationErr::msg)?
    }
}

type DepWrapper = Wrapper<DependencyKind, DepCall>;
type ProvWrapper = Wrapper<ProviderKind, ProviderCall>;

struct Wrapper<K, C> {
    kind: K,
    call: C,
}
impl<K, C> Wrapper<K, C> {
    fn new(kind: K, call: C) -> Self {
        Self { kind, call }
    }
}

enum DepCall {
    Download {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), FoundationErr>>,
    },
    Install {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), FoundationErr>>,
    },
    Initialize {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), FoundationErr>>,
    },
    Start {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<LiveService<DependencyKind>, FoundationErr>>,
    },
    Provider {
        kind: ProviderKind,
        rtn: tokio::sync::oneshot::Sender<Result<Option<Box<dyn Provider>>, FoundationErr>>,
    },
    ProviderCall(ProvWrapper),
}

struct DependencyProxy {
    config: Box<dyn DependencyConfig>,
    call_tx: tokio::sync::mpsc::Sender<DepCall>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl DependencyProxy {
    fn new(
        config: Box<dyn DependencyConfig>,
        call_tx: tokio::sync::mpsc::Sender<DepCall>,
        status: Arc<tokio::sync::watch::Receiver<Status>>,
    ) -> impl Dependency {
        Self {
            config,
            call_tx,
            status,
        }
    }
}

#[async_trait]
impl Dependency for DependencyProxy {
    fn kind(&self) -> &DependencyKind {
        self.config.kind()
    }

    fn config(&self) -> &Box<dyn DependencyConfig> {
        &self.config
    }

    fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.status.clone()
    }

    async fn download(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Download { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Install { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn initialize(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Initialize { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn start(
        &self,
        progress: Progress,
    ) -> Result<LiveService<DependencyKind>, FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Start { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<dyn Provider>>, FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let kind = kind.clone();
        let call = DepCall::Provider { kind, rtn };
        self.call_tx.try_send(call).map_err(FoundationErr::msg)?;
        rtn_rx.blocking_recv().map_err(FoundationErr::msg)?
    }
}

enum ProviderCall {
    Initialize {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), FoundationErr>>,
    },
    Start {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<LiveService<CamelCase>, FoundationErr>>,
    },
}

struct ProviderProxy {
    config: Box<dyn ProviderConfig>,
    call_tx: tokio::sync::mpsc::Sender<ProviderCall>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl ProviderProxy {
    fn new(
        config: Box<dyn ProviderConfig>,
        call_tx: tokio::sync::mpsc::Sender<ProviderCall>,
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
impl Provider for ProviderProxy {
    fn kind(&self) -> &ProviderKind {
        &self.config.kind()
    }

    fn config(&self) -> &Box<dyn ProviderConfig> {
        &self.config
    }

    fn status(&self) -> Status {
        self.status().borrow().clone()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.status.clone()
    }

    async fn initialize(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = ProviderCall::Initialize { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn start(&self, progress: Progress) -> Result<LiveService<CamelCase>, FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = ProviderCall::Start { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }
}

struct Runner {
    call_rx: tokio::sync::mpsc::Receiver<Call>,
    call_tx: tokio::sync::mpsc::Sender<Call>,
    foundation: Box<dyn Foundation>,
    runners: HashMap<DependencyKind, DependencyRunner>,
}

impl Runner {
    fn new(foundation: Box<dyn Foundation>) -> impl Foundation {
        let (call_tx, call_rx) = tokio::sync::mpsc::channel(64);
        let config = foundation.config().clone_me();
        let proxy = FoundationProxy::new(config, call_tx.clone(), foundation.status_watcher());
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
    ) -> Result<Option<Box<dyn Dependency>>, FoundationErr> {
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

        let (dep_call_tx, mut dep_call_rx) = tokio::sync::mpsc::channel(64);
        let foundation_call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            while let Some(call) = dep_call_rx.recv().await {
                let wrapper = DepWrapper::new(kind.clone(), call);
                let call = Call::DepCall(wrapper);
                foundation_call_tx.send(call).await.unwrap_or_default();
            }
        });

        let runner = self.runners.get(&kind).unwrap();
        let config = runner.dependency.config().clone_me();
        let dep = DependencyProxy::new(config, dep_call_tx, runner.dependency.status_watcher());

        Ok(Some(dep))
    }

    async fn run(mut self) -> Result<(), FoundationErr> {
        let logger = logger!(Point::global_foundation());
        while let Some(call) = self.call_rx.recv().await {
            match call {
                Call::Synchronize { progress, rtn } => {
                    rtn.send(self.foundation.synchronize(progress).await)
                        .unwrap_or_default();
                }
                Call::Dependency { kind, rtn } => {
                    rtn.send(self.dependency(kind)).unwrap_or_default();
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
            }
        }
        Ok(())
    }
}

struct DependencyRunner {
    dependency: Box<dyn Dependency>,
    runners: HashMap<ProviderKind, ProviderRunner>,
    call_tx: tokio::sync::mpsc::Sender<Call>,
}

impl DependencyRunner {
    fn new(dependency: Box<dyn Dependency>, call_tx: tokio::sync::mpsc::Sender<Call>) -> Self {
        Self {
            dependency,
            runners: Default::default(),
            call_tx,
        }
    }

    fn kind(&self) -> &DependencyKind {
        self.dependency.kind()
    }

    fn provider(
        &mut self,
        kind: ProviderKind,
    ) -> Result<Option<&mut ProviderRunner>, FoundationErr> {
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
    fn proxy(&mut self, kind: ProviderKind) -> Result<Option<Box<dyn Provider>>, FoundationErr> {
        let runner = self
            .provider(kind.clone())?
            .ok_or(FoundationErr::provider_not_available(kind))?;
        Ok(Some(runner.proxy()))
    }

    async fn handle(&mut self, call: DepCall) {
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
                rtn.send(self.proxy(kind)).unwrap_or_default();
            }
            DepCall::ProviderCall(wrap) => {
                if let Some(provider) = self.provider(wrap.kind).unwrap_or_default() {
                    provider.handle(wrap.call).await;
                }
            }
        }
    }
}

struct ProviderRunner {
    provider: Box<dyn Provider>,
    foundation_call_tx: tokio::sync::mpsc::Sender<Call>,
}

impl ProviderRunner {
    fn new(
        provider: Box<dyn Provider>,
        foundation_call_tx: tokio::sync::mpsc::Sender<Call>,
    ) -> Self {
        let provider = Box::new(provider);
        Self {
            provider,
            foundation_call_tx,
        }
    }

    fn proxy(&self) -> Box<dyn Provider> {
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
        Box::new(ProviderProxy::new(
            self.provider.config().clone_me(),
            prov_call_tx,
            self.provider.status_watcher(),
        ))
    }

    fn kind(&self) -> &ProviderKind {
        self.provider.kind()
    }

    async fn handle(&mut self, call: ProviderCall) {
        match call {
            ProviderCall::Initialize { progress, rtn } => {
                rtn.send(self.provider.initialize(progress).await)
                    .unwrap_or_default();
            }
            ProviderCall::Start { progress, rtn } => {
                rtn.send(self.provider.start(progress).await)
                    .unwrap_or_default();
            }
        }
    }
}
