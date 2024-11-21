use std::collections::HashSet;
use crate::hyperspace::foundation::{DependencyKind, FoundationErr, FoundationKind, ProviderKind, RegistryProvider};
use crate::hyperspace::foundation::config::{ProtoDependencyConfig, ProtoProviderConfig};
use crate::hyperspace::foundation::traits::{Dependency, Foundation, Provider};

pub(in crate::hyperspace::foundation) enum Call {
     Kind(tokio::sync::oneshot::Sender<FoundationKind>),
     Dependency{ kind: DependencyKind, rtn: tokio::sync::oneshot::Sender<Result<dyn Dependency,FoundationErr>>},
     InstallFoundationRequiredDependencies(tokio::sync::oneshot::Sender<Result<(),FoundationErr>>),
     AddDependency{ config: ProtoDependencyConfig, rtn: tokio::sync::oneshot::Sender<Result<dyn Dependency,FoundationErr>>},
     DepCall(DepCallWrapper),
 }

struct FoundationProxy {
     call_tx: tokio::sync::mpsc::Sender<Call>,
 }

impl FoundationProxy {
     fn new(call_tx:tokio::sync::mpsc::Sender<Call>) -> Self {
         Self {
             call_tx
         }
     }

 }

impl Foundation for FoundationProxy {

     fn kind(&self) -> FoundationKind {
         let (rtn_tx, mut rtn_rx) = tokio::sync::oneshot::channel();
         let call = Call::Kind(rtn_tx);
         self.call_tx.try_send(call).unwrap_or_default();
         rtn_rx.try_recv().unwrap()
     }

     fn dependency(&self, kind: &DependencyKind) -> Result<impl Dependency, FoundationErr> {
         let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
         let kind = kind.clone();
         let call = Call::Dependency { kind, rtn };
         self.call_tx.try_send(call)?;
         Ok(rtn_rx.try_recv()?)
     }

     async fn install_foundation_required_dependencies(&mut self) -> Result<(), FoundationErr> {
         let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
         let call = Call::InstallFoundationRequiredDependencies(rtn);
         self.call_tx.send(call).await?;
         rtn_rx.await?
     }

     async fn add_dependency(&mut self, config: ProtoDependencyConfig) -> Result<impl Dependency, FoundationErr> {
         let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
         let call = Call::AddDependency{ config, rtn };
         self.call_tx.send(call).await?;
         rtn_rx.await?
     }

     fn registry(&self) -> &mut impl RegistryProvider {
         todo!()
     }
 }

pub(in crate::hyperspace::foundation) struct DepCallWrapper {
    kind: DependencyKind,
    command: DepCall
}

impl DepCallWrapper {
     fn new(kind: DependencyKind, command: DepCall) -> Self {
         Self {
             kind,
             command,
         }
     }
 }

pub(in crate::hyperspace::foundation) enum DepCall {
     Kind(tokio::sync::oneshot::Sender<DependencyKind>),
     Install(tokio::sync::oneshot::Sender<Result<(),FoundationErr>>),
     Provision{ config: ProtoProviderConfig, rtn: tokio::sync::oneshot::Sender<Result<dyn Provider,FoundationErr>>},
     ProviderKinds(tokio::sync::oneshot::Sender<HashSet<&'static str>>),
     ProviderCall(ProviderCallWrapper)
 }

struct DependencyProxy {
     kind: DependencyKind,
     call_tx: tokio::sync::mpsc::Sender<DepCall>,
 }

impl DependencyProxy {
     fn new(kind: &DependencyKind, foundation_call_tx: tokio::sync::mpsc::Sender<Call>) -> impl Dependency{
         let (call_tx,mut call_rx) = tokio::sync::mpsc::channel(16);

         tokio::spawn( async move {
            while let Some(call) = call_rx.recv().await {
                let call = DepCallWrapper::new(kind.clone(), call);
                let call = Call::DepCall(call);
                foundation_call_tx.send(call).await.unwrap_or_default();
            }
         });

         let kind = kind.clone();

         Self {
             kind,
             call_tx
         }
     }
 }

impl Dependency for DependencyProxy {
     fn kind(&self) -> &DependencyKind {
         &self.kind
     }


     async fn install(&self) -> Result<(), FoundationErr> {
         let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
         let call = DepCall::Install(rtn);
         self.call_tx.send(call).await.unwrap();
         rtn_rx.await?
     }

     /*
     async fn provision(&self, kind: &ProviderKind, config: Value ) -> Result<impl Provider,FoundationErr> {
         let kind = kind.clone();
         let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
         let call = DepCall::Provision{ kind, rtn };
         self.call_tx.send(call).await.unwrap();
         rtn_rx.await?
     }

      */

     /// implementers of this Trait should provide a vec of valid provider kinds
     fn provider_kinds(&self) -> HashSet<&'static str> {
         let (rtn_tx, mut rtn_rx) = tokio::sync::oneshot::channel();
         let call = DepCall::ProviderKinds(rtn_tx);
         self.call_tx.try_send(call).unwrap_or_default();
         rtn_rx.try_recv().unwrap()
     }
 }

struct ProviderCallWrapper {
     kind: ProviderKind,
     call: ProviderCall
 }

impl ProviderCallWrapper {
     fn new(kind: ProviderKind, command: ProviderCall) -> Self {
         Self {
             kind,
             call: command,
         }
     }
 }

pub(in crate::hyperspace::foundation) enum ProviderCall {
     Initialize(tokio::sync::oneshot::Sender<Result<(),FoundationErr>>),
 }

struct ProviderProxy {
    dependency: DependencyKind,
    kind: ProviderKind,
    call_tx: tokio::sync::mpsc::Sender<ProviderCall>,
}

impl ProviderProxy {
    fn new(kind: &ProviderKind, dependency: &DependencyKind, foundation_call_tx: tokio::sync::mpsc::Sender<Call>) -> impl Provider{
        let (call_tx,mut call_rx) = tokio::sync::mpsc::channel(16);

        let dep_kind = dependency.clone();
        tokio::spawn( async move {
            while let Some(call) = call_rx.recv().await {
                let call = ProviderCallWrapper::new(kind.clone(), call);
                let call = DepCall::ProviderCall(call);
                let call = DepCallWrapper::new(dep_kind.clone(), call);
                let call = Call::DepCall(call);
                foundation_call_tx.send(call).await.unwrap_or_default();
            }
        });

        let kind = kind.clone();
        let dependency = dependency.clone();

        Self {
            kind,
            dependency,
            call_tx
        }
    }
}

impl Provider for ProviderProxy {
    async fn initialize(&mut self) -> Result<(), FoundationErr> {
        let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
        let call = ProviderCall::Initialize(rtn);
        self.call_tx.send(call).await.unwrap();
        rtn_rx.await?
    }
}

pub(in crate::hyperspace::foundation) struct Runner {
    call_rx: tokio::sync::mpsc::Receiver<Call>,
    foundation: dyn Foundation
}

impl Runner {

    pub(in crate::hyperspace::foundation) fn new(foundation: impl Foundation ) -> impl Foundation {
        let (call_tx, call_rx) = tokio::sync::mpsc::channel(64);
        let proxy = FoundationProxy::new(call_tx);
        let runner = Self {
            foundation,
            call_rx
        };
        runner.start();
        proxy
    }

    pub(in crate::hyperspace::foundation) fn start(self) {
        tokio::spawn( async move {
            self.run().await;
        });
    }

    async fn run(mut self) -> Result<(),FoundationErr> {
        let logger = logger!(Point::global_foundation());
        while let Some(call) = self.call_rx.recv().await {
            match call {
                Call::Kind(rtn) => {
                    rtn.send(self.foundation.kind()).unwrap_or_default();
                }
                Call::Dependency { kind, rtn } => {}
                Call::InstallFoundationRequiredDependencies(_) => {}
                Call::AddDependency { .. } => {}
                Call::DepCall(_) => {}
            }
        }
        Ok(())
    }

}

struct DependencyRunner {
    dependency: Box<dyn Dependency>,
}

impl DependencyRunner {

    fn new( dependency: impl Dependency ) -> Self {
        let dependency = Box::new(dependency);
        Self {
            dependency
        }
    }

    fn kind(&self) -> &DependencyKind {
        self.dependency.kind()
    }

    async fn handle(&mut self, call: DepCall) -> anyhow::Result<()> {
        match call {
            DepCall::Kind(rtn) => rtn.send(self.dependency.kind().clone()).unwrap_or_default(),
            DepCall::Install(rtn) => {
                rtn.send(self.dependency.install().await).unwrap_or_default();
            },
            DepCall::Provision { config, rtn } => {
                match self.dependency.provision(config).await {
                    Ok(provider) => {
                    }
                    Err(err) => rtn.send(Err(err)).unwrap_or_default()
                }
            }
            DepCall::ProviderKinds(rtn) => {
                let mut set = HashSet::new();
                set.insert(ProviderKind::DockerDaemon.to_string().as_str());
                rtn.send(set).unwrap_or_default();
            }
            DepCall::ProviderCall(call) => {

            }
        }
        Ok(())
    }
}

struct ProviderRunner {
    provider: dyn Provider,
}

impl ProviderRunner {
    fn kind(&self) -> &ProviderKind {
        self.provider.kind()
    }

    async fn handle(&mut self, call: ProviderCall) -> anyhow::Result<()> {
        match call {
            ProviderCall::Initialize(rtn) => {
                rtn.send(self.provider.initialize()).unwrap_or_default();
            }
        }
        Ok(())
    }
}