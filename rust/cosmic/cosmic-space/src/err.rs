use std::convert::Infallible;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::num::ParseIntError;
use std::ops::Range;
use std::rc::Rc;
use std::string::FromUtf8Error;
use std::sync::{Arc, PoisonError};

//use ariadne::{Label, Report, ReportBuilder, ReportKind, Source};
use nom::error::VerboseError;
use nom::Err;
use nom_locate::LocatedSpan;
use nom_supreme::error::{ErrorTree, StackContext};
use serde::de::Error;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError};
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;

use crate::err::report::{Label, Report, ReportKind};
use cosmic_nom::Span;
use cosmic_nom::SpanExtra;

use crate::parse::error::find_parse_err;
use crate::substance::{Errors, Substance};
use crate::wave::core::http2::StatusCode;
use crate::wave::core::ReflectedCore;

pub enum UniErr {
    Status { status: u16, message: String },
    ParseErrs(ParseErrs),
}

impl Into<ReflectedCore> for UniErr {
    fn into(self) -> ReflectedCore {
        match self {
            UniErr::Status { status, message } => ReflectedCore {
                headers: Default::default(),
                status: StatusCode::from_u16(status).unwrap_or(StatusCode::from_u16(500).unwrap()),
                body: Substance::Errors(Errors::default(message.as_str())),
            },
            UniErr::ParseErrs(_) => ReflectedCore {
                headers: Default::default(),
                status: StatusCode::from_u16(500u16).unwrap_or(StatusCode::from_u16(500).unwrap()),
                body: Substance::Errors(Errors::default("parsing error...")),
            },
        }
    }
}

impl Clone for UniErr {
    fn clone(&self) -> Self {
        UniErr::Status {
            status: 500,
            message: self.message(),
        }
    }
}

pub trait CoreReflector {
    fn as_reflected_core(self) -> ReflectedCore;
}

impl CoreReflector for UniErr {
    fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::Text(self.message().to_string()),
        }
    }
}

impl UniErr {
    pub fn str<S: ToString>(s: S) -> UniErr {
        UniErr::new(500, s)
    }

    pub fn map<S>(s: S) -> Self
    where
        S: ToString,
    {
        UniErr::new(500, s)
    }

    pub fn from_status(status: u16) -> UniErr {
        let message = match status {
            400 => "Bad Request".to_string(),
            404 => "Not Found".to_string(),
            403 => "Forbidden".to_string(),
            408 => "Timeout".to_string(),
            500 => "Internal Server Error".to_string(),
            status => format!("{} Error", status),
        };
        UniErr::Status { status, message }
    }
}

/*
impl Into<ParseErrs> for UniErr {
    fn into(self) -> ParseErrs {
        match self {
            UniErr::Status { status, message } => {
                let mut builder = Report::build(ReportKind::Error, (), 0);
                let report = builder.with_message(message).finish();
                let errs = ParseErrs {
                    report: vec![report],
                    source: None,
                };
                errs
            }
            UniErr::ParseErrs(errs) => errs,
        }
    }
}

 */

impl UniErr {
    pub fn timeout() -> Self {
        UniErr::from_status(408)
    }

    pub fn server_error() -> Self {
        UniErr::from_status(500)
    }
    pub fn forbidden() -> Self {
        UniErr::err403()
    }

    pub fn forbidden_msg<S: ToString>(msg: S) -> Self {
        UniErr::Status {
            status: 403,
            message: msg.to_string(),
        }
    }

    pub fn not_found() -> Self {
        UniErr::err404()
    }

    pub fn bad_request() -> Self {
        UniErr::from_status(400)
    }

    pub fn bad_request_msg<M: ToString>(m: M) -> Self {
        UniErr::Status {
            status: 400,
            message: m.to_string(),
        }
    }
}

impl Debug for UniErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UniErr::Status { status, message } => {
                f.write_str(format!("{}: {}", status, message).as_str())
            }
            UniErr::ParseErrs(errs) => {
                errs.print();
                f.write_str("Parse Errors... [Report redacted]")
            }
        }
    }
}

impl UniErr {
    pub fn print(&self) {
        match self {
            UniErr::Status { .. } => {
                println!("{}", self.to_string());
            }
            UniErr::ParseErrs(err) => err.print(),
        }
    }
}

impl UniErr {
    pub fn new<S: ToString>(status: u16, message: S) -> Self {
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

impl StatusErr for UniErr {
    fn status(&self) -> u16 {
        match self {
            UniErr::Status { status, .. } => status.clone(),
            UniErr::ParseErrs(_) => 500u16,
        }
    }

    fn message(&self) -> String {
        match self {
            UniErr::Status { status, message } => message.clone(),
            UniErr::ParseErrs(_) => "Error report".to_string(),
        }
    }
}

pub trait StatusErr {
    fn status(&self) -> u16;
    fn message(&self) -> String;
}

impl Display for UniErr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            UniErr::Status { status, message } => {
                f.write_str(format!("{}: {}", status, message).as_str())
            }
            UniErr::ParseErrs(errs) => {
                errs.print();
                f.write_str("Error Report...")
            }
        }
    }
}

impl std::error::Error for UniErr {}

impl<C> From<SendTimeoutError<C>> for UniErr {
    fn from(e: SendTimeoutError<C>) -> Self {
        UniErr::Status {
            status: 500,
            message: e.to_string(),
        }
    }
}

impl<C> From<tokio::sync::mpsc::error::SendError<C>> for UniErr {
    fn from(e: SendError<C>) -> Self {
        UniErr::from_500(e.to_string())
    }
}

impl<C> From<tokio::sync::broadcast::error::SendError<C>> for UniErr {
    fn from(e: tokio::sync::broadcast::error::SendError<C>) -> Self {
        UniErr::from_500(e.to_string())
    }
}

impl From<tokio::sync::watch::error::RecvError> for UniErr {
    fn from(e: tokio::sync::watch::error::RecvError) -> Self {
        UniErr::from_500(e.to_string())
    }
}

impl From<String> for UniErr {
    fn from(message: String) -> Self {
        Self::Status {
            status: 500,
            message,
        }
    }
}

impl From<Elapsed> for UniErr {
    fn from(e: Elapsed) -> Self {
        Self::Status {
            status: 408,
            message: e.to_string(),
        }
    }
}

impl<T> From<PoisonError<T>> for UniErr {
    fn from(e: PoisonError<T>) -> Self {
        UniErr::Status {
            status: 500,
            message: e.to_string(),
        }
    }
}

impl From<FromUtf8Error> for UniErr {
    fn from(message: FromUtf8Error) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<&str> for UniErr {
    fn from(message: &str) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for UniErr {
    fn from(message: Box<bincode::ErrorKind>) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<Infallible> for UniErr {
    fn from(i: Infallible) -> Self {
        Self::Status {
            status: 500,
            message: i.to_string(),
        }
    }
}

impl From<nom::Err<VerboseError<&str>>> for UniErr {
    fn from(error: nom::Err<VerboseError<&str>>) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<semver::Error> for UniErr {
    fn from(error: semver::Error) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<ErrorTree<&str>> for UniErr {
    fn from(error: ErrorTree<&str>) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<strum::ParseError> for UniErr {
    fn from(error: strum::ParseError) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<()> for UniErr {
    fn from(err: ()) -> Self {
        Self::Status {
            status: 500,
            message: "Empty Error".to_string(),
        }
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for UniErr {
    fn from(err: RecvError) -> Self {
        Self::Status {
            status: 500,
            message: err.to_string(),
        }
    }
}

impl From<ParseIntError> for UniErr {
    fn from(x: ParseIntError) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

impl From<regex::Error> for UniErr {
    fn from(x: regex::Error) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

/*
impl From<ToStrError> for UniErr {
    fn from(x: ToStrError) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

 */

impl<I: Span> From<nom::Err<ErrorTree<I>>> for UniErr {
    fn from(err: Err<ErrorTree<I>>) -> Self {
        fn handle<I: Span>(err: ErrorTree<I>) -> UniErr {
            match err {
                ErrorTree::Base {
                    location,
                    kind: _kind,
                } => UniErr::Status {
                    status: 500,
                    message: format!(
                        "parse error line: {} column: {}",
                        location.location_line(),
                        location.get_column()
                    ),
                },
                ErrorTree::Stack { base, contexts } => match contexts.first() {
                    None => UniErr::Status {
                        status: 500,
                        message: "error, cannot find location".to_string(),
                    },
                    Some((location, _)) => UniErr::Status {
                        status: 500,
                        message: format!(
                            "Stack parse error line: {} column: {}",
                            location.location_line(),
                            location.get_column()
                        ),
                    },
                },
                ErrorTree::Alt(what) => UniErr::Status {
                    status: 500,
                    message: "alt error".to_string(),
                },
            }
        }
        match err {
            Err::Incomplete(_) => UniErr::Status {
                status: 500,
                message: "unexpected incomplete parsing error".to_string(),
            },

            Err::Error(err) => handle(err),
            Err::Failure(err) => handle(err),
        }
    }
}

impl Into<String> for UniErr {
    fn into(self) -> String {
        self.to_string()
    }
}

impl From<io::Error> for UniErr {
    fn from(e: io::Error) -> Self {
        UniErr::new(500, e.to_string().as_str())
    }
}

impl From<ParseErrs> for UniErr {
    fn from(errs: ParseErrs) -> Self {
        UniErr::ParseErrs(errs)
    }
}
impl<I: Span> From<nom::Err<ErrorTree<I>>> for ParseErrs {
    fn from(err: Err<ErrorTree<I>>) -> Self {
        match find_parse_err(&err) {
            UniErr::Status { .. } => ParseErrs {
                report: vec![],
                source: None,
            },
            UniErr::ParseErrs(parse_errs) => parse_errs,
        }
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

    pub fn from_loc_span<I: Span>(message: &str, label: &str, span: I) -> UniErr {
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

    pub fn from_range(message: &str, label: &str, range: Range<usize>, extra: SpanExtra) -> UniErr {
        let mut builder = Report::build(ReportKind::Error, (), 23);
        let report = builder
            .with_message(message)
            .with_label(Label::new(range).with_message(label))
            .finish();
        return ParseErrs::from_report(report, extra).into();
    }

    pub fn from_owned_span<I: Span>(message: &str, label: &str, span: I) -> UniErr {
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

    pub fn print(&self) {}

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

impl From<UniErr> for ParseErrs {
    fn from(u: UniErr) -> Self {
        ParseErrs {
            report: vec![],
            source: None,
        }
    }
}

impl From<serde_urlencoded::de::Error> for UniErr {
    fn from(err: serde_urlencoded::de::Error) -> Self {
        UniErr::Status {
            status: 500u16,
            message: err.to_string(),
        }
    }
}

impl From<serde_urlencoded::ser::Error> for UniErr {
    fn from(err: serde_urlencoded::ser::Error) -> Self {
        UniErr::Status {
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

    impl Default for Report {
        fn default() -> Self {
            Self {
                kind: ReportKind::Error,
                code: None,
                msg: None,
                note: None,
                help: None,
                location: Range { start: 0, end: 0 },
                labels: vec![],
            }
        }
    }

    pub struct ReportBuilder {}

    impl ReportBuilder {
        pub fn with_message<S: ToString>(&self, message: S) -> MessageBuilder {
            MessageBuilder {}
        }
    }

    pub struct MessageBuilder {}

    impl MessageBuilder {
        pub fn with_label(&self, label: Label) -> LabelBuilder {
            LabelBuilder {}
        }
    }

    pub struct LabelBuilder;

    impl LabelBuilder {
        pub fn finish(&self) -> Report {
            Default::default()
        }
    }

    impl Report {
        pub(crate) fn build(p0: ReportKind, p1: (), p2: i32) -> ReportBuilder {
            ReportBuilder {}
        }
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

    impl Label {
        pub fn new(range: std::ops::Range<usize>) -> Self {
            Self {
                span: Range {
                    start: range.start as u32,
                    end: range.end as u32,
                },
                msg: None,
                color: None,
                order: 0,
                priority: 0,
            }
        }

        pub fn with_message(self, msg: &str) -> Label {
            self
        }
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
