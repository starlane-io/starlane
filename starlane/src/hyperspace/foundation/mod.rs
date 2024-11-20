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
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use ascii::AsciiChar::K;
use thiserror::Error;


type CreateFoundation =  dyn FnMut(Value) -> Result<dyn Foundation,FoundationErr> + Sync + Send+ 'static;
type CreateDep =  dyn FnMut(Value) -> Result<dyn Dependency,FoundationErr> + Sync + Send+ 'static;
type CreateProvider =  dyn FnMut(Value) -> Result<dyn Provider,FoundationErr> + Sync + Send+ 'static;

static FOUNDATIONS: Lazy<HashMap<FoundationKind, CreateFoundation>> =
    Lazy::new(|| {
        let mut deps = HashMap::new();
        deps.insert(FoundationKind::DockerDesktop, DockerDesktopFoundation::create );
        deps
    });

pub fn parse_foundation_config<C>(kind: FoundationKind, config: Value) -> Result<FoundationConfig<C>,FoundationErr> where C: Deserialize{
    let config = serde_yaml::from_value(config.clone()).map_err(|err| FoundationErr::foundation_conf_err(kind,err,config))?;
    Ok(config)
}

pub fn parse_dep_config<C>(kind: DependencyKind, config: Value) -> Result<DependencyConfig<C>,FoundationErr> where C: Deserialize{
    let config = serde_yaml::from_value(config.clone()).map_err(|err| FoundationErr::dep_conf_err(kind,err,config))?;
    Ok(config)
}

pub fn parse_provider_config<C>(kind: ProviderKind, config: Value) -> Result<ProviderConfig<C>,FoundationErr> where C: Deserialize{
    let config = serde_yaml::from_value(config.clone()).map_err(|err| FoundationErr::prov_conf_err(kind,err,config))?;
    Ok(config)
}

trait Kind where Self: ToString {
  fn name() -> &'static str;

  fn create(kind: &str) -> Result<impl Self,FoundationErr>;
}


#[async_trait]
pub trait Foundation: Send + Sync + Sized
where
    Self: Sized {

    fn kind() -> FoundationKind;

    fn create( config: Value ) -> Result<impl Foundation,FoundationErr>;

    fn dependency(&self, kind: &DependencyKind ) -> Result<impl Dependency,FoundationErr>;

    /// install any 3rd party dependencies this foundation requires to be minimally operable
    async fn install_foundation_required_dependencies(& mut self) -> Result<(), FoundationErr>;

    /// install a named dependency.  For example the dependency might be "Postgres." The implementing Foundation must
    /// be capable of installing that dependency.  The foundation will make the dependency available after installation
    /// although the method of installing the dependency is under the complete control of the Foundation.  For example:
    /// a LocalDevelopmentFoundation might have an embedded Postgres Database and perhaps another foundation: DockerDesktopFoundation
    /// may actually launch a Postgres Docker image and maybe a KubernetesFoundation may actually install a Postgres Operator ...
    async fn install_dependency(&mut self, key: &DependencyKind, args: Vec<String> ) -> Result<impl Dependency, FoundationErr>;

    /// return the RegistryFoundation
    fn registry(&self) -> &mut impl RegistryProvider;
}




#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct FoundationConfig<C> where C: Deserialize+Serialize{
    pub foundation: FoundationKind,
    pub dependencies: HashMap<DependencyKind,>,
    pub config: C
}

#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct DependencyConfig<C> where C: Deserialize+Serialize{
    pub dependency: DependencyKind,
    pub config: C
}

#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ProviderConfig<C> where C: Deserialize+Serialize{
    pub provider: ProviderKind,
    pub config: C
}

#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct FoundationConfigProto{
    pub foundation: Value,
    pub config: Value
}

impl FoundationConfigProto {
    pub fn create(self) -> Result<impl Foundation,FoundationErr> {
        let kind = serde_yaml::from_str(self.foundation.to_string().as_str()).map_err(|_|FoundationErr::foundation_not_found(self.foundation.to_string()))?;
        let foundation: impl Foundation = FOUNDATIONS.get(&kind).ok_or(FoundationErr::foundation_not_available(kind.clone()))?(self.config)?;
        Ok(foundation)
    }
}





use serde::de::{MapAccess, Visitor};


#[derive(Builder, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
struct DependencyConfigProto{
    pub dependency: Value,
    pub config: Value
}



impl DependencyConfigProto {


}







pub struct RegistryConfig2 {

}


#[derive(Clone,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString)]
pub enum FoundationKind {
    DockerDesktop
}

impl Kind for FoundationKind {
    fn name() -> &'static str {
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

    fn key() -> DependencyKind;


    fn create( config: Value ) -> Result<impl Dependency,FoundationErr>;

    async fn install(&self) -> Result<(), FoundationErr> {
        Ok(())
    }

    async fn provision(&self, kind: &ProviderKind, _config: Value ) -> Result<impl Provider,FoundationErr> {
        Err(FoundationErr::provider_not_available( kind.clone() ))
    }

    fn has_provisioner(kind: &ProviderKind) -> Result<(),FoundationErr> {
        let providers = Self::provider_kinds();
        match kind {
            ProviderKind::Singular if providers.is_empty() => {
                let key = ProviderKey::new(Self::key(), kind.clone());
                Err(FoundationErr::prov_err(key, format!("no providers available for dependency: {}", Self::key().to_string()).to_string()))
            }
            ProviderKind::Singular => {
                Ok(())
            }
            kind => {
                let ext = kind.to_string();
                if providers.contains(ext.as_str()) {
                    Ok(())
                } else {
                    let key = ProviderKey::new(Self::key(), kind.clone());
                    Err(FoundationErr::prov_err(key, format!("provider kind '{}' is not available for dependency: '{}'", ext.to_string(), Self::key().to_string()).to_string()))
                }
            }
        }
    }

    /// implementers of this Trait should provide a vec of valid provider kinds
    fn provider_kinds() -> HashSet<&'static str> {
        HashSet::new()
    }


}

pub trait Provider {
    async fn initialize(&mut self) -> Result<(), FoundationErr>;
}



pub type RawConfig = Value;





#[derive(Clone,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString)]
pub enum DependencyKind {
    Postgres,
}


impl Kind for DependencyKind{
    fn name() -> &'static str {
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
    fn name() -> &'static str {
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


#[derive(Clone)]
pub enum Status<D> {
   Pending,
    /// step is an arbitrary thing whereas 'progress' should alwasy be bound between 0..100
   Progress{ step: String, progress: usize},
   Ready(Arc<D>),
   Err(FoundationErr),
}


#[derive(Clone)]
pub enum DependencyPhase {
    Booting,
    Installing,
    Provisioning
}
