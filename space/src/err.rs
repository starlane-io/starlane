use anyhow::anyhow;
use bincode::ErrorKind;
use nom::error::{FromExternalError, VerboseError};
use serde::de::Error;
use std::convert::Infallible;
use std::fmt::{Debug, Display, Formatter};
use std::io;
use std::num::ParseIntError;
use std::ops::Range;
use std::string::FromUtf8Error;
use std::sync::{Arc, PoisonError};
use nom_supreme::error::BaseErrorKind;
use tokio::sync::mpsc::error::{SendError, SendTimeoutError};
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;

use crate::parse::util::Span;
use crate::parse::util::SpanExtra;

use crate::artifact::asynch::ArtErr;
use crate::command::direct::create::KindTemplate;
use crate::kind::BaseKind;
use crate::parse::ResolverErr;
use crate::point::PointSegKind;
use crate::substance::{Substance, SubstanceErr, SubstanceKind};
use crate::wave::core::http2::StatusCode;
use crate::wave::core::{Method, ReflectedCore};
use serde::{Deserialize, Serialize};
use strum::{IntoEnumIterator, ParseError};
use thiserror::Error;
use starlane_space::parse::SpaceTree;
use starlane_space::status;
use crate::err::report::{Label, Report, ReportKind};
use crate::particle::StatusDetail;
use crate::status::Status;
/*
#[macro_export]
macro_rules! err {
    ($($tt:tt)*) => {
        SpaceErr::Msg(format!($($tt)*).to_string())
    }
}

 */


#[derive(Debug, Clone, Error)]
pub enum SpaceErr {
    #[error("{status}: {message}")]
    Status { status: u16, message: String },
    #[error("Status: {0}")]
    Status2(status::Status),
    #[error(transparent)]
    ParseErrs(#[from] ParseErrs),
    #[error("expected substance: '{expected}' instead found: '{found}'")]
    ExpectedSubstance {
        expected: SubstanceKind,
        found: SubstanceKind,
    },
    #[error(
        "because method was '{method}' expected substance: '{expected}' instead found:  '{found}'"
    )]
    ExpectedBody {
        method: Method,
        expected: SubstanceKind,
        found: SubstanceKind,
    },
    #[error("not implemented: {0}")]
    NotImplemented(String),
    #[error("platform does not have a kind that matches template '{0}'")]
    KindNotAvailable(KindTemplate),
    #[error("expected a sub kind for base kind '{kind}' ... known options: [{subs}]")]
    ExpectedSub { kind: BaseKind, subs: String },
    #[error("{0}")]
    Msg(String),
    #[error("expecting a wildcard in point template.  found: '{0}'")]
    ExpectingWildcardInPointTemplate(String),
    #[error("cannot push to terminal '{0}' PointSegment.")]
    PointPushTerminal(PointSegKind),
    #[error("cannot push a non FileSystem PointSegment '{0}' onto a point until after the FileSystemRoot ':/' segment has been pushed"
    )]
    PointPushNoFileRoot(PointSegKind),
    #[error("expected '{kind}' : '{expected}' found: '{found}'")]
    Expected {
        kind: String,
        expected: String,
        found: String,
    },
    #[error("the root logger is not available. This probably means that the RootLogger initialization is not happening soon enough."
    )]
    RootLoggerNotInt,
    #[error("the root logger has already been initialized therefore another RootLogger cannot be created."
    )]
    RootLoggerAlreadyInit,
    #[error("{0}")]
    Anyhow(#[from] Arc<anyhow::Error>),
}

impl From<status::Status> for SpaceErr {
    fn from(status: Status) -> Self {
        SpaceErr::Status2(status)
    }
}

impl From<status::StatusDetail> for SpaceErr {
    fn from(detail: status::StatusDetail) -> Self {
         let status: status::Status = detail.into();
         status.into()
    }
}



/*    #[error("artifact error: '{0}'")]
    #[serde(skip_serializing)]
//    ArtErr(#[from] ArtErr),
 //   #[error("Err: {0}")]
 //   #[serde(skip_serializing)]
//    Any(#[source] Arc<anyhow::Error>),

 */
impl SpaceErr {
    pub fn err<E>(err: E) -> Self
    where
        E: std::error::Error,
    {
        Self::Msg(err.to_string())
    }

    pub fn to_space_err<E>(err: E) -> Self
    where
        E: ToString,
    {
        Self::Status {
            status: 500u16,
            message: err.to_string(),
        }
    }

    pub fn kind_not_available(template: &KindTemplate) -> Self {
        Self::KindNotAvailable(template.clone())
    }

    pub fn expect_sub<S>(kind: BaseKind) -> Self
    where
        S: IntoEnumIterator + ToString,
    {
        let mut subs = vec![];
        for sub in S::iter() {
            subs.push(sub.to_string());
        }
        let subs = subs.join(", ").to_string();

        Self::ExpectedSub { kind, subs }
    }

    pub fn expected<K, E>(kind: K, expected: E, found: Option<E>) -> Self
    where
        E: ToString,
        K: ToString,
    {
        let kind = kind.to_string();
        let expected = expected.to_string();
        let found = match found {
            None => "None".to_string(),
            Some(some) => some.to_string(),
        };
        Self::Expected {
            kind,
            expected,
            found,
        }
    }
}

impl SpatialError for SpaceErr {}

impl From<anyhow::Error> for SpaceErr {
    fn from(err: anyhow::Error) -> Self {
        Arc::new(err).into()
    }
}

impl SpatialError for ParseErrs {
    fn anyhow(self) -> Arc<anyhow::Error> {
        // first promote it to a space err...
        SpaceErr::from(self).anyhow()
    }
}

impl PrintErr for SpaceErr {
    fn print(&self) {
        match self {
            SpaceErr::Status { status, message } => {}
            SpaceErr::ParseErrs(errs) => {
                for report in &errs.report {
                    let report: ariadne::Report = report.clone().into();
                    report.print(ariadne::Source::from(&errs.src));
                }
            }
            _ => {
                println!("{}", self);
            }
        }
    }
}

impl From<ArtErr> for SpaceErr {
    fn from(err: ArtErr) -> Self {
        SpaceErr::Msg(err.to_string())
    }
}

impl SpaceErr {
    pub fn expected_substance(expected: SubstanceKind, found: SubstanceKind) -> Self {
        Self::ExpectedSubstance { expected, found }
    }

    pub fn unimplemented<S>(s: S) -> Self
    where
        S: ToString,
    {
        Self::NotImplemented(s.to_string())
    }
}

impl Into<ReflectedCore> for SpaceErr {
    fn into(self) -> ReflectedCore {
        match self {
            SpaceErr::Status { status, .. } => ReflectedCore {
                headers: Default::default(),
                status: StatusCode::from_u16(status).unwrap_or(StatusCode::from_u16(500).unwrap()),
                body: self.to_substance(),
            },
            SpaceErr::ParseErrs(_) => ReflectedCore {
                headers: Default::default(),
                status: StatusCode::from_u16(500u16).unwrap_or(StatusCode::from_u16(500).unwrap()),
                body: self.to_substance(),
            },
            x => ReflectedCore {
                headers: Default::default(),
                status: Default::default(),
                body: x.to_substance(),
            },
        }
    }
}

pub trait CoreReflector {
    fn as_reflected_core(self) -> ReflectedCore;
}

impl CoreReflector for SpaceErr {
    fn as_reflected_core(self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(self.status()).unwrap(),
            body: self.to_substance(),
        }
    }
}

impl SpaceErr {
    pub fn str<S: ToString>(s: S) -> SpaceErr {
        SpaceErr::new(500, s)
    }

    pub fn map<S>(s: S) -> Self
    where
        S: ToString,
    {
        SpaceErr::new(500, s)
    }

    pub fn from_status(status: u16) -> SpaceErr {
        let message = match status {
            400 => "Bad Request".to_string(),
            404 => "Not Found".to_string(),
            403 => "Forbidden".to_string(),
            408 => "Timeout".to_string(),
            500 => "Internal Server Error".to_string(),
            status => format!("{} Error", status),
        };
        SpaceErr::Status { status, message }
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

impl SpaceErr {
    pub fn timeout<S: ToString>(s: S) -> Self {
        SpaceErr::new(408, format!("Timeout: {}", s.to_string()))
    }

    pub fn server_error<S: ToString>(s: S) -> Self {
        SpaceErr::new(500, format!("Server Side Error: {}", s.to_string()))
    }
    pub fn forbidden<S: ToString>(s: S) -> Self {
        SpaceErr::new(403, format!("Forbidden: {}", s.to_string()))
    }

    pub fn not_found<S: ToString>(s: S) -> Self {
        SpaceErr::new(404, format!("Not Found: {}", s.to_string()))
    }

    pub fn bad_request<S: ToString>(s: S) -> Self {
        SpaceErr::new(400, format!("Bad Request: {}", s.to_string()))
    }
}

impl SpaceErr {
    pub fn new<S: ToString>(status: u16, message: S) -> Self {
        if message.to_string().as_str() == "500" {
            panic!("500 err message");
        }

        Self::Status {
            status,
            message: message.to_string(),
        }
    }
}

impl LegacyStatusErr for SpaceErr {
    fn status(&self) -> u16 {
        match self {
            SpaceErr::Status { status, .. } => status.clone(),
            _ => 500u16,
        }
    }

    fn message(&self) -> String {
        match self {
            SpaceErr::Status { status, message } => message.clone(),
            SpaceErr::ParseErrs(err) => err.to_string(),
            err => err.to_string(),
        }
    }
}

pub trait LegacyStatusErr {
    fn status(&self) -> u16;
    fn message(&self) -> String;
}

impl<C> From<SendTimeoutError<C>> for SpaceErr {
    fn from(e: SendTimeoutError<C>) -> Self {
        SpaceErr::Status {
            status: 500,
            message: e.to_string(),
        }
    }
}

impl From<&SubstanceErr> for SpaceErr {
    fn from(err: &SubstanceErr) -> Self {
        SpaceErr::from(err.to_string())
    }
}

impl<C> From<tokio::sync::mpsc::error::SendError<C>> for SpaceErr {
    fn from(e: SendError<C>) -> Self {
        SpaceErr::server_error(e.to_string())
    }
}

impl<C> From<tokio::sync::broadcast::error::SendError<C>> for SpaceErr {
    fn from(e: tokio::sync::broadcast::error::SendError<C>) -> Self {
        SpaceErr::server_error(e.to_string())
    }
}

impl From<tokio::sync::watch::error::RecvError> for SpaceErr {
    fn from(e: tokio::sync::watch::error::RecvError) -> Self {
        SpaceErr::server_error(e.to_string())
    }
}

impl From<Elapsed> for SpaceErr {
    fn from(e: Elapsed) -> Self {
        Self::Status {
            status: 408,
            message: e.to_string(),
        }
    }
}

impl<T> From<PoisonError<T>> for SpaceErr {
    fn from(e: PoisonError<T>) -> Self {
        SpaceErr::Status {
            status: 500,
            message: e.to_string(),
        }
    }
}

impl From<FromUtf8Error> for SpaceErr {
    fn from(message: FromUtf8Error) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<&str> for SpaceErr {
    fn from(message: &str) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for SpaceErr {
    fn from(message: Box<bincode::ErrorKind>) -> Self {
        Self::Status {
            status: 500,
            message: message.to_string(),
        }
    }
}

impl From<Infallible> for SpaceErr {
    fn from(i: Infallible) -> Self {
        Self::Status {
            status: 500,
            message: i.to_string(),
        }
    }
}

impl From<nom::Err<VerboseError<&str>>> for SpaceErr {
    fn from(error: nom::Err<VerboseError<&str>>) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<semver::Error> for SpaceErr {
    fn from(error: semver::Error) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<strum::ParseError> for SpaceErr {
    fn from(error: strum::ParseError) -> Self {
        Self::Status {
            status: 500,
            message: error.to_string(),
        }
    }
}

impl From<()> for SpaceErr {
    fn from(err: ()) -> Self {
        Self::Status {
            status: 500,
            message: "Empty Error".to_string(),
        }
    }
}

impl From<tokio::sync::oneshot::error::RecvError> for SpaceErr {
    fn from(err: RecvError) -> Self {
        Self::Status {
            status: 500,
            message: err.to_string(),
        }
    }
}

impl From<ParseIntError> for SpaceErr {
    fn from(x: ParseIntError) -> Self {
        Self::Status {
            status: 500,
            message: x.to_string(),
        }
    }
}

impl From<regex::Error> for SpaceErr {
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

/*
impl<I: Span> From<nom::Err<ErrorTree<I>>> for SpaceErr {
    fn from(err: Err<ErrorTree<I>>) -> Self {
        fn handle<I: Span>(err: ErrorTree<I>) -> SpaceErr {
            match err {
                ErrorTree::Base {
                    location,
                    kind: _kind,
                } => SpaceErr::Status {
                    status: 500,
                    message: format!(
                        "parse error line: {} column: {}",
                        location.location_line(),
                        location.get_column()
                    ),
                },
                ErrorTree::Stack { base, contexts } => match contexts.first() {
                    None => SpaceErr::Status {
                        status: 500,
                        message: "error, cannot find location".to_string(),
                    },
                    Some((location, _)) => SpaceErr::Status {
                        status: 500,
                        message: format!(
                            "Stack parse error line: {} column: {}",
                            location.location_line(),
                            location.get_column()
                        ),
                    },
                },
                ErrorTree::Alt(what) => SpaceErr::Status {
                    status: 500,
                    message: "alt error".to_string(),
                },
            }
        }
        match err {
            Err::Incomplete(_) => SpaceErr::Status {
                status: 500,
                message: "unexpected incomplete parsing error".to_string(),
            },

            Err::Error(err) => handle(err),
            Err::Failure(err) => handle(err),
        }
    }
}*/

impl Into<String> for SpaceErr {
    fn into(self) -> String {
        self.to_string()
    }
}

impl From<io::Error> for SpaceErr {
    fn from(e: io::Error) -> Self {
        SpaceErr::new(500, e.to_string().as_str())
    }
}

/// this is a dirty hack, but its late and I'm
/// frustrated with this particular FromStr::from_str err...
impl From<Report> for ParseErrs {
    fn from(report: Report) -> Self {
        ParseErrs::new(format!("{:?}",report))
    }
}




#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub struct ParseErrs {
    pub report: Vec<Report>,
    pub src: String,
}

impl ParseErrs {
    pub fn report(report: Report) -> Self {
        Self {
            report: vec![report],
            src: "".to_string(),
        }
    }

    pub fn expected<A, B, C>(thing: A, expected: B, found: C) -> Self
    where
        A: AsRef<str>,
        B: AsRef<str>,
        C: AsRef<str>,

    {
        let report = Report::build(ReportKind::Error, (), 0)
            .with_message(format!(
                "'{} expected: '{}' but found: '{}'",
                thing.as_ref(),
                expected.as_ref(),
                found.as_ref()
            ))
            .finish();
        Self::report(report)
    }

    pub fn result_utf8<R>(result: Result<R, FromUtf8Error>) -> Result<R, Self> {
        match result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(Self::new(&format!("ParseErrs(FromUtf8Error): {}", err))),
        }
    }

    pub fn utf8_encoding_err<I>(span: I, err: FromUtf8Error) -> ParseErrs
    where
        I: Span,
    {
        let err = err.to_string();
        Self::from_loc_span(err.as_str(), "FromUtf8Error", span)
    }

    pub fn new<M>(msg: M) -> Self
    where
        M: AsRef<str>,
    {
        let report = Report::build(ReportKind::Error, (), 0)
            .with_message(msg.as_ref())
            .finish();
        Self {
            report: vec![report],
            src: "".to_string(),
        }
    }
}

impl From<Box<bincode::ErrorKind>> for ParseErrs {
    fn from(err: Box<ErrorKind>) -> Self {
        Self::new(err.to_string())
    }
}

impl From<strum::ParseError> for ParseErrs {
    fn from(err: ParseError) -> Self {
        ParseErrs::new(err.to_string())
    }
}

impl From<&str> for ParseErrs {
    fn from(err: &str) -> Self {
        ParseErrs::new(err)
    }
}

impl From<&String> for ParseErrs {
    fn from(err: &String) -> Self {
        ParseErrs::new(err)
    }
}

impl From<ResolverErr> for ParseErrs {
    fn from(err: ResolverErr) -> Self {
        ParseErrs::new(err.to_string())
    }
}

impl From<String> for ParseErrs {
    fn from(err: String) -> Self {
        ParseErrs::new(err)
    }
}
impl From<Infallible> for ParseErrs {
    fn from(err: Infallible) -> Self {
        ParseErrs::new(err.to_string())
    }
}

impl From<FromUtf8Error> for ParseErrs {
    fn from(err: FromUtf8Error) -> Self {
        Self::result_utf8(Err(err)).unwrap()
    }
}

impl From<regex::Error> for ParseErrs {
    fn from(err: regex::Error) -> Self {
        Self::new(&err.to_string())
    }
}

impl Display for ParseErrs {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "ParseErrs -> {} errors", self.report.len())
    }
}

impl Default for ParseErrs {
    fn default() -> Self {
        Self {
            report: vec![],
            src: Default::default(),
        }
    }
}

impl From<semver::Error> for ParseErrs {
    fn from(err: semver::Error) -> Self {
        ParseErrs::new(&err.to_string())
    }
}

impl PrintErr for ParseErrs {
    fn print(&self) {
        println!("Report len: {}", self.report.len());
        for report in &self.report {
            let report: ariadne::Report = report.clone().into();
            report.print(ariadne::Source::from(&self.src));
        }
    }
}

impl ParseErrs {
    pub fn from_report<S>(report: Report, source: S) -> Self
    where
        S: ToString,
    {
        Self {
            report: vec![report],
            // not good that we are copying the string here... need to return to this and make it more efficient...
            src: source.to_string(),
        }
    }

    pub fn from_loc_span<I, S>(message: &str, label: S, span: I) -> ParseErrs
    where
        I: Span,
        S: ToString,
    {
        let mut builder = Report::build(ReportKind::Error, (), 23);
        let report = builder
            .with_message(message)
            .with_label(
                Label::new(span.location_offset()..(span.location_offset() + span.len()))
                    .with_message(label),
            )
            .finish();
        return ParseErrs::from_report(report, span.extra());
    }

    pub fn from_range(
        message: &str,
        label: &str,
        range: Range<usize>,
        extra: SpanExtra,
    ) -> ParseErrs {
        let mut builder = Report::build(ReportKind::Error, (), 23);
        let report = builder
            .with_message(message)
            .with_label(Label::new(range).with_message(label))
            .finish();
        return ParseErrs::from_report(report, extra);
    }

    pub fn from_owned_span<I: Span>(message: &str, label: &str, span: I) -> ParseErrs {
        let mut builder = Report::build(ReportKind::Error, (), 23);
        let report = builder
            .with_message(message)
            .with_label(
                Label::new(span.location_offset()..(span.location_offset() + span.len()))
                    .with_message(label),
            )
            .finish();
        return ParseErrs::from_report(report, span.extra());
    }

    pub fn fold<E: Into<ParseErrs>>(errs: Vec<E>) -> ParseErrs {
        let errs: Vec<ParseErrs> = errs.into_iter().map(|e| e.into()).collect();

        let source = if let Some(first) = errs.first() {
            first.src.clone()
        } else {
            Default::default()
        };

        let mut rtn = ParseErrs {
            report: vec![],
            src: source,
        };

        for err in errs {
            for report in err.report {
                rtn.report.push(report)
            }
        }
        rtn
    }
}

impl From<String> for SpaceErr {
    fn from(value: String) -> Self {
        SpaceErr::server_error(value)
    }
}

/*

impl From<SpaceErr> for ParseErrs {
    fn from(u: SpaceErr) -> Self {
        ParseErrs {
            report: vec![],
            source: None,
        }
    }
}

 */

impl Into<ParseErrs> for SpaceErr {
    fn into(self) -> ParseErrs {
        match self {
            SpaceErr::ParseErrs(errs) => errs,
            _ => Default::default(),
        }
    }
}

impl From<serde_urlencoded::de::Error> for SpaceErr {
    fn from(err: serde_urlencoded::de::Error) -> Self {
        SpaceErr::Status {
            status: 500u16,
            message: err.to_string(),
        }
    }
}

impl From<serde_urlencoded::ser::Error> for SpaceErr {
    fn from(err: serde_urlencoded::ser::Error) -> Self {
        SpaceErr::Status {
            status: 500u16,
            message: err.to_string(),
        }
    }
}

pub mod report {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct Report {
        kind: ReportKind,
        code: Option<String>,
        msg: Option<String>,
        note: Option<String>,
        help: Option<String>,
        location: Range,
        labels: Vec<Label>,
    }

    impl Into<ariadne::Report> for Report {
        fn into(self) -> ariadne::Report {
            let mut builder = ariadne::Report::build(self.kind.into(), (), 0);
            if let Some(msg) = self.msg {
                builder.set_message(msg);
            }
            for label in self.labels {
                builder.add_label(label.into());
            }
            builder.finish()
        }
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

    pub struct ReportBuilder {
        kind: ReportKind,
        code: Option<String>,
        msg: Option<String>,
        note: Option<String>,
        help: Option<String>,
        location: Range,
        labels: Vec<Label>,
    }

    impl ReportBuilder {
        pub fn with_message<S: ToString>(mut self, message: S) -> Self {
            self.msg.replace(message.to_string());
            self
        }

        pub fn with_label(mut self, label: Label) -> Self {
            self.labels.push(label);
            self
        }

        pub fn finish(self) -> Report {
            Report {
                kind: self.kind,
                code: None,
                msg: self.msg,
                note: None,
                help: None,
                location: self.location,
                labels: self.labels,
            }
        }
    }

    impl Report {
        pub(crate) fn build(kind: ReportKind, p1: (), p2: i32) -> ReportBuilder {
            ReportBuilder {
                kind,
                code: None,
                msg: None,
                note: None,
                help: None,
                location: Default::default(),
                labels: vec![],
            }
        }
    }

    #[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq)]
    pub enum ReportKind {
        Error,
        Warning,
        Advice,
    }

    impl Into<ariadne::ReportKind> for ReportKind {
        fn into(self) -> ariadne::ReportKind {
            match self {
                ReportKind::Error => ariadne::ReportKind::Error,
                ReportKind::Warning => ariadne::ReportKind::Warning,
                ReportKind::Advice => ariadne::ReportKind::Advice,
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct Range {
        pub start: u32,
        pub end: u32,
    }

    impl Into<std::ops::Range<usize>> for Range {
        fn into(self) -> std::ops::Range<usize> {
            std::ops::Range {
                start: self.start as usize,
                end: self.end as usize,
            }
        }
    }

    impl Default for Range {
        fn default() -> Self {
            Self { start: 0, end: 0 }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct Label {
        span: Range,
        msg: Option<String>,
        color: Option<Color>,
        order: i32,
        priority: i32,
    }

    impl Into<ariadne::Label> for Label {
        fn into(self) -> ariadne::Label {
            let mut rtn = ariadne::Label::new(self.span.into());
            if let Some(msg) = self.msg {
                rtn = rtn.with_message(msg);
            }
            rtn
        }
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

        pub fn with_message<S>(mut self, msg: S) -> Label
        where
            S: ToString,
        {
            self.msg.replace(msg.to_string());
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


pub trait PrintErr {
    fn print(&self);
}

pub trait ToSpaceErr
where
    Self: Display,
{
    fn to_space_err(&self) -> SpaceErr {
        SpaceErr::Msg(format!("{}", self).to_string())
    }
}

#[derive(Clone, Debug, Error)]
pub enum AutoboxErr {
    #[error("cannot convert '{thing}' from '{from}' to '{to}'")]
    CannotConvert {
        thing: String,
        from: String,
        to: String,
    },
}

impl AutoboxErr {
    pub fn no_into<A, B, C>(thing: A, from: B, to: C) -> Self
    where
        A: AsRef<str>,
        B: AsRef<str>,
        C: AsRef<str>,
    {
        Self::CannotConvert {
            thing: thing.as_ref().to_string(),
            from: from.as_ref().to_string(),
            to: to.as_ref().to_string(),
        }
    }
}

/*
impl From<strum::ParseErr> for ParseErrs {
    fn from(err: ParseError) -> Self {
        ParseErrs::new(err.to_string())
    Err}
}

 */

pub trait SpatialError
where
    Self: Sized + ToString,
{
    fn anyhow(self) -> Arc<anyhow::Error> {
        Arc::new(anyhow!(self.to_string()))
    }

    fn to_substance(self) -> Substance {
        Substance::Err(SubstanceErr(self.to_string()))
    }
}

pub fn any_result<R, E>(result: Result<R, E>) -> Result<R, Arc<anyhow::Error>>
where
    E: Display,
{
    match result {
        Ok(ok) => Ok(ok),
        Err(err) => Err(Arc::new(anyhow!("{}", err))),
    }
}

pub trait HyperSpatialError: SpatialError {}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn compile() {}
}
