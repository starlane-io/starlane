use crate::base;
use starlane_space::parse::CamelCase;
use starlane_space::point::Point;
use starlane_space::progress::Progress;
use starlane_macros::logger;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::watch::Receiver;
use starlane_hyperspace::base::config::BaseConfig;
use starlane_hyperspace::base::{BaseSub, Foundation};
use starlane_hyperspace::base::kinds::ProviderKind;
use starlane_hyperspace::base::provider::Provider;
use starlane_space::status::{EntityReadier, EntityResult, Status, StatusProbe, StatusResult};
use crate::backend::call::Call;
use crate::backend::relay::FoundationTx;


struct Wrapper<K, C> {
    kind: K,
    call: C,
}
impl<K, C> Wrapper<K, C> {
    fn new(kind: K, call: C) -> Self {
        Self { kind, call }
    }
}

struct ProviderTx<P>
where
    P: Provider,
{
//    config: <P as BaseSub>::Config,
    call_tx: tokio::sync::mpsc::Sender<Call<P>>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl<P> ProviderTx<P>
where
    P: Provider,
{
    fn new(
        call_tx: tokio::sync::mpsc::Sender<Call<P>>,
        status: Arc<tokio::sync::watch::Receiver<Status>>,
    ) -> Self {
        Self {
            call_tx,
            status,
        }
    }
}


impl<P> StatusProbe for ProviderTx<P>
where
    P: Provider,
{
    async fn probe(&self) -> StatusResult {
        todo!()
    }
}

impl<P> EntityReadier for ProviderTx<P>
where
    P: Provider,
{
    type Entity = ();

    async fn ready(&self) -> EntityResult<Self::Entity> {
        todo!()
    }
}

#[async_trait]
impl<P> Provider for ProviderTx<P>
where
    P: Provider {

    fn provider_kind(&self) -> &ProviderKind {
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
        let call = Call::Initialize { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }

    async fn start(&self, progress: Progress) -> Result<LiveService<CamelCase>, BaseErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = Call::Start { progress, rtn };
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }
}

struct Runner<F>
where
    F: Foundation,
{
    call_rx: tokio::sync::mpsc::Receiver<Method<F>>,
    call_tx: tokio::sync::mpsc::Sender<Method<F>>,
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
                Method::Probe { progress, rtn } => {
                    rtn.send(self.foundation.synchronize(progress).await)
                        .unwrap_or_default();
                }
                Method::MakeReady { kind, rtn } => {
                    rtn.send(self.proxy(kind)).unwrap_or_default();
                }
                Method::Install { progress, rtn } => {
                    rtn.send(self.foundation.install(progress).await)
                        .unwrap_or_default();
                }
                Method::Registry(rtn) => {
                    rtn.send(self.foundation.registry()).unwrap_or_default();
                }
                Method::DepCall(wrap) => match self.runners.get_mut(&wrap.kind) {
                    None => {}
                    Some(runner) => runner.handle(wrap.call).await,
                },
                /// should never be called...
                Method::_Phantom(_) => {}
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
    call_tx: tokio::sync::mpsc::Sender<Method<F::Dependency>>,
}

impl<F> DependencyRunner<F>
where
    F: Foundation,
{
    fn new(dependency: F::Dependency, call_tx: tokio::sync::mpsc::Sender<Method<F>>) -> Self {
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
                let call = Method::DepCall(prov_wrapper);
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
    foundation_call_tx: tokio::sync::mpsc::Sender<Method<F>>,
}

impl<F> ProviderRunner<F>
where
    F: Foundation,
{
    fn new(
        provider: F::Provider,
        foundation_call_tx: tokio::sync::mpsc::Sender<Method<F>>,
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
                let call = Method::DepCall(dep_wrapper);
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

    async fn handle(&mut self, call: Call<F>) {
        match call {
            Call::Initialize { progress, rtn } => {
                rtn.send(self.provider.initialize(progress).await)
                    .unwrap_or_default();
            }
            Call::Start { progress, rtn } => {
                rtn.send(self.provider.start(progress).await)
                    .unwrap_or_default();
            }
            Call::_Phantom(_) => {}
        }
    }
}
