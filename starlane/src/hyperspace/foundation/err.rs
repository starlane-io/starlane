use std::fmt::Display;
use serde_yaml::Value;
use std::rc::Rc;
use serde::de;
use thiserror::Error;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, ProviderKey, ProviderKind};
use crate::space::err::ToSpaceErr;

pub struct Call;

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

    pub fn foundation_verbose_error<C>(kind: FoundationKind, err: serde_yaml::Error, config: C ) -> Self where C: ToString {
        let err = err.to_string();
        let config =config.to_string();
        Self::FoundationVerboseErr {kind,err,config}
    }

    pub fn settings_err<E>(err: E ) -> Self  where E: ToString {
        Self::FoundationSettingsErr(format!("{}",err.to_string()).to_string())
    }

    pub fn config_err<E>(err: E ) -> Self  where E: ToString {
        Self::FoundationConfErr(format!("{}",err.to_string()).to_string())
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
    FoundationVerboseErr { kind: FoundationKind,err: String, config: String },
    #[error("foundation settings err: {0}")]
    FoundationSettingsErr(String),
    #[error("foundation config err: {0}")]
    FoundationConfErr(String),
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