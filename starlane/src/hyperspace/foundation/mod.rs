pub mod docker;
pub mod config;

use crate::hyperspace::foundation::docker::DockerDesktopFoundation;
use crate::hyperspace::platform::PlatformConfig;
use crate::hyperspace::reg::Registry;
use async_trait::async_trait;
use derive_builder::Builder;
use futures::TryFutureExt;
use itertools::Itertools;
use once_cell::sync::Lazy;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::process;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use ascii::AsciiChar::K;
use thiserror::Error;


/// # FOUNDATION
/// A ['Foundation'] provides abstracted control over the services and dependencies that drive Starlane.
/// Presently there is only the ['DockerDesktopFoundation'] which uses a local Docker Service
/// to pull dependent Docker Images, run docker instances and in general enables the Starlane [`Platform`]
/// manage the lifecycle of arbitrary services.
///
/// There are two sub concepts that ['Foundation'] provides: ['Dependency'] and  ['Provider'].
/// The [`FoundationConfig`] enumerates dependencies which are typically things that don't ship
/// with the Starlane binary.  Common examples are: Postgres, Keycloak, Docker.  Each foundation
/// implementation must know how to ready that Dependency and potentially even launch an
/// instance of that Dependency.  For Example: Postgres Database is a common dependency especially
/// because the default Starlane [`Registry`] (and at the time of this writing the only Registry support).
/// The Postgres [`Dependency`] ensures that Postgres is accessible and properly configured for the
/// Starlane Platform.
///
/// ## ADDING DEPENDENCIES
/// Additional Dependencies can be added via [`Foundation::add_dependency`]  The Foundation
/// implementation must understand how to get the given [`DependencyKind`] and it's entirely possible
/// that the supported Dependencies differ from Foundation to Foundation.
///
/// ## PROVIDER
/// A [`Dependency`] has a one to many child concept called a [`Provider`] (poorly named!)  Not all Dependencies
/// have a Provider.  A Provider is something of an instance of a given Dependency.... For example:
/// The Postgres Cluster [`DependencyKind::Postgres`]  (talking the actual postgresql software which can serve multiple databases)
/// The Postgres Dependency may have multiple Databases ([`ProviderKind::Database`]).  The provider
/// utilizes a common Dependency to provide a specific service etc.
///
/// ## THE REGISTRY
/// There is one special dependency that the Foundation must manage which is the [`Foundation::registry`]
/// the Starlane Registry is the only required dependency from the vanilla Starlane installation
///
type CreateFoundation =  dyn FnMut(Value) -> Result<dyn Foundation,FoundationErr> + Sync + Send+ 'static;
type CreateDep =  dyn FnMut(Value) -> Result<dyn Dependency,FoundationErr> + Sync + Send+ 'static;
type CreateProvider =  dyn FnMut(Value) -> Result<dyn Provider,FoundationErr> + Sync + Send+ 'static;


static FOUNDATIONS: Lazy<HashMap<FoundationKind, CreateFoundation>> =
    Lazy::new(|| {
        let mut foundations = HashMap::new();
        foundations.insert(FoundationKind::DockerDesktop, DockerDesktopFoundation::create );
        foundations
    });



static FOUNDATION: Lazy<impl Foundation> =
    Lazy::new(|| {
        let foundation_config = STARLANE_CONFIG.foundation.clone();
        let foundation = match create_foundation(foundation_config) {
            Ok(foundation) => foundation,
            Err(err) => {
                let msg = format!("[PANIC] Starlane instance cannot create Foundation.  Caused by: '{}'", err.is_fatal()).to_string();
                let logger = logger!(Point::global_foundation());
                logger.error(msg);
                process::exit(1);
            }
        };
       foundation
    });

fn create_foundation(config: ProtoFoundationConfig) -> Result<impl Foundation,FoundationErr> {

    Ok(foundation)
}

/// should be called and retained by [`Platform`]
pub(crate) fn foundation(config: ProtoFoundationConfig) -> Result<impl Foundation,FoundationErr>  {
    let foundation = FOUNDATIONS.get(&config.kind).ok_or(FoundationErr::foundation_not_available(config.kind))?(config);
    let foundation = Runner::new(foundation);

    foundation
}











trait Kind where Self: ToString {
  fn identifier() -> &'static str;

  fn create(kind: &str) -> Result<impl Self,FoundationErr>;
}


#[async_trait]
pub trait Foundation: Send + Sync + Sized
where
    Self: Sized {


    fn kind(&self) -> FoundationKind;


    fn dependency(&self, kind: &DependencyKind ) -> Result<impl Dependency,FoundationErr>;

    /// install any 3rd party dependencies this foundation requires to be minimally operable
    async fn install_foundation_required_dependencies(& mut self) -> Result<(), FoundationErr>;

    /// install a named dependency.  For example the dependency might be "Postgres." The implementing Foundation must
    /// be capable of installing that dependency.  The foundation will make the dependency available after installation
    /// although the method of installing the dependency is under the complete control of the Foundation.  For example:
    /// a LocalDevelopmentFoundation might have an embedded Postgres Database and perhaps another foundation: DockerDesktopFoundation
    /// may actually launch a Postgres Docker image and maybe a KubernetesFoundation may actually install a Postgres Operator ...
    async fn add_dependency(&mut self, config: ProtoDependencyConfig ) -> Result<impl Dependency, FoundationErr>;

    /// return the RegistryFoundation
    fn registry(&self) -> &mut impl RegistryProvider;
}




#[derive(Builder, Clone, Serialize, Deserialize)]
pub struct FoundationConfig<C> where C: Deserialize+Serialize{
    pub foundation: FoundationKind,
    pub config: C
}

#[derive(Builder, Clone, Serialize, Deserialize)]
pub struct DependencyConfig<C> where C: Deserialize+Serialize{
    pub dependency: DependencyKind,
    pub config: C
}

#[derive(Builder, Clone, Serialize, Deserialize)]
pub struct ProviderConfig<C> where C: Deserialize+Serialize{
    pub provider: ProviderKind,
    pub config: C
}








use serde::de::{MapAccess, Visitor};
use starlane_primitive_macros::logger;
use crate::env::STARLANE_CONFIG;
use crate::hyperspace::foundation::config::{ProtoDependencyConfig, ProtoFoundationConfig};
use crate::hyperspace::foundation::runner::{Call, DepCallWrapper, Runner};
use crate::space::point::Point;

#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct DependencyConfigProto{
    pub dependency: Value,
    pub config: Value
}



impl DependencyConfigProto {


}

#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct ProviderConfigProto {
    pub provider: Value,
    pub config: Value
}






pub struct RegistryConfig2 {

}


#[derive(Clone,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString)]
pub enum FoundationKind {
    DockerDesktop
}

impl Kind for FoundationKind {
    fn identifier() -> &'static str {
        "foundation"
    }

    fn create(kind: &str) -> Result<impl Kind, FoundationErr> {
        match kind {
            "DockerDesktop" => Ok(Self::DockerDesktop),
            kind => Err(FoundationErr::foundation_not_found(kind))
        }
    }
}



pub trait Dependency {

    fn kind(&self) -> &DependencyKind;


    async fn install(&self) -> Result<(), FoundationErr> {
        Ok(())
    }

    async fn provision(&self, kind: &ProviderKind, _config: Value ) -> Result<impl Provider,FoundationErr> {
        Err(FoundationErr::provider_not_available( kind.clone() ))
    }

    fn has_provisioner(kind: &ProviderKind) -> Result<(),FoundationErr> {
        let providers = Self::provider_kinds();
        match kind {
            kind => {
                let ext = kind.to_string();
                if providers.contains(ext.as_str()) {
                    Ok(())
                } else {
                    let key = ProviderKey::new(Self::kind(), kind.clone());
                    Err(FoundationErr::prov_err(key, format!("provider kind '{}' is not available for dependency: '{}'", ext.to_string(), Self::kind().to_string()).to_string()))
                }
            }
        }
    }

    /// implementers of this Trait should provide a vec of valid provider kinds
    fn provider_kinds(&self) -> HashSet<&'static str> {
        HashSet::new()
    }


}

pub trait Provider {
    async fn initialize(&mut self) -> Result<(), FoundationErr>;
}



pub type RawConfig = Value;





#[derive(Clone,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, Serialize, Deserialize)]
pub enum DependencyKind {
    Postgres,
}


impl Kind for DependencyKind{
    fn identifier() -> &'static str {
        "dependency"
    }

    fn create(kind: &str) -> Result<impl Kind, FoundationErr> {
        match kind {
            "Postgres" => Ok(Self::Postgres),
            kind => Err(FoundationErr::dep_not_found(kind))
        }
    }
}

#[derive(Clone,Eq,PartialEq,Hash)]
pub struct ProviderKey{
    dep: DependencyKind,
    kind: ProviderKind
}

impl ProviderKey {
    pub fn new(dep: DependencyKind, kind: ProviderKind) -> Self {
        Self {
            dep,
            kind,
        }
    }
}

impl ToString for ProviderKey {
    fn to_string(&self) -> String {
        format!("<{}:{}>", self.dep, self.kind)
    }
}

#[derive(Clone,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString)]
pub enum ProviderKind {
    /// this means that the Dependency has one and only one Provider
    Database,
}

impl Kind for ProviderKind{
    fn identifier() -> &'static str {
        "provider"
    }

    fn create(kind: &str) -> Result<impl Kind, FoundationErr> {
        match kind {
            "Database" => Ok(Self::Database),
            kind => Err(FoundationErr::provider_not_found(kind))
        }
    }
}


#[derive(Clone)]
pub struct LiveService<S> where S: Clone{
    pub service: S,
    tx: tokio::sync::mpsc::Sender<()>
}

impl <S> Deref for LiveService<S> {
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl <S> DerefMut for LiveService<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.service
    }
}


#[derive(Error,Clone)]
pub enum FoundationErr {
    #[error("[{id}] -> PANIC! <{kind}> error message: '{msg}'")]
    Panic {id: String, kind: String, msg: String},
    #[error("FoundationConfig.foundation is set to '{0}' which this Starlane build does not recognize")]
    FoundationNotFound(String),
    #[error("foundation: '{0}' is recognized but is not available on this build of Starlane")]
    FoundationNotAvailable(String),
    #[error("DependencyConfig.foundation is set to '{0}' which this Starlane build does not recognize")]
    DepNotFound(String),
    #[error("dependency: '{0}' is recognized but is not available on this build of Starlane")]
    DepNotAvailable(String),
    #[error("ProviderConfig.provider is set to '{0}' which this Starlane build does not recognize")]
    ProviderNotFound(String),
    #[error("provider: '{0}' is recognized but is not available on this build of Starlane")]
    ProviderNotAvailable(String),
    #[error("error converting foundation config for '{kind}' serialization err: '{err}' config: {config}")]
    FoundationConfigErr { kind: FoundationKind,err: Rc<serde_yaml::Error>, config: Rc<Value> },
    #[error("[{kind}] Foundation Error: '{msg}'")]
    FoundationErr{ kind: FoundationKind, msg: String },
    #[error("[{kind}] Error: '{msg}'")]
    DepErr{ kind: DependencyKind, msg: String},
    #[error("[{key}] Error: '{msg}'")]
    ProviderErr{ key: ProviderKey, msg: String},
    #[error("error converting foundation args for dependency: '{kind}' serialization err: '{err}' from config: '{config}'")]
    DepConfErr { kind: DependencyKind,err: Rc<serde_yaml::Error>, config: String},
    #[error("error converting foundation args for provider: '{kind}' serialization err: '{err}' from config: '{config}'")]
    ProvConfErr { kind: ProviderKind, err: Rc<serde_yaml::Error>, config: String},
    #[error("illegal attempt to change foundation after it has already been initialized.  Foundation can only be initialized once")]
    FoundationAlreadyCreated,
    #[error("Foundation Runner call sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerMpscSendErr(Rc<tokio::sync::mpsc::error::SendError<Call>>),
    #[error("Foundation Runner return sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerOneshotRecvErr(Rc<tokio::sync::oneshot::error::RecvError>),
    #[error("Foundation Runner call sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerMpscTrySendErr(Rc<tokio::sync::mpsc::error::TrySendError<Call>>),
    #[error("Foundation Runner return sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerOneshotTryRecvErr(Rc<tokio::sync::oneshot::error::TryRecvError>),
}



impl From<tokio::sync::mpsc::error::SendError<Call>> for FoundationErr {
    fn from(err: tokio::sync::mpsc::error::SendError<Call>) -> Self {
        Self::FoundationRunnerMpscSendErr(Rc::new(err))
    }
}


impl From<tokio::sync::mpsc::error::TrySendError<Call>> for FoundationErr {
    fn from(err: tokio::sync::mpsc::error::TrySendError<Call>) -> Self {
        Self::FoundationRunnerMpscTrySendErr(Rc::new(err))
    }
}


impl From<tokio::sync::oneshot::error::RecvError> for FoundationErr {
    fn from(err: tokio::sync::oneshot::error::RecvError) -> Self {
        Self::FoundationRunnerOneshotRecvErr(Rc::new(err))
    }
}


impl From<tokio::sync::oneshot::error::TryRecvError> for FoundationErr {
    fn from(err: tokio::sync::oneshot::error::TryRecvError) -> Self {
        Self::FoundationRunnerOneshotTryRecvErr(Rc::new(err))
    }
}

impl FoundationErr {


    pub fn is_fatal(&self) -> bool {
        match self {
            FoundationErr::Panic { .. } => true,
            FoundationErr::FoundationNotFound(_) => true ,
            FoundationErr::FoundationNotAvailable(_) => true,
            _ => false
        }
    }
}

impl FoundationErr {

    pub fn panic<ID,KIND,MSG>(id: ID, kind: KIND, msg: MSG) -> Self where ID: AsRef<str>, KIND: AsRef<str> , MSG: AsRef<str> {
        let id = id.as_ref().to_string();
        let kind = kind.as_ref().to_string();
        let msg = msg.as_ref().to_string();
        FoundationErr::Panic { id, kind, msg }
    }

    pub fn foundation_not_found<K>(kind: K) -> Self where K: AsRef<str>{
        FoundationErr::FoundationNotFound(kind.as_ref().to_string())
    }

    pub fn foundation_not_available(kind: FoundationKind) -> Self {
        FoundationErr::FoundationNotAvailable(kind.to_string())
    }

    pub fn foundation_err<MSG>(kind: FoundationKind, msg: MSG) -> Self where MSG: AsRef<str>{
        let msg = msg.as_ref().to_string();
        FoundationErr::FoundationErr{ kind, msg }
    }

    pub fn dep_not_found<KIND>(kind:KIND) -> Self where KIND: AsRef<str>{
        FoundationErr::DepNotAvailable(kind.as_ref().to_string())
    }

    pub fn dep_not_available(kind: DependencyKind) -> Self {
        FoundationErr::DepNotAvailable(kind.to_string())
    }

    pub fn provider_not_found<KIND>(kind:KIND) -> Self where KIND: AsRef<str>{
        FoundationErr::ProviderNotFound(kind.as_ref().to_string())
    }

    pub fn provider_not_available(kind: ProviderKind) -> Self {
        FoundationErr::ProviderNotAvailable(kind.to_string())
    }




    pub fn dep_err(kind: DependencyKind, msg: String ) -> Self {
        Self::DepErr{kind,msg}
    }

    pub fn prov_err(key: ProviderKey, msg: String ) -> Self {
        Self::ProviderErr{key,msg}
    }

    pub fn foundation_conf_err(kind: FoundationKind, err: serde_yaml::Error, config: Value) -> Self {
        let err =Rc::new(err);
        let config =Rc::new(config);
        Self::FoundationConfigErr {kind,err,config}
    }


    pub fn dep_conf_err(kind: DependencyKind, err: serde_yaml::Error, config: Value) -> Self {
        let err =Rc::new(err);
        let config = config.to_string();
        Self::DepConfErr {kind,err,config}
    }

    pub fn prov_conf_err( kind: ProviderKind, err: serde_yaml::Error, config: Value) -> Self {
        let err =Rc::new(err);
        let config = config.to_string();
        Self::ProvConfErr {kind,err,config}
    }
}


pub trait RegistryProvider: Provider{
    fn registry(& mut self) -> Result<LiveService<Registry>,FoundationErr>;
}





mod runner {
    use std::collections::HashSet;
    use once_cell::sync::Lazy;
    use serde_yaml::Value;
    use starlane_primitive_macros::logger;
    use crate::hyperspace::foundation::{Dependency, DependencyKind, Foundation, FoundationErr, FoundationKind, Kind, Provider, ProviderKey, ProviderKind, RegistryProvider, FOUNDATIONS};
    use crate::hyperspace::foundation::config::{ProtoDependencyConfig, ProtoFoundationConfig};
    use crate::hyperspace::shutdown::add_shutdown_hook;
    use crate::space::point::Point;


   pub(super) enum Call {
        Kind(tokio::sync::oneshot::Sender<FoundationKind>),
        Dependency{ kind: DependencyKind, rtn: tokio::sync::oneshot::Sender<Result<dyn Dependency,FoundationErr>>},
        InstallFoundationRequiredDependencies(tokio::sync::oneshot::Sender<Result<(),FoundationErr>>),
        AddDependency{ config: ProtoDependencyConfig, rtn: tokio::sync::oneshot::Sender<Result<dyn Dependency,FoundationErr>>},
        DepCall(DepCallWrapper)
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


   pub(super) struct DepCallWrapper {
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

    pub(super) enum DepCall {
        Kind(tokio::sync::oneshot::Sender<DependencyKind>),
        Install(tokio::sync::oneshot::Sender<Result<(),FoundationErr>>),
        Provision{ kind: ProviderKind, rtn: tokio::sync::oneshot::Sender<Result<dyn Provider,FoundationErr>>},
        ProviderKinds(tokio::sync::oneshot::Sender<HashSet<&'static str>>)
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

        async fn provision(&self, kind: &ProviderKind, config: Value ) -> Result<impl Provider,FoundationErr> {
            let kind = kind.clone();
            let (rtn, mut rtn_rx) = tokio::sync::oneshot::channel();
            let call = DepCall::Provision{ kind, rtn };
            self.call_tx.send(call).await.unwrap();
            rtn_rx.await?
        }

        /// implementers of this Trait should provide a vec of valid provider kinds
        fn provider_kinds(&self) -> HashSet<&'static str> {
            let (rtn_tx, mut rtn_rx) = tokio::sync::oneshot::channel();
            let call = DepCall::ProviderKinds(rtn_tx);
            self.call_tx.try_send(call).unwrap_or_default();
            rtn_rx.try_recv().unwrap()
        }
    }




    pub(super) struct Runner {
        call_rx: tokio::sync::mpsc::Receiver<Call>,
        foundation: dyn Foundation
    }

    impl Runner {

        pub(super) fn new( foundation: impl Foundation ) -> impl Foundation {
            let (call_tx, call_rx) = tokio::sync::mpsc::channel(64);
            let proxy = FoundationProxy::new(call_tx);
            let runner = Self {
                foundation,
                call_rx
            };
            runner.start();
            proxy
        }

        pub(super) fn start(self) {
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
                }
            }
            Ok(())
        }

    }

}
