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
use starlane_hyperspace::base::provider::{Provider, ProviderKindDisc};
use starlane_space::status::{EntityReadier, EntityResult, Status, StatusProbe, StatusResult};
use crate::backend::Backend;
use crate::backend::call::Call;
use crate::backend::provider::Method;
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

struct ProviderTx<B>
where
    B: Backend
{
//    config: <P as BaseSub>::Config,
    call_tx: tokio::sync::mpsc::Sender<Call<B::Method>>,
    status: Arc<tokio::sync::watch::Receiver<Status>>,
}

impl<B> ProviderTx<B>
where
    B: Backend,
{
    fn new(
        call_tx: tokio::sync::mpsc::Sender<Call<B::Method>>,
        status: Arc<tokio::sync::watch::Receiver<Status>>,
    ) -> Self {
        Self {
            call_tx,
            status,
        }
    }
}


#[async_trait]
impl<B> StatusProbe for ProviderTx<B>
where
    B: Backend {
    async fn probe(&self) -> StatusResult {
        todo!()
    }
}

impl <B> BaseSub for ProviderTx<B> where B: Backend {
    
}

#[async_trait]
impl<B> Provider for ProviderTx<B>
where
    B: Backend<Method=Call<String>,Result=EntityResult<()>>{

    /*
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
    
     */
}

struct Runner<F>
where
    F: Foundation,
{
    call_rx: tokio::sync::mpsc::Receiver<Method>,
    call_tx: tokio::sync::mpsc::Sender<Method>,
    foundation: F,
//    runners: HashMap<ProviderKind, DependencyRunner<F>>,
}

impl<F> Runner<F>
where
    F: Foundation,
{
    /*
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
    
     */

    fn start(self) {
        tokio::spawn(async move {
            todo!();
            //self.run().await;
        });
    }

    /*

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
    
     */
}


/*
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

 */
