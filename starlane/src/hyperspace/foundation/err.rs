use std::fmt::{Display, Formatter};
use serde_yaml::Value;
use std::rc::Rc;
use ascii::AsciiChar::k;
use serde::{de, Deserialize, Serialize};
use thiserror::Error;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind,  ProviderKind};
use crate::space::err::{ParseErrs, ToSpaceErr};

pub struct Call;


pub struct FoundationErrBuilder<'z> {
    pub kind: Option<(&'static str, &'static str)>,
    pub config: Option<&'z (dyn Display+'z)>,
    pub settings: Option<&'z (dyn Display+'z)>,
}

impl <'z> Default for FoundationErrBuilder<'z> {
    fn default() -> Self {
        Self {
            kind: None,
            config: None,
            settings: None,
        }
    }
}

impl <'z> FoundationErrBuilder<'z> {
    pub fn kind(&mut self, kind: impl IKind) -> & mut Self{
        self.kind = Some((kind.category(),kind.as_str()));
        self
    }

    pub fn config(&mut self, config: &'z (dyn Display+'z)) -> & mut Self{
        self.config = Some(config);
        self
    }

    pub fn settings(&mut self, settings: &'z (dyn Display+'z)) -> & mut Self{
        self.settings = Some(settings);
        self
    }


    pub fn err( mut self, err: impl Display+'z ) -> FoundationErr  {
        let mut rtn = String::new();

        if let Some((cat,kind)) = self.kind {
            let fmt = format!("Foundation ERR {}::{} --> ",cat,kind);
            rtn.push_str(fmt.as_str());
        }
        let (action,verbose) = if let Some(config) = self.config {
            ("config".to_string(), config.to_string())
        } else if let Some(settings) = self.settings {
            ("settings".to_string(), settings.to_string())
        } else {
            ("<yaml>".to_string(), "<?>".to_string() )
        };

        let fmt = format!("{}: \n```{}```\n", action, verbose);
        rtn.push_str(fmt.as_str());



        let err= format!("serde err: '{}'",err );
        rtn.push_str(err.as_str());
        FoundationErr::Msg(rtn)
    }

}

impl FoundationErr {

    pub fn kind<'z>(kind: &impl IKind) -> FoundationErrBuilder<'z>{
        FoundationErrBuilder {
            kind: Some((kind.category(),kind.as_str())),
            ..Default::default()
        }
    }

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
        FoundationErr::FoundationError { kind, msg }
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

    pub fn prov_err(kind: ProviderKind, msg: String ) -> Self {
        Self::ProviderErr{ kind: kind,msg}
    }

    pub fn foundation_verbose_error<C>(kind: FoundationKind, err: serde_yaml::Error, config: C ) -> Self where C: ToString {
        let err = err.to_string();
        let config =config.to_string();
        Self::FoundationVerboseErr {kind,err,config}
    }

    pub fn settings_err<E>(err: E ) -> Self  where E: ToString {
        Self::FoundationSettingsErr(format!("{}",err.to_string()).to_string())
    }

    pub fn msg(err: impl Display ) -> Self {
        Self::Msg(format!("{}",err).to_string())
    }


    pub fn config_err(err: impl Display ) -> Self {
        Self::FoundationConfErr(format!("{}",err).to_string())
    }


    pub fn kind_not_found(category: impl Display, variant: impl Display) -> Self {
        let variant = variant.to_string();
        let category = category.to_string();
        Self::KindNotFound {category,variant}
    }

    pub fn missing_kind_declaration(category: impl ToString) -> Self {
        let category = category.to_string();
        Self::MissingKind(category)
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


#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ActionItem {
    pub title: String,
    pub website: Option<String>,
    pub details: String
}



impl ActionItem {
    pub fn new(title: String, details: String) -> Self {
        Self {
            title,
            details,
            website: None,
        }
    }

    pub fn with_website( & mut self, website: String ) {
        self.website = Some(website);
    }

    pub fn print( vec: &Vec<Self> ) {

    }
}

impl Display for ActionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.title)
    }
}

#[derive(Error,Clone,Debug,Serialize,Deserialize)]
pub enum FoundationErr {
    #[error("{kind} {actions.len()} User Actions Required")]
    ActionRequired { kind: FoundationKind, actions: Vec<ActionItem> },
    #[error("[{id}] -> PANIC! <{kind}> error message: '{msg}'")]
    Panic {id: String, kind: String, msg: String},
    #[error("FoundationConfig.config is set to '{0}' which this Starlane build does not recognize")]
    FoundationNotFound(String),
    #[error("config: '{0}' is recognized but is not available on this build of Starlane")]
    FoundationNotAvailable(String),
    #[error("DependencyConfig.config is set to '{0}' which this Starlane build does not recognize")]
    DepNotFound(String),
    #[error("implementation: '{0}' is recognized but is not available on this build of Starlane")]
    DepNotAvailable(String),
    #[error("ProviderConfig.provider is set to '{0}' which this Starlane build does not recognize")]
    ProviderNotFound(String),
    #[error("provider: '{0}' is recognized but is not available on this build of Starlane")]
    ProviderNotAvailable(String),
    #[error("error converting config config for '{kind}' serialization err: '{err}' config: {config}")]
    FoundationVerboseErr { kind: FoundationKind,err: String, config: String },
    #[error("config settings err: {0}")]
    FoundationSettingsErr(String),
    #[error("config config err: {0}")]
    FoundationConfErr(String),
    #[error("[{kind}] Foundation Error: '{msg}'")]
    FoundationError { kind: FoundationKind, msg: String },
    #[error("[{kind}] Error: '{msg}'")]
    DepErr{ kind: DependencyKind, msg: String},
    #[error("Action Required: {cat}: {kind} cannot {action} without user help.  Additional Info: '{summary}'")]
    UserActionRequired{ cat: String, kind: String, action: String, summary: String, },
    #[error("[{kind}] Error: '{msg}'")]
    ProviderErr{ kind: ProviderKind, msg: String},
    #[error("error converting config args for implementation: '{kind}' serialization err: '{err}' from config: '{config}'")]
    DepConfErr { kind: DependencyKind,err: String, config: String},
    #[error("error converting config args for provider: '{kind}' serialization err: '{err}' from config: '{config}'")]
    ProvConfErr { kind: ProviderKind, err: Rc<serde_yaml::Error>, config: String},
    #[error("illegal attempt to change config after it has already been initialized.  Foundation can only be initialized once")]
    FoundationAlreadyCreated,
    #[error("Foundation Runner call sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerMpscSendErr(Rc<tokio::sync::mpsc::error::SendError<Call>>),
    #[error("Foundation Runner return sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerOneshotRecvErr(Rc<tokio::sync::oneshot::error::RecvError>),
    #[error("Foundation Runner call sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerMpscTrySendErr(Rc<tokio::sync::mpsc::error::TrySendError<Call>>),
    #[error("Foundation Runner return sender err (this could be fatal) caused by: {0}")]
    FoundationRunnerOneshotTryRecvErr(Rc<tokio::sync::oneshot::error::TryRecvError>),
    #[error("{0}")]
    Msg(String),
    #[error("{category} does not have a kind variant `{variant}`")]
    KindNotFound{category:String,variant:String},
    #[error("Missing `kind:` mapping for {0}")]
    MissingKind(String),
    #[error("{0}")]
    ParseErrs(#[from] ParseErrs),
    #[error("")]
    NotInstalledErr{ dependency: String }
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