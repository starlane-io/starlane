use std::convert::Infallible;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::string::FromUtf8Error;

use crate::parse::error::find_parse_err;
use crate::substance::substance::{Errors, Substance};
use crate::wave::ReflectedCore;
use ariadne::{Label, Report, ReportBuilder, ReportKind, Source};
use cosmic_nom::Span;
use cosmic_nom::SpanExtra;
use http::header::ToStrError;
use http::status::InvalidStatusCode;
use http::uri::InvalidUri;
use http::StatusCode;
use nom::error::VerboseError;
use nom::Err;
use nom_locate::LocatedSpan;
use nom_supreme::error::{ErrorTree, StackContext};
use serde::de::Error;
use std::num::ParseIntError;
use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, PoisonError};
use tokio::sync::mpsc::error::{SendError, SendTimeoutError};
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;

pub enum MsgErr {
    Status { status: u16, message: String },
    ParseErrs(ParseErrs),
}

impl Into<ReflectedCore> for MsgErr {
    fn into(self) -> ReflectedCore {
        match self {
            MsgErr::Status { status, message } => ReflectedCore {
                headers: Default::default(),
                status: StatusCode::from_u16(status).unwrap_or(StatusCode::from_u16(500).unwrap()),
                body: Substance::Errors(Errors::default(message.as_str())),
            },
            MsgErr::ParseErrs(_) => ReflectedCore {
                headers: Default::default(),
                status: StatusCode::from_u16(500u16).unwrap_or(StatusCode::from_u16(500).unwrap()),
                body: Substance::Errors(Errors::default("parsing error...")),
            },
        }
    }
}

/*
impl PlatErr for MsgErr {

    fn to_cosmic_err(&self) -> MsgErr {
        MsgErr::Status { status: self.status(), message: self.to_string() }
    }

    fn new<S>(message: S) -> Self where S: ToString {
        MsgErr::Status { status: 500u16, message: message.to_string() }
    }

    fn status_msg<S>(status: u16, message: S) -> Self where S: ToString {
        MsgErr::Status { status, message: message.to_string() }
    }

    fn status(&self) -> u16 {
        match self {
            MsgErr::Status { status, message } => status.clone(),
            MsgErr::ParseErrs(_) => 500u16,
        }
    }
}

 */

impl Clone for MsgErr {
    fn clone(&self) -> Self {
        MsgErr::Status { status: 500, message: self.message() }
    }
}



impl MsgErr {


    pub fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::Text(self.message().to_string())
        }
    }
    pub fn from_status(status: u16) -> MsgErr {
        let message = match status {
            400 => "Bad Request".to_string(),
            404 => "Not Found".to_string(),
            403 => "Forbidden".to_string(),
            408 => "Timeout".to_string(),
            500 => "Internal Server Error".to_string(),
            status => format!("{} Error", status),
        };
        MsgErr::Status { status, message }
    }
}

impl Into<ParseErrs> for MsgErr {
    fn into(self) -> ParseErrs {
        match self {
            MsgErr::Status { status, message } => {
                let mut builder = Report::build(ReportKind::Error, (), 0);
                let report = builder.with_message(message).finish();
                let errs = ParseErrs {
                    report: vec![report],
                    source: None,
                };
                errs
            }
            MsgErr::ParseErrs(errs) => errs,
        }
    }
}

impl MsgErr {
    pub fn timeout() -> Self {
        MsgErr::from_status(408)
    }

    pub fn server_error() -> Self {
        MsgErr::from_status(500)
    }
    pub fn forbidden() -> Self {
        MsgErr::err403()
    }

    pub fn forbidden_msg<S:ToString>(msg: S) -> Self {
        MsgErr::Status {
            status: 403,
            message: msg.to_string()
        }
    }


    pub fn not_found() -> Self {
        MsgErr::err404()
    }

    pub fn bad_request() -> Self {
        MsgErr::from_status(400)
    }
}

impl Debug for MsgErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgErr::Status { status, message } => {
                f.write_str(format!("{}: {}", status, message).as_str())
            }
            MsgErr::ParseErrs(_) => f.write_str("Error Report..."),
        }
    }
}

impl MsgErr {
    pub fn print(&self) {
        match self {
            MsgErr::Status { .. } => {
                println!("{}", self.to_string());
            }
            MsgErr::ParseErrs(err) => err.print(),
        }
    }
}

impl MsgErr {
    pub fn new(status: u16, message: &str) -> Self {
        Self::Status {
            status,
            message: message.to_string(),
        }
    }

    pub fn err404() -> Self {
        Self::Status {
            status: 404,
            message: "Not Found".to_string(),
        }
    }

    pub fn err403() -> Self {
        Self::Status {
            status: 403,
            message: "Forbidden".to_string(),
        }
    }

    pub fn err500() -> Self {
        Self::Status {
            status: 500,
            message: "Internal Server Error".to_string(),
        }
    }

    pub fn err400() -> Self {
        Self::Status {
            status: 400,
            message: "Bad Request".to_string(),
        }
    }

    pub fn from_500<S: ToString>(message: S) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl StatusErr for MsgErr {
    fn status(&self) -> u16 {
        match self {
            MsgErr::Status { status, .. } => {
                status.clone()
            }
            MsgErr::ParseErrs(_) => {
                500u16
            }
        }
    }

    fn message(&self) -> String {
        match self {
            MsgErr::Status { status, message } => message.clone(),
            MsgErr::ParseErrs(_) => "Error report".to_string(),
        }
    }
}

pub trait StatusErr {
    fn status(&self) -> u16;
    fn message(&self) -> String;
}


impl Display for MsgErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MsgErr::Status { status, message } => {
                f.write_str(format!("{}: {}", status, message).as_str())
            }
            MsgErr::ParseErrs(_) => f.write_str("Error Report..."),
        }
    }
}

impl std::error::Error for MsgErr {}


impl <C> From<SendTimeoutError<C>> for MsgErr {
    fn from(e: SendTimeoutError<C>) -> Self {
        MsgErr::Status {
            status: 500,
            message: e.to_string()
        }
    }
}

impl <C> From<tokio::sync::mpsc::error::SendError<C>> for MsgErr {
    fn from(e: SendError<C>) -> Self {
        MsgErr::from_500(e.to_string())
    }
}

impl From<String> for MsgErr {
    fn from(message: String) -> Self {
        Self::Status {
            status: 500,
            message,
        }
    }
}

impl From<Elapsed> for MsgErr {
    fn from(e: Elapsed) -> Self {
        Self::Status {
            status: 408,
            message: e.to_string(),
        }
    }
}

impl<T> From<PoisonError<T>> for MsgErr {
    fn from(e: PoisonError<T>) -> Self {
        MsgErr::Status {
            status: 500,
            message: e.to_string(),
        }
    }
}

impl From<InvalidStatusCode> for MsgErr {
    fn from(error: InvalidStatusCode) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<FromUtf8Error> for MsgErr {
    fn from(message: FromUtf8Error) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<&str> for MsgErr {
    fn from(message: &str) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for MsgErr {
    fn from(message: Box<bincode::ErrorKind>) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<Infallible> for MsgErr {
    fn from(i: Infallible) -> Self {
        Self::Status {
            status: 500,
            message: i.to_string(),
        }
    }
}

impl From<nom::Err<VerboseError<&str>>> for MsgErr {
    fn from(error: nom::Err<VerboseError<&str>>) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<semver::Error> for MsgErr {
    fn from(error: semver::Error) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<ErrorTree<&str>> for MsgErr {
    fn from(error: ErrorTree<&str>) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<strum::ParseError> for MsgErr {
    fn from(error: strum::ParseError) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<()> for MsgErr {
    fn from(err: ()) -> Self {
        Self::Status{
            status: 500,
            message: "Empty Error".to_string()
        }
    }
}


impl From<tokio::sync::oneshot::error::RecvError> for MsgErr {
    fn from(err: RecvError) -> Self {
         Self::Status{
             status: 500,
             message: err.to_string()
         }
    }
}

impl From<ParseIntError> for MsgErr {
    fn from(x: ParseIntError) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

impl From<regex::Error> for MsgErr {
    fn from(x: regex::Error) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

impl From<InvalidUri> for MsgErr {
    fn from(x: InvalidUri) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

impl From<http::Error> for MsgErr {
    fn from(x: http::Error) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

impl From<ToStrError> for MsgErr {
    fn from(x: ToStrError) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

impl<I: Span> From<nom::Err<ErrorTree<I>>> for MsgErr {
    fn from(err: Err<ErrorTree<I>>) -> Self {
        fn handle<I: Span>(err: ErrorTree<I>) -> MsgErr {
            match err {
                ErrorTree::Base {
                    location,
                    kind: _kind,
                } => MsgErr::Status {
                    status: 500,
                    message: format!(
                        "parse error line: {} column: {}",
                        location.location_line(),
                        location.get_column()
                    ),
                },
                ErrorTree::Stack { base, contexts } => match contexts.first() {
                    None => MsgErr::Status {
                        status: 500,
                        message: "error, cannot find location".to_string(),
                    },
                    Some((location, _)) => MsgErr::Status {
                        status: 500,
                        message: format!(
                            "Stack parse error line: {} column: {}",
                            location.location_line(),
                            location.get_column()
                        ),
                    },
                },
                ErrorTree::Alt(what) => MsgErr::Status {
                    status: 500,
                    message: "alt error".to_string(),
                },
            }
        }
        match err {
            Err::Incomplete(_) => MsgErr::Status {
                status: 500,
                message: "unexpected incomplete parsing error".to_string(),
            },

            Err::Error(err) => handle(err),
            Err::Failure(err) => handle(err),
        }
    }
}

impl Into<String> for MsgErr {
    fn into(self) -> String {
        self.to_string()
    }
}

impl From<io::Error> for MsgErr {
    fn from(e: io::Error) -> Self {
        MsgErr::new(500, e.to_string().as_str())
    }
}

impl From<ParseErrs> for MsgErr {
    fn from(errs: ParseErrs) -> Self {
        MsgErr::ParseErrs(errs)
    }
}
impl<I: Span> From<nom::Err<ErrorTree<I>>> for ParseErrs {
    fn from(err: Err<ErrorTree<I>>) -> Self {
        match find_parse_err(&err) {
            MsgErr::Status { .. } => ParseErrs {
                report: vec![],
                source: None,
            },
            MsgErr::ParseErrs(parse_errs) => parse_errs,
        }
    }
}

pub struct SubstErr {}

impl SubstErr {
    pub fn report(&self) -> Result<Report, MsgErr> {
        unimplemented!()
    }
}

pub struct ParseErrs {
    pub report: Vec<Report>,
    pub source: Option<Arc<String>>,
}

impl ParseErrs {
    pub fn from_report(report: Report, source: Arc<String>) -> Self {
        Self {
            report: vec![report],
            source: Some(source),
        }
    }

    pub fn from_loc_span<I: Span>(message: &str, label: &str, span: I) -> MsgErr {
        let mut builder = Report::build(ReportKind::Error, (), 23);
        let report = builder
            .with_message(message)
            .with_label(
                Label::new(span.location_offset()..(span.location_offset() + span.len()))
                    .with_message(label),
            )
            .finish();
        return ParseErrs::from_report(report, span.extra()).into();
    }

    pub fn from_range(message: &str, label: &str, range: Range<usize>, extra: SpanExtra) -> MsgErr {
        let mut builder = Report::build(ReportKind::Error, (), 23);
        let report = builder
            .with_message(message)
            .with_label(Label::new(range).with_message(label))
            .finish();
        return ParseErrs::from_report(report, extra).into();
    }

    pub fn from_owned_span<I: Span>(message: &str, label: &str, span: I) -> MsgErr {
        let mut builder = Report::build(ReportKind::Error, (), 23);
        let report = builder
            .with_message(message)
            .with_label(
                Label::new(span.location_offset()..(span.location_offset() + span.len()))
                    .with_message(label),
            )
            .finish();
        return ParseErrs::from_report(report, span.extra()).into();
    }

    pub fn print(&self) {
        if let Some(source) = self.source.as_ref() {
            for report in &self.report {
                report
                    .print(Source::from(source.as_str()))
                    .unwrap_or_default()
            }
        }
    }

    pub fn fold<E: Into<ParseErrs>>(errs: Vec<E>) -> ParseErrs {
        let errs: Vec<ParseErrs> = errs.into_iter().map(|e| e.into()).collect();

        let source = if let Some(first) = errs.first() {
            if let Some(source) = first.source.as_ref().cloned() {
                Some(source)
            } else {
                None
            }
        } else {
            None
        };

        let mut rtn = ParseErrs {
            report: vec![],
            source,
        };

        for err in errs {
            for report in err.report {
                rtn.report.push(report)
            }
        }
        rtn
    }
}
impl From<serde_urlencoded::de::Error> for MsgErr {
    fn from(err: serde_urlencoded::de::Error) -> Self {
        MsgErr::Status {
            status: 500u16,
            message: err.to_string(),
        }
    }
}

impl From<serde_urlencoded::ser::Error> for MsgErr {
    fn from(err: serde_urlencoded::ser::Error) -> Self {
        MsgErr::Status {
            status: 500u16,
            message: err.to_string(),
        }
    }
}

pub mod report {
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize)]
    pub struct Report {
        kind: ReportKind,
        code: Option<String>,
        msg: Option<String>,
        note: Option<String>,
        help: Option<String>,
        location: Range,
        labels: Vec<Label>,
    }

    #[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum ReportKind {
        Error,
        Warning,
        Advice,
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct Range {
        pub start: u32,
        pub end: u32,
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub struct Label {
        span: Range,
        msg: Option<String>,
        color: Option<Color>,
        order: i32,
        priority: i32,
    }

    #[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Copy, Clone)]
    pub enum Color {
        /// No color has been set. Nothing is changed when applied.
        Unset,

        /// Terminal default #9. (foreground code `39`, background code `49`).
        Default,

        /// Black #0 (foreground code `30`, background code `40`).
        Black,

        /// Red: #1 (foreground code `31`, background code `41`).
        Red,

        /// Green: #2 (foreground code `32`, background code `42`).
        Green,

        /// Yellow: #3 (foreground code `33`, background code `43`).
        Yellow,

        /// Blue: #4 (foreground code `34`, background code `44`).
        Blue,

        /// Magenta: #5 (foreground code `35`, background code `45`).
        Magenta,

        /// Cyan: #6 (foreground code `36`, background code `46`).
        Cyan,

        /// White: #7 (foreground code `37`, background code `47`).
        White,

        /// A color number from 0 to 255, for use in 256-color terminals.
        Fixed(u8),

        /// A 24-bit RGB color, as specified by ISO-8613-3.
        RGB(u8, u8, u8),
    }
}
