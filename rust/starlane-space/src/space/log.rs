use crate::space::err::SpaceErr;
use crate::space::loc;
use crate::space::loc::{Layer, ToPoint, ToSurface, Uuid};
use crate::space::parse::CamelCase;
use crate::space::point::Point;
use crate::space::selector::Selector;
use crate::space::substance::LogSubstance;
use crate::space::util::{timestamp, uuid};
use crate::space::wasm::Timestamp;
use crate::space::wave::core::cmd::CmdMethod;
use crate::space::wave::exchange::synch::{ProtoTransmitter, ProtoTransmitterBuilder};
use crate::space::wave::exchange::SetStrategy;
use crate::space::wave::{DirectedProto, Handling, HandlingKind, Priority, Retries, ToRecipients, WaitTime, Wave};
use crate::Agent;
use core::str::FromStr;
use derive_builder::Builder;
use once_cell::sync::Lazy;
use regex::Regex;
use serde;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use starlane_primitive_macros::{create_mark, mark};
use std::cell::LazyCell;
use std::collections::HashMap;
use std::future::Future;
use std::io::Write;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, LazyLock};
use tokio::task_local;
use log::warn;
use crate::space::particle::traversal::Traversal;

task_local! {
    static LOGGER: SpanLogger;
}




pub async fn log_entry_point<F>(f: F)
where
    F: FnMut()+Future,
{
    if LOGGER.try_with(|v|{}).is_ok() {

    }

    let root = root_logger();
    let span= root.span();
    LOGGER.scope(span, f).await;
}

static ROOT_LOGGER: LazyLock<RootLogger> = LazyLock::new(|| unsafe {
    match starlane_root_log_appender() {
        Ok(appender) => RootLogger {
            source: LogSource::Shell,
            appender,
        },
        Err(err) => {
            let appender = Arc::new(StdOutAppender());
            let logger = RootLogger {
                source: LogSource::Shell,
                appender,
            };
            logger
        }
    }
});
pub fn root_logger() -> RootLogger {
    ROOT_LOGGER.clone()
}

#[no_mangle]
extern "C" {
    pub fn starlane_root_log_appender() -> Result<Arc<dyn LogAppender>, SpaceErr>;
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl Default for Level {
    fn default() -> Self {
        Level::Info
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq,Builder)]
pub struct Log {
    #[builder(default)]
    pub point: Option<Point>,
    #[builder(default)]
    pub mark: Option<LogMark>,
    #[builder(default)]
    pub action: Option<CamelCase>,
    pub source: LogSource,
    #[builder(default)]
    pub span: Option<Uuid>,
    pub timestamp: i64,
    pub payload: LogPayload,
    pub level: Level,
}

impl ToString for Log {
    fn to_string(&self) -> String {
        match &self.point {
            None => {
                format!("None {} {}",
                self.level.to_string(),
                self.payload.to_string()).to_string()
            }
            Some(point) => {
                format!("{} {} {}",
                point.to_string(),
                self.level.to_string(),
                self.payload.to_string()).to_string()
            }
        }

    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
pub enum LogSource {
    Shell,
    Core,
}
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum LogSpanEventKind {
    Entry,
    Exit,
}

/*
pub trait SpanEvent {
    fn point(&self) -> &Option<Point>;
    fn span_id(&self) -> &Uuid;

    fn kind(&self) -> &LogSpanEventKind;

    fn attributes(&self) -> &HashMap<String, String>;

    fn mark(&self) -> LogMark;
}

 */

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SpanEvent {
    pub point: Option<Point>,
    pub span: Uuid,
    pub kind: LogSpanEventKind,
    pub attributes: HashMap<String, String>,
    pub timestamp: Timestamp,
    pub mark: LogMark,
}

impl SpanEvent {}


impl SpanEvent {

    pub fn opt_point(
        span: &LogSpan,
        point: &Option<Point>,
        kind: LogSpanEventKind,
        attributes: HashMap<String, String>,
        mark: LogMark,
    ) -> SpanEvent {

        match point {
            None => Self::no_point(span, kind, attributes, mark),
            Some(point) => Self::point( span, point, kind, attributes, mark)
        }

    }


    pub fn no_point(
        span: &LogSpan,
        kind: LogSpanEventKind,
        attributes: HashMap<String, String>,
        mark: LogMark,
    ) -> SpanEvent {
        SpanEvent {
            span: span.id.clone(),
            point: None,
            kind,
            attributes,
            timestamp: timestamp(),
            mark,
        }
    }
    pub fn point(
        span: &LogSpan,
        point: &Point,
        kind: LogSpanEventKind,
        attributes: HashMap<String, String>,
        mark: LogMark,
    ) -> SpanEvent {
        SpanEvent {
            span: span.id.clone(),
            point: Option::Some(point.clone()),
            kind,
            attributes,
            timestamp: timestamp(),
            mark,
        }
    }
}

pub type TrailSpanId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct LogSpan {
    pub id: TrailSpanId,
    pub point: Option<Point>,
    pub mark: Option<LogMark>,
    pub action: Option<CamelCase>,
    pub parent: Option<Uuid>,
    pub attributes: HashMap<String, String>,
    pub entry_timestamp: Timestamp,
}

impl LogSpan {
    pub fn new(point: Point) -> Self {
        Self {
            id: uuid(),
            point: Some(point),
            mark: None,
            action: None,
            parent: None,
            attributes: Default::default(),
            entry_timestamp: timestamp(),
        }
    }

    pub fn parent(point: Point, parent: Uuid) -> Self {
        Self {
            id: uuid(),
            point: Some(point),
            mark: None,
            action: None,
            parent: Some(parent),
            attributes: Default::default(),
            entry_timestamp: timestamp(),
        }
    }

    pub fn optional(point: Option<Point>) -> Self {
        Self {
            id: uuid(),
            point,
            mark: None,
            action: None,
            parent: None,
            attributes: Default::default(),
            entry_timestamp: timestamp(),
        }
    }
    pub fn pointless() -> Self {
        Self {
            id: uuid(),
            point: None,
            mark: None,
            action: None,
            parent: None,
            attributes: Default::default(),
            entry_timestamp: timestamp(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PointlessLog {
    timestamp: Timestamp,
    message: String,
    level: Level,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum LogPayload {
    Message(String),
    Json(Value),
    Both { message: String, json: Value },
}

impl ToString for LogPayload {
    fn to_string(&self) -> String {
        match self {
            LogPayload::Message(message) => message.clone(),
            LogPayload::Json(json) => json.to_string(),
            LogPayload::Both { json, message } => {
                format!("{} {}", json.to_string(), message.clone())
            }
        }
    }
}

pub struct RootLoggerBuilder {
    pub point: Option<Point>,
    pub span: Option<Uuid>,
    pub logger: RootLogger,
    pub level: Level,
    pub message: Option<String>,
    pub json: Option<Value>,
    msg_overrides: Vec<String>,
//    topic_tx: HashMap<String, tokio::sync::mpsc::Sender<LogTopicState>>,
}

impl RootLoggerBuilder {
    pub fn new(logger: RootLogger, span: Option<Uuid>) -> Self {
        RootLoggerBuilder {
            logger,
            span,
            point: None,
            level: Level::default(),
            message: None,
            json: None,
            msg_overrides: vec![],
//            topic_tx: Default::default(),
        }
    }

    pub fn level(mut self, level: Level) -> Self {
        self.level = level;
        self
    }

    pub fn trace(mut self) -> Self {
        self.level = Level::Trace;
        self
    }
    pub fn debug(mut self) -> Self {
        self.level = Level::Debug;
        self
    }
    pub fn info(mut self) -> Self {
        self.level = Level::Info;
        self
    }
    pub fn warn(mut self) -> Self {
        self.level = Level::Warn;
        self
    }
    pub fn error(mut self) -> Self {
        self.level = Level::Error;
        self
    }

    pub fn point(mut self, p: Point) -> Self {
        self.point = Some(p);
        self
    }

    pub fn msg<M>(mut self, m: M) -> Self
    where
        M: ToString,
    {
        self.message = Some(m.to_string());
        self
    }

    pub fn json<'a, J>(mut self, json: J) -> Self
    where
        J: Into<&'a str>,
    {
        match serde_json::from_str(json.into()) {
            Ok(json) => {
                self.json = Some(json);
            }
            Err(err) => {
                self.msg_overrides
                    .push(format!("error parsing log json: {}", err.to_string()));
            }
        }
        self
    }

    pub fn json_value(mut self, json: Value) -> Self {
        self.json = Some(json);
        self
    }

    pub fn commit(mut self) {
        if self.message.is_none() && self.json.is_none() {
            self.msg_overrides
                .push("Log must have either a message or json or both".to_string())
        }

        if self.point.is_none() {
            self.msg_overrides
                .push("Particle Point must be set for a Log".to_string())
        }

        let message = if self.msg_overrides.is_empty() {
            self.message
        } else {
            let mut rtn = String::new();
            rtn.push_str("LOG ERROR OVERRIDES: this means there was an error int he logging process itself.\n");
            for over in self.msg_overrides {
                rtn.push_str(over.as_str());
            }
            match self.message {
                None => {}
                Some(message) => {
                    rtn.push_str(format!("original message: {}", message).as_str());
                }
            }
            Some(rtn)
        };

        if self.point.is_none() {
            let log = PointlessLog {
                timestamp: timestamp(),
                message: message.expect("message"),
                level: Level::Error,
            };
            self.logger.pointless(log);
            return;
        }

        let content = if message.is_some() && self.json.is_none() {
            LogPayload::Message(message.expect("message"))
        } else if message.is_none() && self.json.is_some() {
            LogPayload::Json(self.json.expect("message"))
        } else if message.is_some() && self.json.is_some() {
            LogPayload::Both {
                message: message.expect("message"),
                json: self.json.expect("json"),
            }
        } else {
            panic!("LogBuilder: must set Logger before LogBuilder.send() can be called")
        };

        let point = self.point.expect("point");
        let log = Log {
            point: Option::Some(point),
            mark: None,
            action: None,
            level: self.level,
            timestamp: timestamp().timestamp_millis(),
            payload: content,
            source: self.logger.source(),
            span: self.span,
        };
        self.logger.log(log);
    }
}

pub trait LogAppender: Send + Sync {
    fn log(&self, log: Log);

    fn span_event(&self, log: SpanEvent);

    /// PointlessLog is used for error diagnosis of the logging system itself, particularly
    /// where there is parsing error due to a bad point
    fn pointless(&self, log: PointlessLog);
}

#[derive(Clone)]
pub struct RootLogger {
    source: LogSource,
    appender: Arc<dyn LogAppender>,
}

/*
impl Default for RootLogger {
    fn default() -> Self {
        RootLogger::new(LogSource::Core, Arc::new(StdOutAppender::new()))
    }
}

 */

impl RootLogger {
    fn source(&self) -> LogSource {
        self.source.clone()
    }

    fn log(&self, log: Log) {
        self.appender.log(log);
    }

    fn raw<R>(&self, txt: R, level: Level) where R: AsRef<str> {
        let mut builder = LogBuilder::default();
        builder.source(LogSource::Core);
        builder.timestamp(timestamp().millis);
        builder.payload(LogPayload::Message(txt.as_ref().to_string()));
        // technically unwrap should not fail unless Log was changed
        let log = builder.build().unwrap();
        self.log(log);
    }

    pub fn info<R>(&self, txt: R)  where R: AsRef<str> {
        self.raw(txt, Level::Info);
    }

    pub fn debug<R>(&self, txt: R)  where R: AsRef<str> {
        self.raw(txt, Level::Debug);
    }

    pub fn trace<R>(&self, txt: R)  where R: AsRef<str> {
        self.raw(txt, Level::Trace);
    }

    pub fn warn<R>(&self, txt: R)  where R: AsRef<str> {
        self.raw(txt, Level::Warn);
    }


    pub fn error<R>(&self, txt: R)  where R: AsRef<str> {
        self.raw(txt, Level::Error);
    }


    fn span_event(&self, log: SpanEvent) {
        self.appender.span_event(log);
    }

    /// PointlessLog is used for error diagnosis of the logging system itself, particularly
    /// where there is parsing error due to a bad point
    fn pointless(&self, log: PointlessLog) {
        self.appender.pointless(log);
    }

    pub fn point<P: ToPoint>(&self, point: P) -> PointLogger {
        PointLogger {
            logger: self.clone(),
            point: point.to_point(),
            mark: create_mark!(),
            action: None,
        }
    }

    pub fn span(&self) -> SpanLogger {
        let span = LogSpan::pointless();

        let logger = SpanLogger {
            root_logger: self.clone(),
            span: span.clone(),
            commit_on_drop: true,
        };

        self.span_event(SpanEvent::no_point(
            &span,
            LogSpanEventKind::Entry,
            Default::default(),
            create_mark!(),
        ));

        logger
    }
}
pub struct NoAppender {}

impl NoAppender {
    pub fn new() -> Self {
        NoAppender {}
    }
}

impl LogAppender for NoAppender {
    fn log(&self, log: Log) {}


    fn span_event(&self, log: SpanEvent) {}

    fn pointless(&self, log: PointlessLog) {}
}

pub struct StdOutAppender();

impl StdOutAppender {
    pub fn new() -> Self {
        StdOutAppender()
    }
}

impl LogAppender for StdOutAppender {
    fn log(&self, log: Log) {
        let action = match log.action {
            None => "None".to_string(),
            Some(action) => action.to_string(),
        };

        let point = match log.point{
            None => "None".to_string(),
            Some(point) => point.to_string(),
        };
        println!("{} | {}", point, log.payload.to_string())
    }


    fn span_event(&self, log: SpanEvent) {
        /*         println!(
                   "{} | Span({})",
                   log.point.to_string(),
                   log.span.to_string(),
               )

        */
    }

    fn pointless(&self, log: PointlessLog) {
        println!("{}", log.message);
    }
}

pub struct SynchTransmittingLogAppender {
    transmitter: ProtoTransmitter,
}

impl SynchTransmittingLogAppender {
    pub fn new(mut transmitter: ProtoTransmitterBuilder) -> Self {
        transmitter.method = SetStrategy::Override(CmdMethod::Log.into());
        transmitter.to = SetStrategy::Override(
            Point::global_logger()
                .to_surface()
                .with_layer(Layer::Core)
                .to_recipients(),
        );
        transmitter.handling = SetStrategy::Fill(Handling {
            kind: HandlingKind::Durable,
            priority: Priority::Low,
            retries: Retries::Medium,
            wait: WaitTime::High,
        });
        let transmitter = transmitter.build();
        Self { transmitter }
    }
}

impl LogAppender for SynchTransmittingLogAppender {
    fn log(&self, log: Log) {
        let mut directed = DirectedProto::signal();
        if let Some(from) = &log.point {
            directed.from(from.to_surface().clone());
            directed.agent(Agent::Point(from.clone()));
        }

        directed.body(LogSubstance::Log(log).into());
        self.transmitter.signal(directed);
    }


    fn span_event(&self, log: SpanEvent) {
        let point = log.point.clone();
        let mut directed = DirectedProto::signal();
        directed.from = point.clone().map(|p| p.to_surface());
        directed.agent = point.map(|p| Agent::Point(p.clone()));
        directed.body(LogSubstance::Event(log).into());
        self.transmitter.signal(directed);
    }

    fn pointless(&self, log: PointlessLog) {
        let mut directed = DirectedProto::signal();
        directed.from(Point::anonymous());
        directed.agent(Agent::Anonymous);
        directed.body(LogSubstance::Pointless(log).into());
        self.transmitter.signal(directed);
    }
}

#[derive(Clone)]
pub struct PointLogger {
    pub logger: RootLogger,
    pub point: Point,
    pub mark: LogMark,
    pub action: Option<CamelCase>,
}

impl Default for PointLogger {
    fn default() -> Self {
        Self {
            logger: root_logger(),
            point: Point::root(),
            mark: create_mark!(),
            action: None,
        }
    }
}

impl PointLogger {

    pub fn push_mark(&self, mark: &'static str) -> Result<SpanLogger,SpaceErr> {
        Ok(self.span())
    }


    pub fn source(&self) -> LogSource {
        self.logger.source()
    }

    pub fn span(&self) -> SpanLogger
    where
    {
        let mark = create_mark!();
        let span = LogSpan::new(self.point.clone());
        let logger = SpanLogger {
            root_logger: self.logger.clone(),
            span: span.clone(),
            commit_on_drop: true,
        };

        self.logger.span_event(SpanEvent::point(
            &span,
            &self.point,
            LogSpanEventKind::Entry,
            Default::default(),
            mark,
        ));

        logger
    }

    pub fn point(&self, point: Point) -> PointLogger {
        PointLogger {
            logger: self.logger.clone(),
            point,
            mark: create_mark!(),
            action: None,
        }
    }

    pub fn push_point<S: ToString>(&self, segs: S) -> Result<PointLogger, SpaceErr> {
        Ok(PointLogger {
            logger: self.logger.clone(),
            point: self.point.push(segs)?,
            mark: create_mark!(),
            action: None,
        })
    }

    /*
    pub fn pop_mark(&self) -> PointLogger {
        PointLogger {
            logger: self.logger.clone(),
            point: self.point.clone(),
            mark: self.mark.pop(),
            action: self.action.clone(),
        }
    }

    pub fn push_mark<S: ToString>(&self, segs: S) -> Result<PointLogger, SpaceErr> {
        Ok(PointLogger {
            logger: self.logger.clone(),
            point: self.point.clone(),
            mark: self.mark.push(segs)?,
            action: None,
        })
    }

     */

    pub fn push_action<A: ToString>(&self, action: A) -> Result<PointLogger, SpaceErr> {
        Ok(PointLogger {
            logger: self.logger.clone(),
            point: self.point.clone(),
            mark: self.mark.clone(),
            action: Some(CamelCase::from_str(action.to_string().as_str())?),
        })
    }

    pub fn msg<M>(&self, level: Level, message: M)
    where
        M: ToString,
    {
        let point = self.point.clone();
        let mark = self.mark.clone();

        self.logger.log(Log {
            point: Some(point),
            mark: Some(mark),
            action: self.action.clone(),
            level,
            timestamp: timestamp().timestamp_millis(),
            payload: LogPayload::Message(message.to_string()),
            span: None,
            source: self.logger.source(),
        })
    }

    pub fn handle(&self, log: LogSubstance) {
        match log {
            LogSubstance::Log(log) => {
                self.logger.log(log);
            }
            LogSubstance::Span(span) => {
                println!("start log span...");
                // not sure how to handle this
            }
            LogSubstance::Event(event) => {
                self.logger.span_event(event);
            }

            LogSubstance::Pointless(pointless) => {
                self.logger.pointless(pointless);
            }
        }
    }

    pub fn trace<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Trace, message);
    }

    pub fn debug<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Trace, message);
    }

    pub fn info<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Trace, message);
    }

    pub fn warn<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Warn, message);
    }

    pub fn error<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Error, message);
    }

    pub fn result<R, E>(&self, result: Result<R, E>) -> Result<R, E>
    where
        E: ToString,
    {
        match &result {
            Ok(_) => {}
            Err(err) => {
                //self.error(err.to_string());
                self.msg(Level::Error, err.to_string());
            }
        }
        result
    }

    pub fn eat<R, E>(&self, result: Result<R, E>)
    where
        E: ToString,
    {
        match &result {
            Ok(_) => {}
            Err(err) => {
                self.error(err.to_string());
            }
        }
    }

    pub fn result_ctx<R, E>(&self, ctx: &str, result: Result<R, E>) -> Result<R, E>
    where
        E: ToString,
    {
        match &result {
            Ok(_) => {}
            Err(err) => {
                self.msg(Level::Error, err.to_string());
            }
        }
        result
    }

    pub fn eat_ctx<R, E>(&self, ctx: &str, result: Result<R, E>)
    where
        E: ToString,
    {
        match &result {
            Ok(_) => {}
            Err(err) => {
                self.error(format!("{} {}", ctx, err.to_string()));
            }
        }
    }

    pub fn track<T, F>(&self, trackable: &T, f: F)
    where
        T: Trackable,
        F: FnOnce() -> Tracker,
    {
        if trackable.track() {
            let tracker = f();
            self.msg(tracker.level.clone(), trackable.track_fmt(&tracker));
        }
    }

    pub fn track_msg<T, F, M, S>(&self, trackable: &T, f: F, m: M)
    where
        T: Trackable,
        F: FnOnce() -> Tracker,
        M: FnOnce() -> S,
        S: ToString,
    {
        if trackable.track() {
            let tracker = f();
            let message = m().to_string();
            self.msg(
                tracker.level.clone(),
                format!("{} {}", trackable.track_fmt(&tracker), message),
            );
        }
    }
}

pub struct SpanLogBuilder {
    pub entry_timestamp: Timestamp,
    pub attributes: HashMap<String, String>,
}

impl SpanLogBuilder {
    pub fn new() -> Self {
        Self {
            entry_timestamp: timestamp(),
            attributes: HashMap::new(),
        }
    }
}



#[derive(Clone)]
pub struct SpanLogger
{
    pub root_logger: RootLogger,
    pub span: LogSpan,
    pub commit_on_drop: bool,
}

impl SpanLogger {
    pub fn push_action(&self, action: &str) -> Result<SpanLogger,SpaceErr>{
        self.warn(format!("push_action in logs is disabled {} ",action));
        Ok(self.span())
    }
}

impl SpanLogger {
    pub fn track<T,F>(&self, track: &T, tracker: F) where T: Trackable, F: FnMut() -> Tracker{
        self.warn(format!("tracking is broken tracking: '{}'", track.track_id()));
    }
}

impl SpanLogger
{
    pub fn span_uuid(&self) -> Uuid {
        self.span.id.clone()
    }

    pub fn point(&self) -> &Option<Point> {
        &self.span.point
    }


    pub fn push_mark(&self, mark: &'static str) -> Result<SpanLogger,SpaceErr> {
        Ok(self.span())
    }


    pub fn span(&self) -> Self
    {
        let point = self.point().clone();
        let span = LogSpan::optional(point);
        SpanLogger {
            root_logger: self.root_logger.clone(),
            span,
            commit_on_drop: true,
        }
    }

    pub fn span_attr(&self, attr: HashMap<String, String>) -> SpanLogger {
        let mut span = LogSpan::pointless();
        span.attributes = attr;
        SpanLogger {
            root_logger: self.root_logger.clone(),
            span,
            commit_on_drop: true,
        }
    }

    pub fn current_span(&self) -> &LogSpan {
        &self.span
    }

    pub fn entry_timestamp(&self) -> Timestamp {
        self.span.entry_timestamp.clone()
    }

    pub fn set_span_attr<K, V>(&mut self, key: K, value: V)
    where
        K: ToString,
        V: ToString,
    {
        self.span
            .attributes
            .insert(key.to_string(), value.to_string());
    }

    pub fn get_span_attr<K>(&self, key: K) -> Option<String>
    where
        K: ToString,
    {
        self.span.attributes.get(&key.to_string()).cloned()
    }

    pub fn msg<M>(&self, level: Level, message: M)
    where
        M: ToString,
    {
        let point = self.point().clone();
        let mark = self.span.mark.clone();

        self.root_logger.log(Log {
            point,
            mark,
            action: self.span.action.clone(),
            level,
            timestamp: timestamp().timestamp_millis(),
            payload: LogPayload::Message(message.to_string()),
            span: Some(self.span_uuid()),
            source: self.root_logger.source(),
        })
    }

    pub fn trace<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Trace, message);
    }

    pub fn debug<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Trace, message);
    }

    pub fn info<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Trace, message);
    }

    pub fn warn<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Warn, message);
    }

    pub fn error<M>(&self, message: M)
    where
        M: ToString,
    {
        self.msg(Level::Error, message);
    }





    pub fn result<R, E>(&self, result: Result<R, E>) -> Result<R, E>
    where
        E: ToString,
    {
        match &result {
            Ok(_) => {}
            Err(err) => {
                self.error(err.to_string());
            }
        }
        result
    }

    pub fn result_ctx<R, E>(&self, ctx: &str, result: Result<R, E>) -> Result<R, E>
    where
        E: ToString,
    {
        match &result {
            Ok(_) => {}
            Err(err) => {
                self.error(format!("{} {}", ctx, err.to_string()));
            }
        }
        result
    }

    pub fn push_point<S: ToString>(&self, segs: S) -> Result<PointLogger, SpaceErr> {
        Ok(PointLogger {
            logger: self.root_logger.clone(),
            point: match self.point() {
                None => Point::from_str(segs.to_string().as_str())?,
                Some(point) => point.clone().push(segs.to_string())?
            },
            mark: create_mark!(),
            action: None,
        })
    }
}

impl Drop for SpanLogger
{
    fn drop(&mut self) {
        if self.commit_on_drop {
            let log = SpanEvent::opt_point(
                &self.span,
                self.point(),
                LogSpanEventKind::Exit,
                self.span.attributes.clone(),
                create_mark![],
            );
            self.root_logger.span_event(log)
        }
    }
}

/*
pub struct LogBuilder {
    logger: RootLogger,
    builder: RootLoggerBuilder,
}

impl LogBuilder {
    pub fn new(logger: RootLogger, builder: RootLoggerBuilder) -> Self {
        LogBuilder { logger, builder }
    }

    pub fn trace(mut self) -> Self {
        self.builder = self.builder.trace();
        self
    }
    pub fn debug(mut self) -> Self {
        self.builder = self.builder.debug();
        self
    }
    pub fn info(mut self) -> Self {
        self.builder = self.builder.info();
        self
    }
    pub fn warn(mut self) -> Self {
        self.builder = self.builder.warn();
        self
    }
    pub fn error(mut self) -> Self {
        self.builder = self.builder.error();
        self
    }

    pub fn msg<M>(mut self, m: M) -> Self
    where
        M: ToString,
    {
        self.builder = self.builder.msg(m);
        self
    }

    pub fn json<'a, J>(mut self, json: J) -> Self
    where
        J: Into<&'a str>,
    {
        self.builder = self.builder.json(json);
        self
    }

    pub fn json_value(mut self, json: Value) -> Self {
        self.builder = self.builder.json_value(json);
        self
    }

    pub fn commit(mut self) {
        self.builder.commit();
    }
}

 */

/*
pub struct AuditLogBuilder {
    logger: RootLogger,
    point: Option<Point>,
    span: Uuid,
    attributes: HashMap<String, String>,
}

impl AuditLogBuilder {
    pub fn new(logger: RootLogger, point: Point, span: Uuid) -> Self {
        AuditLogBuilder {
            logger,
            point,
            attributes: HashMap::new(),
            span,
        }
    }

    // make nice appended call:
    // logger.audit().append("hello","kitty").commit();
    pub fn append<K: ToString, V: ToString>(mut self, key: K, value: V) -> Self {
        self.attributes.insert(key.to_string(), value.to_string());
        self
    }

    pub fn add<K: ToString, V: ToString>(&mut self, key: K, value: V) {
        self.attributes.insert(key.to_string(), value.to_string());
    }

    pub fn kind<K>(mut self, kind: K) -> Self
    where
        K: ToString,
    {
        self.attributes.insert("kind".to_string(), kind.to_string());
        self
    }

    pub fn commit(mut self) {
        let log = AuditLog {
            point: self.point,
            timestamp: timestamp(),
            metrics: self.attributes,
        };
        self.logger.audit(log)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct AuditLog {
    pub point: Point,
    pub timestamp: Timestamp,
    pub metrics: HashMap<String, String>,
}

 */

pub trait Spanner
{
    fn span_id(&self) -> String;
    fn span_type(&self) -> &'static str;
}

pub trait Trackable {
    fn track_id(&self) -> String;
    fn track_method(&self) -> String;
    fn track_payload(&self) -> String;
    fn track_from(&self) -> String;
    fn track_to(&self) -> String;
    fn track(&self) -> bool;

    fn track_payload_fmt(&self) -> String {
        self.track_payload()
    }

    fn track_key_fmt(&self) -> String {
        format!(
            "{}::<{}>::[{}]",
            self.track_id(),
            self.track_method(),
            self.track_payload_fmt()
        )
    }

    fn track_fmt(&self, tracker: &Tracker) -> String {
        format!(
            "{}<{}> : {} : ({} -> {})",
            tracker.parsec,
            tracker.action,
            self.track_key_fmt(),
            self.track_from(),
            self.track_to()
        )
    }
}

pub struct Tracker {
    pub parsec: String,
    pub action: String,
    pub level: Level,
}

impl Tracker {
    pub fn new<P: ToString, A: ToString>(parsec: P, action: A) -> Self {
        Self {
            parsec: parsec.to_string(),
            action: action.to_string(),
            level: Level::Info,
        }
    }
}

pub type Track = TrackDef<String>;
pub type TrackRegex = TrackDef<Regex>;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct TrackDef<R> {
    selector: Selector,
    stop: R,
    action: R,
}

impl TrackRegex {
    pub fn new<S: ToString>(selector: S, stop: S, action: S) -> Result<Self, SpaceErr> {
        let selector = Selector::from_str(selector.to_string().as_str())?;
        let stop = Regex::from_str(stop.to_string().as_str())?;
        let action = Regex::from_str(action.to_string().as_str())?;

        Ok(Self {
            selector,
            stop,
            action,
        })
    }
}

impl TrackDef<String> {
    pub fn new<S: ToString>(selector: S, stop: S, action: S) -> Result<Self, SpaceErr> {
        let selector = Selector::from_str(selector.to_string().as_str())?;
        Regex::from_str(stop.to_string().as_str())?;
        Regex::from_str(action.to_string().as_str())?;

        let stop = stop.to_string();
        let action = action.to_string();

        Ok(Self {
            selector,
            stop,
            action,
        })
    }

    pub fn to_regex(&self) -> Result<TrackRegex, SpaceErr> {
        Ok(TrackRegex {
            selector: self.selector.clone(),
            stop: Regex::from_str(self.stop.as_str())?,
            action: Regex::from_str(self.action.as_str())?,
        })
    }
}

pub struct FileAppender(tokio::sync::mpsc::Sender<Log>);

impl FileAppender {
    pub fn new<A>(writer: A) -> Self
    where
        A: Write + Sync + Send + 'static,
    {
        FileAppender(InnerFileAppender::new(writer))
    }
}

impl LogAppender for FileAppender {
    fn log(&self, log: Log) {
        let action = match &log.action {
            None => "None".to_string(),
            Some(action) => action.to_string(),
        };
        self.0.try_send(log).unwrap();
    }


    fn span_event(&self, log:  SpanEvent) {
        todo!();
    }

    fn pointless(&self, log: PointlessLog) {
        println!("{}", log.message);
    }
}

struct InnerFileAppender<F>
where
    F: Write,
{
    rx: tokio::sync::mpsc::Receiver<Log>,
    writer: F,
}

impl<F> InnerFileAppender<F>
where
    F: Write + Sync + Send + 'static,
{
    fn new(writer: F) -> tokio::sync::mpsc::Sender<Log> {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);

        let appender = Self { rx, writer };

        appender.start();

        tx
    }

    fn start(mut self) {
        tokio::spawn(async move {
            while let Some(log) = self.rx.recv().await {
                let point = match &log.point {
                    Some(point) => point.to_string(),
                    None => "None".to_string(),
                };
                let log = format!("{} | {}", point, log.payload.to_string());
                self.writer.write_all(log.as_bytes()).unwrap_or_default();
                self.writer.flush().unwrap_or_default();
            }
        });
    }
}

/*
struct TopicUpdate {
    name: String,
    state: LogTopicState,
}

impl TopicUpdate {
    fn new(name: String, state: LogTopicState) -> Self {
        Self { name, state }
    }
}

pub enum LogTopicKind {
    Operation,
}


pub trait AsLogMark {
    fn as_log_mark(&self) -> LogMark;
}

 */

macro_rules! log{
    ($($args: expr),*) => {

        LOGGER.try_with( |logger| {
        let package= env!("CARGO_PKG_NAME")
        print!("TRACE: file: {}, line: {}", file!(), line!());
            print!(", {}: {}", stringify!($args), $args);
        } );

        print!("TRACE: file: {}, line: {}", file!(), line!());
        $(
            print!(", {}: {}", stringify!($args), $args);
        )*
        println!(""); // to get a new line at the end
    }
}

#[derive(Debug, Clone, Builder, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct LogMark {
    pub package: String,
    pub file: String,
    pub line: String,
    #[builder(default)]
    pub object: Option<String>,
    #[builder(default)]
    pub function: Option<String>,
}
