use crate::error::MsgErr;
use crate::version::v0_0_1::id::id::{Point, ToPoint, Uuid};
use crate::version::v0_0_1::util::{timestamp, uuid};
use crate::version::v0_0_1::{mesh_portal_timestamp};
use serde_json::Value;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use serde::{Serialize,Deserialize};
use serde;
use crate::version::v0_0_1::command::command::common::StateSrc::Substance;
use chrono::serde::ts_milliseconds;

#[derive(Debug, Clone, Serialize, Deserialize, Eq,PartialEq,strum_macros::Display)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Eq , PartialEq)]
pub struct Log {
    pub point: Point,
    pub source: LogSource,
    pub span: Option<Uuid>,
    pub timestamp: i64,
    pub payload: LogPayload,
    pub level: Level,
}



impl ToString for Log {
    fn to_string(&self) -> String {
        format!(
            "{} {} {}",
            self.point.to_string(),
            self.level.to_string(),
            self.payload.to_string()
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, strum_macros::Display)]
pub enum LogSource {
   Shell,
   Core
}
#[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
pub enum LogSpanEventKind {
    Entry,
    Exit
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSpanEvent {
    pub span: Uuid,
    pub kind: LogSpanEventKind,
    pub attributes: HashMap<String,String>,

    #[serde(with= "ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
}

impl LogSpanEvent {
    pub fn new( span: &LogSpan, kind: LogSpanEventKind, attributes: HashMap<String,String> ) -> LogSpanEvent {
        LogSpanEvent {
            span: span.uuid.clone(),
            kind,
            attributes,
            timestamp: Utc::now()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSpan{
    pub uuid: Uuid,
    pub point: Point,
    pub parent: Option<String>,
    pub attributes: HashMap<String,String>,
    #[serde(with= "ts_milliseconds")]
    pub entry_timestamp: DateTime<Utc>,
}

impl LogSpan {
    pub fn new( point: Point ) -> Self {
        Self {
            uuid: uuid(),
            point,
            parent: None,
            attributes: Default::default(),
            entry_timestamp: Utc::now()
        }
    }

    pub fn parent( point: Point, parent: Uuid ) -> Self {
        Self {
            uuid: uuid(),
            point,
            parent: Some(parent),
            attributes: Default::default(),
            entry_timestamp: Utc::now()
        }
    }

    pub fn opt(point: Point, span: Option<Self>) -> Self {
        let mut span = span.unwrap_or(
        Self {
            uuid: uuid(),
            point: point.clone(),
            parent: None,
            attributes: Default::default(),
            entry_timestamp: Utc::now()
        });
        span.point = point;
        span
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointlessLog {
    #[serde(with= "ts_milliseconds")]
    timestamp: DateTime<Utc>,
    message: String,
    level: Level,
}

#[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq)]
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


pub struct RootLogBuilder
{
    pub point: Option<Point>,
    pub span: Option<String>,
    pub logger: RootLogger,
    pub level: Level,
    pub message: Option<String>,
    pub json: Option<Value>,
    msg_overrides: Vec<String>,
}

impl RootLogBuilder
{
    pub fn new(logger: RootLogger, span: Option<String>) -> Self {
        RootLogBuilder {
            logger,
            span,
            point: None,
            level: Level::default(),
            message: None,
            json: None,
            msg_overrides: vec![],
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

    pub fn json<'a,J>(mut self, json: J) -> Self
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
                level: Level::Error
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
                point,
                level: self.level,
                timestamp: timestamp().timestamp_millis(),
                payload: content,
                source: self.logger.source(),
                span: self.span
            };
        self.logger.log(log);
    }
}

pub trait LogAppender: Send+Sync{
    fn log(&self, log: Log);

    fn audit(&self, log: AuditLog);

    fn span_event(&self, log: LogSpanEvent);

    /// PointlessLog is used for error diagnosis of the logging system itself, particularly
    /// where there is parsing error due to a bad point
    fn pointless(&self, log: PointlessLog);
}

#[derive(Clone)]
pub struct RootLogger {
   source: LogSource,
   appender: Arc<dyn LogAppender>
}

impl RootLogger {

    pub fn new( source: LogSource, appender: Arc<dyn LogAppender>) -> Self {
        Self {
            source,
            appender
        }
    }

    pub fn stdout(source: LogSource) -> Self {
        Self{
            source,
            ..RootLogger::default()
        }
    }


    fn source(&self) -> LogSource {
        self.source.clone()
    }

    fn log(&self, log: Log) {
        self.appender.log(log);
    }

    fn audit(&self, log: AuditLog) {
        self.appender.audit(log);
    }

    fn span_event(&self, log: LogSpanEvent) {
        self.appender.span_event(log);
    }

    /// PointlessLog is used for error diagnosis of the logging system itself, particularly
    /// where there is parsing error due to a bad point
    fn pointless(&self, log: PointlessLog) {
        self.appender.pointless(log);
    }

    pub fn point<P:ToPoint>(&self, point: P) -> PointLogger {
        PointLogger {
            logger: self.clone(),
            point: point.to_point()
        }
    }
}

pub struct StdOutAppender();

impl LogAppender for StdOutAppender {
    fn log(&self, log: Log) {
        println!("{}",log.payload.to_string() )
    }

    fn audit(&self, log: AuditLog) {
        println!("audit log..." )
    }

    fn span_event(&self, log: LogSpanEvent) {
        println!("span..." )
    }

    fn pointless(&self, log: PointlessLog) {
        println!("{}",log.message  );
    }
}

impl Default for RootLogger {
    fn default() -> Self {
        Self {
            appender: Arc::new(StdOutAppender()),
            source: LogSource::Core
        }
    }
}

#[derive(Clone)]
pub struct PointLogger {
    pub logger: RootLogger,
    pub point: Point,
}

impl PointLogger {

    pub fn source(&self) -> LogSource {
        self.logger.source()
    }

    pub fn opt_span( &self, span: Option<LogSpan>) -> SpanLogger {
        let new = span.is_none();
        let span = LogSpan::opt(self.point.clone(), span );
        let logger = SpanLogger {
            root_logger: self.logger.clone(),
            span: span.clone(),
            commit_on_drop: true
        };

        if new {
            self.logger.span_event(LogSpanEvent::new(&span, LogSpanEventKind::Entry, Default::default()));
        }

        logger
    }

    pub fn for_span_async(&self, span: LogSpan) -> SpanLogger {
        let mut span = self.for_span(span);
        span.commit_on_drop = false;
        span
    }

    pub fn for_span(&self, span: LogSpan) -> SpanLogger {
        let mut span = SpanLogger {
            root_logger: self.logger.clone(),
            span,
            commit_on_drop: true
        };
        span
    }



    pub fn span(&self) -> SpanLogger {
        let span = LogSpan::new(self.point.clone());
        let logger = SpanLogger {
            root_logger: self.logger.clone(),
            span: span.clone(),
            commit_on_drop: true
        };

        self.logger.span_event(LogSpanEvent::new(&span, LogSpanEventKind::Entry, Default::default()));

        logger
    }

    pub fn span_async(&self) -> SpanLogger {
        let mut span = self.span();
        span.commit_on_drop = false;
        span
    }


    pub fn point(&self, point: Point) -> PointLogger {
        PointLogger {
            logger: self.logger.clone(),
            point
        }
    }

    pub fn push<S:ToString>(&self, point_segs: S ) -> Result<PointLogger,MsgErr> {
        Ok(PointLogger {
            logger: self.logger.clone(),
            point: self.point.push(point_segs.to_string())?
        })
    }

    pub fn msg<M>(&self, level: Level, message :M ) where M: ToString {
        self.logger.log(Log {
            point: self.point.clone(),
            level,
            timestamp: timestamp().timestamp_millis(),
            payload: LogPayload::Message(message.to_string()),
            span:  None,
            source: self.logger.source()
        })
    }


    pub fn trace<M>(&self, message: M)
        where
            M: ToString,
    {
        self.msg(Level::Trace,message);
    }

    pub fn debug<M>(&self, message: M) where M:ToString {
        self.msg(Level::Trace,message);
    }

    pub fn info<M>(&self, message: M) where M:ToString {
        self.msg(Level::Trace,message);
    }

    pub fn warn<M>(&self, message: M) where M:ToString {
        self.msg(Level::Warn, message );
    }

    pub fn error<M>(&self, message: M) where M:ToString {
        self.msg(Level::Error, message );
    }

}



pub struct SpanLogBuilder {
    pub entry_timestamp: DateTime<Utc>,
    pub attributes: HashMap<String,String>,
}

impl SpanLogBuilder {
    pub fn new() -> Self {
        Self {
            entry_timestamp: timestamp(),
            attributes: HashMap::new()
        }
    }
}

#[derive(Clone)]
pub struct SpanLogger {
    root_logger: RootLogger,
    span: LogSpan,
    commit_on_drop: bool
}

impl SpanLogger {
    pub fn span_uuid(&self) -> String {
        self.span.uuid.clone()
    }

    pub fn point(&self) -> &Point {
        &self.span.point
    }

    pub fn span(&self) -> SpanLogger {
        let span = LogSpan::new(self.point().clone() );
        SpanLogger {
            root_logger: self.root_logger.clone(),
            span,
            commit_on_drop: true
        }
    }

    pub fn span_attr(&self, attr: HashMap<String,String>) -> SpanLogger {
        let mut span = LogSpan::new(self.point().clone() );
        span.attributes = attr;
        SpanLogger {
            root_logger: self.root_logger.clone(),
            span,
            commit_on_drop: true
        }
    }


    pub fn span_async(&self) -> SpanLogger {
        let mut span = self.span();
        span.commit_on_drop = false;
        span
    }


    pub fn current_span(&self) -> &LogSpan{
        &self.span
    }

    pub fn entry_timestamp(&self) -> DateTime<Utc>{
        self.span.entry_timestamp.clone()
    }

    pub fn set_span_attr<K,V>( &mut self, key: K, value: V) where K: ToString, V: ToString {
        self.span.attributes.insert( key.to_string(), value.to_string() );
    }

    pub fn get_span_attr<K>( &self, key: K) -> Option<String> where K: ToString {
        self.span.attributes.get( &key.to_string() ).cloned()
    }

    pub fn msg<M>(&self, level: Level, message :M ) where M: ToString {
        self.root_logger.log(Log {
            point: self.point().clone(),
            level,
            timestamp: timestamp().timestamp_millis(),
            payload: LogPayload::Message(message.to_string()),
            span:  Some(self.span_uuid()),
            source: self.root_logger.source()
        })
    }

    pub fn trace<M>(&self, message: M)
        where
            M: ToString,
    {
        self.msg(Level::Trace,message);
    }

    pub fn debug<M>(&self, message: M) where M:ToString {
        self.msg(Level::Trace,message);
    }

    pub fn info<M>(&self, message: M) where M:ToString {
        self.msg(Level::Trace,message);
    }

    pub fn warn<M>(&self, message: M) where M:ToString {
        self.msg(Level::Warn, message );
    }

    pub fn error<M>(&self, message: M) where M:ToString {
        self.msg(Level::Error, message );
    }


    pub fn audit(&self) -> AuditLogBuilder {
        AuditLogBuilder {
            logger: self.root_logger.clone(),
            point: self.point().clone(),
            span: self.span.uuid.clone(),
            attributes: HashMap::new(),
        }
    }

    pub fn builder(&self) -> LogBuilder {
        let builder = RootLogBuilder::new( self.root_logger.clone(), None);
        let builder = LogBuilder::new(self.root_logger.clone(), builder);
        builder
    }

    pub fn log_audit(&self, log: AuditLog) {
        self.root_logger.audit(log);
    }
}

impl Drop for SpanLogger {
    fn drop(&mut self) {
        if self.commit_on_drop {
            let log = LogSpanEvent::new(&self.span, LogSpanEventKind::Exit, self.span.attributes.clone());
            self.root_logger.span_event(log)
        }
    }
}



pub struct LogBuilder
{
    logger: RootLogger,
    builder: RootLogBuilder,
}

impl LogBuilder
{
    pub fn new(logger: RootLogger, builder: RootLogBuilder) -> Self {
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

    pub fn json<'a,J>(mut self, json: J) -> Self
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

pub struct AuditLogBuilder {
    logger: RootLogger,
    point: Point,
    span: String,
    attributes: HashMap<String, String>,
}

impl AuditLogBuilder {
    pub fn new(logger: RootLogger, point: Point, span: String) -> Self {
        AuditLogBuilder {
            logger,
            point,
            attributes: HashMap::new(),
            span
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLog {
    pub point: Point,
    #[serde(with= "ts_milliseconds")]
    pub timestamp: DateTime<Utc>,
    pub metrics: HashMap<String, String>,
}
