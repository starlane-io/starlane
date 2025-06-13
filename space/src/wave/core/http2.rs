use serde::{Deserialize, Serialize};

use crate::err::SpaceErr;
use crate::substance::{FormErrs, Substance};
use crate::wave::core::{DirectedCore, HeaderMap, Method, ReflectedCore};
use url::Url;

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
    Eq,
    PartialEq,
    Hash,
)]
pub enum HttpMethod {
    Options,
    Get,
    Post,
    Put,
    Delete,
    Head,
    Trace,
    Connect,
    Patch,
}

impl Default for HttpMethod {
    fn default() -> Self {
        Self::Get
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    pub method: HttpMethod,
    pub headers: HeaderMap,
    pub uri: Url,
    pub body: Substance,
}

impl HttpRequest {
    pub fn ok(&self, payload: Substance) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body: payload,
        }
    }

    pub fn fail(&self, error: &str) -> ReflectedCore {
        let errors = FormErrs::default(error);
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::FormErrs(errors),
        }
    }
}

impl Into<DirectedCore> for HttpRequest {
    fn into(self) -> DirectedCore {
        DirectedCore {
            headers: self.headers,
            method: self.method.into(),
            uri: self.uri,
            body: self.body,
        }
    }
}

impl TryFrom<DirectedCore> for HttpRequest {
    type Error = SpaceErr;

    fn try_from(core: DirectedCore) -> Result<Self, Self::Error> {
        if let Method::Http(method) = core.method {
            Ok(Self {
                method: method.into(),
                headers: core.headers,
                uri: core.uri,
                body: core.body,
            })
        } else {
            Err("expected Http".into())
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct StatusCode {
    pub code: u16,
}

impl StatusCode {
    pub fn from_u16(code: u16) -> Result<Self, SpaceErr> {
        Ok(Self { code })
    }

    pub fn as_u16(&self) -> u16 {
        self.code
    }

    pub fn is_success(&self) -> bool {
        self.code >= 200 && self.code <= 299
    }

    pub fn fail() -> StatusCode {
        Self::from_u16(500u16).unwrap()
    }
}

impl ToString for StatusCode {
    fn to_string(&self) -> String {
        self.code.to_string()
    }
}

impl Default for StatusCode {
    fn default() -> Self {
        StatusCode::from_u16(200).unwrap()
    }
}
