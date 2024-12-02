use crate::hyperspace::foundation::config::{Config, DependencyConfig, ProviderConfig};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, ProviderKind};
use crate::hyperspace::foundation::state::State;
use crate::hyperspace::foundation::{config, Dependency, Foundation, LiveService, Provider};
use crate::hyperspace::reg::Registry;
use crate::space::parse::CamelCase;
use crate::space::point::Point;
use crate::space::progress::Progress;
use starlane_primitive_macros::logger;
use std::collections::HashMap;

#[derive(Clone)]
struct FoundationConfig<C>
where
    C: config::FoundationConfig + Sized,
{
    config: C,
}

impl<C> FoundationConfig<C>
where
    C: config::FoundationConfig + Sized,
{
    pub fn new(config: C) -> Self {
        Self { config }
    }
}

impl<C> config::FoundationConfig for FoundationConfig<C>
where
    C: config::FoundationConfig + Sized,
{
    fn kind(&self) -> &FoundationKind {
        self.config.kind()
    }

    fn dependency_kinds(&self) -> &Vec<DependencyKind> {
        self.dependency_kinds()
    }

    fn dependency(&self, kind: &DependencyKind) -> Option<&impl DependencyConfig> {
        self.dependency(kind)
    }
}

enum Call {
    Dependency {
        kind: DependencyKind,
        rtn: tokio::sync::oneshot::Sender<Result<Option<dyn Dependency>, FoundationErr>>,
    },

    InstallFoundationRequiredDependencies {
        progress: Progress,
        rtn: tokio::sync::oneshot::Sender<Result<(), FoundationErr>>,
    },
    Registry(tokio::sync::oneshot::Sender<Result<Registry, FoundationErr>>),
    DepCall(DepWrapper),
}

struct FoundationProxy {
    config: dyn config::FoundationConfig,
    call_tx: tokio::sync::mpsc::Sender<Call>,
}

impl FoundationProxy {
    fn new(
        config: impl config::FoundationConfig,
        call_tx: tokio::sync::mpsc::Sender<Call>,
    ) -> Self {
        let config = FoundationConfig::new(config);
        Self { config, call_tx }
    }
}

impl Foundation for FoundationProxy {
    fn kind(&self) -> &FoundationKind {
        self.config.kind()
    }

    fn config(&self) -> &impl config::FoundationConfig {
        &self.config
    }

    fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, rx) = tokio::sync::oneshot::channel();
        self.call_tx
            .try_send(Call::InstallFoundationRequiredDependencies { progress, rtn })
            .map_err(FoundationErr::msg)?;
        Ok(rx.blocking_recv().map_err(FoundationErr::msg)??)
    }

    fn dependency(&self, kind: &DependencyKind) -> Result<Option<impl Dependency>, FoundationErr> {
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
struct Callable<C, S> {
    pub(crate) config: C,
    pub(crate) call_tx: tokio::sync::mpsc::Sender<C>,
}

impl<C, S> Callable<C, S> {
    pub fn new(config: C, call_tx: tokio::sync::mpsc::Sender<C>) -> Callable<C, S> {
        Self { config, call_tx }
    }
}

enum DepCall {
    QueryState(tokio::sync::oneshot::Sender<State>),
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
        rtn: tokio::sync::oneshot::Sender<Result<dyn Provider, FoundationErr>>,
    },
    ProviderCall(ProvWrapper),
}

struct DependencyProxy {
    config: dyn DependencyConfig,
    call_tx: tokio::sync::mpsc::Sender<DepCall>,
}

impl DependencyProxy {
    fn new(
        config: impl DependencyConfig,
        call_tx: tokio::sync::mpsc::Sender<DepCall>,
    ) -> impl Dependency {
        Self { config, call_tx }
    }
}

impl Dependency for DependencyProxy {
    fn kind(&self) -> &DependencyKind {
        self.config.kind()
    }

    fn config(&self) -> &impl DependencyConfig {
        &self.config
    }

    fn state(&self) -> &State {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::QueryState(rtn);
        self.call_tx.try_send(call).map_err(FoundationErr::msg)?;
        let state = rtn_rx.blocking_recv().map_err(FoundationErr::msg)?;
        &state
    }

    async fn install(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Install { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn download(&self, progress: Progress) -> Result<(), FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = DepCall::Download { progress, rtn };
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

    fn provider(&self, kind: &ProviderKind) -> Result<Option<impl Provider>, FoundationErr> {
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
    State(tokio::sync::oneshot::Sender<State>),
}

struct ProviderProxy {
    config: dyn ProviderConfig,
    call_tx: tokio::sync::mpsc::Sender<ProviderCall>,
}

impl ProviderProxy {
    fn new(
        config: impl ProviderConfig,
        call_tx: tokio::sync::mpsc::Sender<ProviderCall>,
    ) -> impl Provider {
        Self { config, call_tx }
    }
}

impl Provider for ProviderProxy {
    fn kind(&self) -> &ProviderKind {
        &self.config.kind()
    }

    fn config(&self) -> &impl ProviderConfig {
        &self.config
    }

    fn state(&self) -> &State {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = ProviderCall::State(rtn);
        self.call_tx.try_send(call).map_err(FoundationErr::msg)?;
        let state = rtn_rx.blocking_recv().map_err(FoundationErr::msg)?;
        &state
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
    foundation: dyn Foundation,
    runners: HashMap<DependencyKind, DependencyRunner>,
}

impl Runner {
    fn new(foundation: impl Foundation) -> impl Foundation {
        let (call_tx, call_rx) = tokio::sync::mpsc::channel(64);
        let config = foundation.config().clone();
        let proxy = FoundationProxy::new(config, call_tx.clone());
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
    ) -> Result<Option<impl Dependency>, FoundationErr> {
        if !self.runners.contains_key(&kind) {
            match self.foundation.dependency(&kind) {
                Ok(None) => return Ok(None),
                Err(err) => return Err(err),
                Ok(Some(dep)) => {
                    let runner = DependencyRunner::new(dep,self.call_tx.clone());
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
        let dep = DependencyProxy::new(runner.dependency.config().clone(), dep_call_tx);

        Ok(Some(dep))
    }

    async fn run(mut self) -> Result<(), FoundationErr> {
        let logger = logger!(Point::global_foundation());
        while let Some(call) = self.call_rx.recv().await {
            match call {
                Call::Dependency { kind, rtn } => {
                    rtn.send(self.dependency(kind)).unwrap_or_default();
                }

                Call::InstallFoundationRequiredDependencies { progress, rtn } => {
                    rtn.send(self.foundation.install(progress))
                        .unwrap_or_default();
                }
                Call::Registry(rtn) => {
                    rtn.send(self.foundation.registry()).unwrap_or_default();
                }
                Call::DepCall(wrap) => {
                    match self.runners.get_mut(&wrap.kind) {
                        None => {

                        }
                        Some(runner) => {
                            runner.handle(wrap.call).await
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

struct DependencyRunner {
    dependency: dyn Dependency,
    runners: HashMap<ProviderKind, ProviderRunner>,
    call_tx: tokio::sync::mpsc::Sender<Call>,
}

impl DependencyRunner {
    fn new(dependency: impl Dependency, call_tx: tokio::sync::mpsc::Sender<Call>) -> Self {
        Self {
            dependency,
            runners: Default::default(),
            call_tx,
        }
    }

    fn kind(&self) -> &DependencyKind {
        self.dependency.kind()
    }

    fn provider(&mut self, kind: ProviderKind) -> Result<Option<impl Dependency>, FoundationErr> {
        if !self.runners.contains_key(&kind) {
            match self.dependency.provider(&kind) {
                Ok(None) => return Ok(None),
                Err(err) => return Err(err),
                Ok(Some(dep)) => {
                    let runner = ProviderRunner::new(dep);
                    self.runners.insert(kind.clone(), runner);
                }
            }
        }

        let _kind = kind.clone();
        let (prov_call_tx, mut prov_call_rx) = tokio::sync::mpsc::channel(64);
        let foundation_call_tx = self.call_tx.clone();
        tokio::spawn(async move {
            while let Some(prov_call) = prov_call_rx.recv().await {
                let prov_wrapper = ProvWrapper::new(kind.clone(), prov_call);
                let dep_call = DepCall::ProviderCall(prov_wrapper);
                let dep_wrapper = DepWrapper::new(kind.dep.clone(), dep_call);
                let call = Call::DepCall(dep_wrapper);
                foundation_call_tx.send(call).await.unwrap_or_default();
            }
        });

        let runner = self.runners.get(&kind).unwrap();
        let dep = ProviderProxy::new(runner.provider.config().clone(), prov_call_tx);

        Ok(Some(dep))
    }

    async fn handle(&mut self, call: DepCall) {
        match call {
            DepCall::QueryState(rtn) => {
                rtn.send(self.dependency.state().clone())
                    .unwrap_or_default();
            }
            DepCall::Download { progress, rtn } => {
                rtn.send(self.dependency.download(progress).unwrap())
                    .unwrap_or_default();
            }
            DepCall::Install { progress, rtn } => {
                rtn.send(self.dependency.install(progress).unwrap())
                    .unwrap_or_default();
            }
            DepCall::Initialize { progress, rtn } => {
                rtn.send(self.dependency.initialize(progress).unwrap())
                    .unwrap_or_default();
            }
            DepCall::Start { progress, rtn } => {
                rtn.send(self.dependency.start(progress).unwrap())
                    .unwrap_or_default();
            }
            DepCall::Provider { kind, rtn } => {
                rtn.send(self.provider(kind)).unwrap_or_default();
            }
            DepCall::ProviderCall(call) => match self.runners.get_mut(&call.kind) {
                None => {}
                Some(provider) => {
                    provider.handle(call.call).await;
                }
            },
        }
    }
}

struct ProviderRunner {
    provider: Box<dyn Provider>,
}

impl ProviderRunner {
    fn new(provider: impl Provider + Sized) -> Self {
        let provider = Box::new(provider);
        Self { provider }
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
            ProviderCall::State(rtn) => {
                rtn.send(self.provider.state().clone()).unwrap_or_default();
            }
        }
    }
}
