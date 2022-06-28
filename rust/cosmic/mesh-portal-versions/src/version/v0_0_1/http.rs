use crate::error::MsgErr;
use crate::version::v0_0_1::wave::{Method, ReqCore, RespCore};
use crate::version::v0_0_1::id::id::Meta;
use crate::version::v0_0_1::substance::substance::{Errors, Substance};
use http::{HeaderMap, StatusCode, Uri};
use serde::{Deserialize, Serialize};
use crate::version::v0_0_1::util::ValueMatcher;

#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display, strum_macros::EnumString, Eq, PartialEq,Hash)]
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

impl Into<http::Method> for HttpMethod {
    fn into(self) -> http::Method {
        match self {
            HttpMethod::Options => http::Method::OPTIONS,
            HttpMethod::Get => http::Method::GET,
            HttpMethod::Post => http::Method::POST,
            HttpMethod::Put => http::Method::PUT,
            HttpMethod::Delete => http::Method::DELETE,
            HttpMethod::Head => http::Method::HEAD,
            HttpMethod::Trace => http::Method::TRACE,
            HttpMethod::Connect => http::Method::CONNECT,
            HttpMethod::Patch => http::Method::PATCH,
        }
    }
}

impl TryFrom<http::Method> for HttpMethod {
    type Error = MsgErr;

    fn try_from(method: http::Method) -> Result<Self, Self::Error> {
        match method.as_str() {
            "OPTIONS" => Ok(HttpMethod::Options),
            "GET" => Ok(HttpMethod::Get),
            "POST" => Ok(HttpMethod::Post),
            "PUT" => Ok(HttpMethod::Put),
            "DELETE" => Ok(HttpMethod::Delete),
            "HEAD" => Ok(HttpMethod::Head),
            "TRACE" => Ok(HttpMethod::Trace),
            "CONNECT" => Ok(HttpMethod::Connect),
            "PATCH" => Ok(HttpMethod::Patch),
            _ => Err("http method extensions not supported".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpRequest {
    #[serde(with = "http_serde::method")]
    pub method: http::Method,

    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,

    #[serde(with = "http_serde::uri")]
    pub uri: Uri,
    pub body: Substance,
}

impl HttpRequest {
    pub fn ok(&self, payload: Substance) -> RespCore {
        RespCore {
            headers: Default::default(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body: payload,
        }
    }

    pub fn fail(&self, error: &str) -> RespCore {
        let errors = Errors::default(error);
        RespCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::Errors(errors),
        }
    }
}

impl TryFrom<ReqCore> for HttpRequest {
    type Error = MsgErr;

    fn try_from(core: ReqCore) -> Result<Self, Self::Error> {
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
