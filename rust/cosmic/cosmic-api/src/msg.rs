use crate::error::MsgErr;
use crate::id::id::Meta;
use crate::parse::camel_case_chars;
use crate::parse::error::result;
use crate::parse::model::MethodScopeSelector;
use crate::substance::substance::{Errors, Substance};
use crate::util::{ValueMatcher, ValuePattern};
use crate::wave::{DirectedCore, Method, ReflectedCore};
use cosmic_nom::new_span;
use http::{HeaderMap, StatusCode, Uri};
use nom::combinator::all_consuming;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct MsgMethod {
    string: String,
}

impl MsgMethod {
    pub fn new<S: ToString>(string: S) -> Result<Self, MsgErr> {
        let tmp = string.to_string();
        let string = result(all_consuming(camel_case_chars)(new_span(tmp.as_str())))?.to_string();
        Ok(Self { string })
    }
}

impl ToString for MsgMethod {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl ValueMatcher<MsgMethod> for MsgMethod {
    fn is_match(&self, x: &MsgMethod) -> Result<(), ()> {
        if *self == *x {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Into<MethodScopeSelector> for MsgMethod {
    fn into(self) -> MethodScopeSelector {
        MethodScopeSelector::new(
            ValuePattern::Pattern(Method::Msg(self)),
            Regex::new(".*").unwrap(),
        )
    }
}

impl TryFrom<String> for MsgMethod {
    type Error = MsgErr;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for MsgMethod {
    type Error = MsgErr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl Deref for MsgMethod {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

impl Default for MsgMethod {
    fn default() -> Self {
        Self {
            string: "Def".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgRequest {
    pub method: MsgMethod,

    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,

    #[serde(with = "http_serde::uri")]
    pub uri: Uri,
    pub body: Substance,
}

impl Default for MsgRequest {
    fn default() -> Self {
        Self {
            method: Default::default(),
            headers: Default::default(),
            uri: Default::default(),
            body: Default::default(),
        }
    }
}

impl MsgRequest {
    pub fn new<M>(method: M) -> Result<Self, MsgErr>
    where
        M: TryInto<MsgMethod, Error = MsgErr>,
    {
        Ok(MsgRequest {
            method: method.try_into()?,
            ..Default::default()
        })
    }

    pub fn with_body(mut self, body: Substance) -> Self {
        self.body = body;
        self
    }

    pub fn ok(&self, payload: Substance) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body: payload,
        }
    }

    pub fn fail(&self, error: &str) -> ReflectedCore {
        let errors = Errors::default(error);
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::Errors(errors),
        }
    }
}

impl TryFrom<DirectedCore> for MsgRequest {
    type Error = MsgErr;

    fn try_from(core: DirectedCore) -> Result<Self, Self::Error> {
        if let Method::Msg(action) = core.method {
            Ok(Self {
                method: action,
                headers: core.headers,
                uri: core.uri,
                body: core.body,
            })
        } else {
            Err("expected Msg".into())
        }
    }
}
