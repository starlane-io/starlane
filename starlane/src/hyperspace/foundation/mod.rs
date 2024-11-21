//pub mod docker;
pub mod config;
pub struct Call;

#[derive(Clone, Serialize, Deserialize)]
pub struct StarlaneConfig {
    pub context: String,
    pub home: String,
    pub can_nuke: bool,
    pub can_scorch: bool,
    pub control_port: u16,
}
/*
pub mod traits;
pub mod factory;
pub mod runner;

 */
use std::fmt::Display;
use crate::hyperspace::platform::PlatformConfig;
use crate::hyperspace::reg::Registry;
use derive_builder::Builder;
use futures::TryFutureExt;
use itertools::Itertools;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use std::future::Future;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::str::FromStr;
use thiserror::Error;


trait Kind where Self: Clone+Eq+PartialEq+Display+Hash {
  fn identifier() -> &'static str;

//  fn create(kind: &str) -> Result<impl Self,FoundationErr>;
}


use serde::de::{MapAccess, Visitor};
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


#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString)]
pub enum FoundationKind {
    DockerDesktop
}

impl Kind for FoundationKind {
    fn identifier() -> &'static str {
        "foundation"
    }
}


pub type RawConfig = Value;





#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString, Serialize, Deserialize)]
pub enum DependencyKind {
    Postgres,
    Docker
}


impl Kind for DependencyKind{
    fn identifier() -> &'static str {
        "dependency"
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Hash)]
pub struct ProviderKey{
    dep: DependencyKind,
    kind: ProviderKind
}

impl Display for ProviderKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{}:{}", self.dep, self.kind))
    }
}


impl ProviderKey {
    pub fn new(dep: DependencyKind, kind: ProviderKind) -> Self {
        Self {
            dep,
            kind,
        }
    }
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,strum_macros::Display,strum_macros::EnumString)]
pub enum ProviderKind {
    /// this means that the Dependency has one and only one Provider
    Database,
    DockerDaemon
}

impl Kind for ProviderKind{
    fn identifier() -> &'static str {
        "provider"
    }


}


#[derive(Clone)]
pub struct LiveService<S> where S: Clone{
    pub service: S,
    tx: tokio::sync::mpsc::Sender<()>
}

impl <S> Deref for LiveService<S> where S: Clone{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

impl <S> DerefMut for LiveService<S> where S: Clone{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.service
    }
}


#[derive(Error,Clone,Debug)]
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
    FoundationConfigErr { kind: FoundationKind,err: String, config: String },
    #[error("[{kind}] Foundation Error: '{msg}'")]
    FoundationErr{ kind: FoundationKind, msg: String },
    #[error("[{kind}] Error: '{msg}'")]
    DepErr{ kind: DependencyKind, msg: String},
    #[error("Action Required: {cat}: {kind} cannot {action} without user help.  Additional Info: '{summary}'")]
    UserActionRequired{ cat: String, kind: String, action: String, summary: String, },
    #[error("[{key}] Error: '{msg}'")]
    ProviderErr{ key: ProviderKey, msg: String},
    #[error("error converting foundation args for dependency: '{kind}' serialization err: '{err}' from config: '{config}'")]
    DepConfErr { kind: DependencyKind,err: String, config: String},
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

    pub fn user_action_required<CAT,KIND,ACTION,SUMMARY>(cat: CAT, kind: KIND, action: ACTION, summary: SUMMARY ) -> Self where CAT: AsRef<str>, KIND: AsRef<str> , ACTION: AsRef<str>, SUMMARY: AsRef<str> {
        let cat = cat.as_ref().to_string();
        let kind = kind.as_ref().to_string();
        let action = action.as_ref().to_string();
        let summary= summary.as_ref().to_string();
        FoundationErr::UserActionRequired {cat, kind, action, summary}
    }


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
        let err = err.to_string();
        let config =config.as_str().unwrap_or("?").to_string();
        Self::FoundationConfigErr {kind,err,config}
    }


    pub fn dep_conf_err(kind: DependencyKind, err: serde_yaml::Error, config: Value) -> Self {
        let err = err.to_string();
        let config =config.as_str().unwrap_or("?").to_string();
        Self::DepConfErr {kind,err,config}
    }

    pub fn prov_conf_err( kind: ProviderKind, err: serde_yaml::Error, config: Value) -> Self {
        let err =Rc::new(err);

        let config =config.as_str().unwrap_or("?").to_string();
        Self::ProvConfErr {kind,err,config}
    }
}








