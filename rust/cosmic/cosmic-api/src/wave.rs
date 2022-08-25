use crate::bin::Bin;
use crate::cli::RawCommand;
use crate::command::Command;
use crate::config::config::bind::RouteSelector;
use crate::error::{MsgErr, StatusErr};
use crate::http::HttpMethod;
use crate::id::id::{
    Layer, Point, PointSeg, Port, PortSelector, RouteSeg, Sub, ToPoint, ToPort, Topic, Uuid,
};
use crate::id::StarKey;
use crate::log::{
    LogSpan, LogSpanEvent, PointLogger, RootLogger, SpanLogger, Spannable, Trackable, TrailSpanId,
};
use crate::msg::MsgMethod;
use crate::parse::model::Subst;
use crate::parse::sub;
use crate::particle::particle::{Details, Status};
use crate::particle::Watch;
use crate::quota::Timeouts;
use crate::security::{Permissions, Privilege, Privileges};
use crate::selector::selector::Selector;
use crate::substance::substance::{
    Call, CallKind, CmdCall, Errors, HttpCall, MsgCall, MultipartFormBuilder, SubstanceKind,
    SysCall, ToRequestCore, Token,
};
use crate::substance::substance::{Substance, ToSubstance};
use crate::sys::AssignmentKind;
use crate::util::{uuid, ValueMatcher, ValuePattern};
use crate::{ANONYMOUS, HYPERUSER};
use alloc::borrow::Cow;
use core::borrow::Borrow;
use cosmic_macros_primitive::Autobox;
use cosmic_nom::{Res, SpanExtra};
use dashmap::DashMap;
use http::{HeaderMap, StatusCode, Uri};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::env::var;
use std::marker::PhantomData;
use std::ops;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, RwLock};
use tokio::time::Instant;

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum WaveKind {
    Ping,   // Request
    Pong,   // Response
    Ripple, // Broadcast
    Echo,   // Broadcast Response
    Signal, // Notification
            /*
            Photon, // Ack
                   Reverb,  // Ack
                  */
}

impl WaveKind {
    pub fn reflected_kind(&self) -> Result<ReflectedKind, MsgErr> {
        match self {
            WaveKind::Pong => Ok(ReflectedKind::Pong),
            WaveKind::Echo => Ok(ReflectedKind::Echo),
            _ => Err(MsgErr::not_found()),
        }
    }
}

pub type UltraWave = UltraWaveDef<Recipients>;
pub type SingularUltraWave = UltraWaveDef<Port>;

impl SingularUltraWave {
    pub fn to_ultra(self) -> Result<UltraWave, MsgErr> {
        match self {
            SingularUltraWave::Ping(ping) => Ok(UltraWave::Ping(ping)),
            SingularUltraWave::Pong(pong) => Ok(UltraWave::Pong(pong)),
            SingularUltraWave::Echo(echo) => Ok(UltraWave::Echo(echo)),
            SingularUltraWave::Signal(signal) => Ok(UltraWave::Signal(signal)),
            SingularUltraWave::Ripple(ripple) => {
                let ripple = ripple.to_multiple();
                Ok(UltraWave::Ripple(ripple))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum UltraWaveDef<T>
where
    T: ToRecipients + Clone,
{
    Ping(Wave<Ping>),
    Pong(Wave<Pong>),
    Ripple(Wave<RippleDef<T>>),
    Echo(Wave<Echo>),
    Signal(Wave<Signal>),
}

impl<W> Spannable for UltraWaveDef<W>
where
    W: ToRecipients + Clone,
{
    fn span_id(&self) -> String {
        self.id().to_string()
    }

    fn span_type(&self) -> &'static str {
        "Wave"
    }
}

impl Trackable for UltraWave {
    fn track_id(&self) -> String {
        self.id().to_short_string()
    }

    fn track_method(&self) -> String {
        match self {
            UltraWave::Ping(ping) => ping.core.method.to_deep_string(),
            UltraWave::Pong(pong) => pong.core.status.to_string(),
            UltraWave::Ripple(ripple) => ripple.core.method.to_deep_string(),
            UltraWave::Echo(echo) => echo.core.status.to_string(),
            UltraWave::Signal(signal) => signal.core.method.to_deep_string(),
        }
    }

    fn track_payload(&self) -> String {
        match self {
            UltraWave::Ping(ping) => ping.core.body.kind().to_string(),
            UltraWave::Pong(pong) => pong.core.body.kind().to_string(),
            UltraWave::Ripple(ripple) => ripple.core.body.kind().to_string(),
            UltraWave::Echo(echo) => echo.core.body.kind().to_string(),
            UltraWave::Signal(signal) => signal.core.body.kind().to_string(),
        }
    }

    fn track_from(&self) -> String {
        self.from().to_string()
    }

    fn track_to(&self) -> String {
        self.to().to_string()
    }

    fn track(&self) -> bool {
        match self {
            UltraWave::Ping(ping) => ping.track,
            UltraWave::Pong(pong) => pong.track,
            UltraWave::Ripple(ripple) => ripple.track,
            UltraWave::Echo(echo) => echo.track,
            UltraWave::Signal(signal) => signal.track,
        }
    }
    fn track_payload_fmt(&self) -> String {
        match self {
            UltraWave::Signal(signal) => signal.track_payload_fmt(),
            UltraWave::Ping(_) => self.track_payload(),
            UltraWave::Pong(_) => self.track_payload(),
            UltraWave::Ripple(_) => self.track_payload(),
            UltraWave::Echo(_) => self.track_payload(),
        }
    }
}

impl<T> UltraWaveDef<T>
where
    T: ToRecipients + Clone,
{
    pub fn has_visited(&self, star: &Point) -> bool {
        match self {
            UltraWaveDef::Ripple(ripple) => ripple.history.contains(star),
            _ => false,
        }
    }

    pub fn id(&self) -> WaveId {
        match self {
            UltraWaveDef::Ping(w) => w.id.clone(),
            UltraWaveDef::Pong(w) => w.id.clone(),
            UltraWaveDef::Ripple(w) => w.id.clone(),
            UltraWaveDef::Echo(w) => w.id.clone(),
            UltraWaveDef::Signal(w) => w.id.clone(),
        }
    }
}

impl UltraWave {
    pub fn can_shard(&self) -> bool {
        match self {
            UltraWave::Ripple(_) => true,
            _ => false,
        }
    }

    pub fn to_singular(self) -> Result<SingularUltraWave, MsgErr> {
        match self {
            UltraWave::Ping(ping) => Ok(SingularUltraWave::Ping(ping)),
            UltraWave::Pong(pong) => Ok(SingularUltraWave::Pong(pong)),
            UltraWave::Echo(echo) => Ok(SingularUltraWave::Echo(echo)),
            UltraWave::Signal(signal) => Ok(SingularUltraWave::Signal(signal)),
            UltraWave::Ripple(_) => Err(MsgErr::from_500("cannot change Ripple into a singular")),
        }
    }

    pub fn wrap_in_transport(self, from: Port, to: Port) -> DirectedProto {
        let mut signal = DirectedProto::ping();
        signal.kind(DirectedKind::Signal);
        signal.fill(&self);
        signal.from(from);
        signal.agent(self.agent().clone());
        signal.handling(self.handling().clone());
        signal.method(SysMethod::Transport);
        signal.track = self.track();
        signal.body(Substance::UltraWave(Box::new(self)));
        signal.to(to);
        signal
    }

    pub fn unwrap_from_hop(self) -> Result<Wave<Signal>, MsgErr> {
        let signal = self.to_signal()?;
        signal.unwrap_from_hop()
    }

    pub fn unwrap_from_transport(self) -> Result<UltraWave, MsgErr> {
        let signal = self.to_signal()?;
        signal.unwrap_from_transport()
    }

    pub fn to_substance(self) -> Substance {
        Substance::UltraWave(Box::new(self))
    }

    pub fn to_directed(self) -> Result<DirectedWave, MsgErr> {
        match self {
            UltraWave::Ping(ping) => Ok(ping.to_directed()),
            UltraWave::Ripple(ripple) => Ok(ripple.to_directed()),
            UltraWave::Signal(signal) => Ok(signal.to_directed()),
            _ => Err(MsgErr::bad_request()),
        }
    }

    pub fn to_reflected(self) -> Result<ReflectedWave, MsgErr> {
        match self {
            UltraWave::Pong(pong) => Ok(pong.to_reflected()),
            UltraWave::Echo(echo) => Ok(echo.to_reflected()),
            _ => Err(MsgErr::bad_request_msg(format!("expected: ReflectedWave; encountered: {}", self.desc() ))),
        }
    }

    pub fn kind(&self) -> WaveKind {
        match self {
            UltraWave::Ping(_) => WaveKind::Ping,
            UltraWave::Pong(_) => WaveKind::Pong,
            UltraWave::Ripple(_) => WaveKind::Ripple,
            UltraWave::Echo(_) => WaveKind::Echo,
            UltraWave::Signal(_) => WaveKind::Signal
        }
    }

    /// return a description of this wave for debugging purposes
    pub fn desc(&self) -> String {
        if self.is_directed() {
            let directed = self.clone().to_directed().unwrap();
            format!("{}<{}>[{}]",self.kind().to_string(), directed.core().method.to_string(), directed.core().body.kind().to_string() )
        } else {
            let reflected = self.clone().to_reflected().unwrap();
            format!("{}<{}>[{}]",self.kind().to_string(), reflected.core().status.to_string(), reflected.core().body.kind().to_string() )
        }
    }

    pub fn hops(&self) -> u16 {
        match self {
            UltraWave::Ping(w) => w.hops,
            UltraWave::Pong(w) => w.hops,
            UltraWave::Ripple(w) => w.hops,
            UltraWave::Echo(w) => w.hops,
            UltraWave::Signal(w) => w.hops,
        }
    }

    pub fn inc_hops(&mut self) {
        match self {
            UltraWave::Ping(w) => w.hops += 1,
            UltraWave::Pong(w) => w.hops += 1,
            UltraWave::Ripple(w) => w.hops += 1,
            UltraWave::Echo(w) => w.hops += 1,
            UltraWave::Signal(w) => w.hops += 1,
        };
    }

    pub fn add_to_history(&mut self, star: Point) {
        match self {
            UltraWave::Ripple(ripple) => {
                ripple.history.insert(star);
            }
            _ => {}
        }
    }

    pub fn to_signal(self) -> Result<Wave<Signal>, MsgErr> {
        match self {
            UltraWave::Signal(signal) => Ok(signal),
            _ => Err(MsgErr::bad_request_msg(format!("expecting: Wave<Signal> encountered: Wave<{}>", self.kind().to_string()))),
        }
    }

    pub fn method(&self) -> Option<&Method> {
        match self {
            UltraWave::Ping(ping) => Some(&ping.method),
            UltraWave::Ripple(ripple) => Some(&ripple.method),
            UltraWave::Signal(signal) => Some(&signal.method),
            _ => None,
        }
    }

    pub fn is_directed(&self) -> bool {
        match self {
            UltraWave::Ping(_) => true,
            UltraWave::Pong(_) => false,
            UltraWave::Ripple(_) => true,
            UltraWave::Echo(_) => false,
            UltraWave::Signal(_) => true,
        }
    }

    pub fn is_reflected(&self) -> bool {
        match self {
            UltraWave::Ping(_) => false,
            UltraWave::Pong(_) => true,
            UltraWave::Ripple(_) => false,
            UltraWave::Echo(_) => true,
            UltraWave::Signal(_) => false,
        }
    }

    pub fn to(&self) -> Recipients {
        match self {
            UltraWave::Ping(ping) => ping.to.clone().to_recipients(),
            UltraWave::Pong(pong) => pong.to.clone().to_recipients(),
            UltraWave::Ripple(ripple) => ripple.to.clone(),
            UltraWave::Echo(echo) => echo.to.clone().to_recipients(),
            UltraWave::Signal(signal) => signal.to.clone().to_recipients(),
        }
    }

    pub fn from(&self) -> &Port {
        match self {
            UltraWave::Ping(ping) => &ping.from,
            UltraWave::Pong(pong) => &pong.from,
            UltraWave::Ripple(ripple) => &ripple.from,
            UltraWave::Echo(echo) => &echo.from,
            UltraWave::Signal(signal) => &signal.from,
        }
    }

    pub fn set_agent(&mut self, agent: Agent) {
        match self {
            UltraWave::Ping(ping) => ping.agent = agent,
            UltraWave::Pong(pong) => pong.agent = agent,
            UltraWave::Ripple(ripple) => ripple.agent = agent,
            UltraWave::Echo(echo) => echo.agent = agent,
            UltraWave::Signal(signal) => signal.agent = agent,
        }
    }

    pub fn set_to(&mut self, to: Port) {
        match self {
            UltraWave::Ping(ping) => ping.to = to,
            UltraWave::Pong(pong) => pong.to = to,
            UltraWave::Ripple(ripple) => ripple.to = to.to_recipients(),
            UltraWave::Echo(echo) => echo.to = to,
            UltraWave::Signal(signal) => signal.to = to,
        }
    }

    pub fn set_from(&mut self, from: Port) {
        match self {
            UltraWave::Ping(ping) => ping.from = from,
            UltraWave::Pong(pong) => pong.from = from,
            UltraWave::Ripple(ripple) => ripple.from = from,
            UltraWave::Echo(echo) => echo.from = from,
            UltraWave::Signal(signal) => signal.from = from,
        }
    }

    pub fn agent(&self) -> &Agent {
        match self {
            UltraWave::Ping(ping) => &ping.agent,
            UltraWave::Pong(pong) => &pong.agent,
            UltraWave::Ripple(ripple) => &ripple.agent,
            UltraWave::Echo(echo) => &echo.agent,
            UltraWave::Signal(signal) => &signal.agent,
        }
    }

    pub fn handling(&self) -> &Handling {
        match self {
            UltraWave::Ping(ping) => &ping.handling,
            UltraWave::Pong(pong) => &pong.handling,
            UltraWave::Ripple(ripple) => &ripple.handling,
            UltraWave::Echo(echo) => &echo.handling,
            UltraWave::Signal(signal) => &signal.handling,
        }
    }

    pub fn track(&self) -> bool {
        match self {
            UltraWave::Ping(ping) => ping.track,
            UltraWave::Pong(pong) => pong.track,
            UltraWave::Ripple(ripple) => ripple.track,
            UltraWave::Echo(echo) => echo.track,
            UltraWave::Signal(signal) => signal.track,
        }
    }

    pub fn set_track(&mut self, track: bool) {
        match self {
            UltraWave::Ping(ping) => ping.track = track,
            UltraWave::Pong(pong) => pong.track = track,
            UltraWave::Ripple(ripple) => ripple.track = track,
            UltraWave::Echo(echo) => echo.track = track,
            UltraWave::Signal(signal) => signal.track = track,
        }
    }

    pub fn scope(&self) -> &Scope {
        match self {
            UltraWave::Ping(ping) => &ping.scope,
            UltraWave::Pong(pong) => &pong.scope,
            UltraWave::Ripple(ripple) => &ripple.scope,
            UltraWave::Echo(echo) => &echo.scope,
            UltraWave::Signal(signal) => &signal.scope,
        }
    }
    pub fn to_ripple(self) -> Result<Wave<Ripple>, MsgErr> {
        match self {
            UltraWave::Ripple(ripple) => Ok(ripple),
            _ => Err("not a ripple".into()),
        }
    }
}

impl<S> ToSubstance<S> for UltraWave
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        match self {
            UltraWave::Ping(ping) => ping.to_substance(),
            UltraWave::Pong(pong) => pong.to_substance(),
            UltraWave::Ripple(ripple) => ripple.to_substance(),
            UltraWave::Echo(echo) => echo.to_substance(),
            UltraWave::Signal(signal) => signal.to_substance(),
        }
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        match self {
            UltraWave::Ping(ping) => ping.to_substance_ref(),
            UltraWave::Pong(pong) => pong.to_substance_ref(),
            UltraWave::Ripple(ripple) => ripple.to_substance_ref(),
            UltraWave::Echo(echo) => echo.to_substance_ref(),
            UltraWave::Signal(signal) => signal.to_substance_ref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct WaveId {
    uuid: Uuid,
    kind: WaveKind,
}

impl WaveId {
    pub fn new(kind: WaveKind) -> Self {
        let uuid = uuid();
        Self::with_uuid(kind, uuid)
    }

    pub fn with_uuid(kind: WaveKind, uuid: Uuid) -> Self {
        Self { uuid, kind }
    }

    pub fn to_short_string(&self) -> String {
        format!(
            "<Wave<{}>>::{}",
            self.kind.to_string(),
            self.uuid[..8].to_string()
        )
    }
}

impl ToString for WaveId {
    fn to_string(&self) -> String {
        format!("<Wave<{}>>::{}", self.kind.to_string(), self.uuid)
    }
}

pub struct RootInCtx {
    pub to: Port,
    pub wave: DirectedWave,
    pub session: Option<Session>,
    pub logger: SpanLogger,
    pub transmitter: ProtoTransmitter,
}

impl RootInCtx {
    pub fn new(
        wave: DirectedWave,
        to: Port,
        logger: SpanLogger,
        transmitter: ProtoTransmitter,
    ) -> Self {
        Self {
            wave,
            to,
            logger,
            session: None,
            transmitter: transmitter,
        }
    }

    pub fn status(self, status: u16, from: Port) -> Bounce<ReflectedWave> {
        match self.wave {
            DirectedWave::Ping(ping) => Bounce::Reflected(ReflectedWave::Pong(Wave::new(
                Pong::new(
                    ReflectedCore::status(status),
                    ping.from.clone(),
                    self.to.clone().to_recipients(),
                    ping.id.clone(),
                ),
                from,
            ))),
            DirectedWave::Ripple(ripple) => Bounce::Reflected(ReflectedWave::Echo(Wave::new(
                Echo::new(
                    ReflectedCore::status(status),
                    ripple.from.clone(),
                    ripple.to.clone(),
                    ripple.id.clone(),
                ),
                from,
            ))),
            DirectedWave::Signal(_) => Bounce::Absorbed,
        }
    }

    pub fn not_found(self) -> Bounce<ReflectedWave> {
        let to = self.to.clone();
        self.status(404, to)
    }

    pub fn timeout(self) -> Bounce<ReflectedWave> {
        let to = self.to.clone();
        self.status(408, to)
    }

    pub fn bad_request(self) -> Bounce<ReflectedWave> {
        let to = self.to.clone();
        self.status(400, to)
    }

    pub fn server_error(self) -> Bounce<ReflectedWave> {
        let to = self.to.clone();
        self.status(500, to)
    }

    pub fn forbidden(self) -> Bounce<ReflectedWave> {
        let to = self.to.clone();
        self.status(401, to)
    }

    pub fn unavailable(self) -> Bounce<ReflectedWave> {
        let to = self.to.clone();
        self.status(503, to)
    }

    pub fn unauthorized(self) -> Bounce<ReflectedWave> {
        let to = self.to.clone();
        self.status(403, to)
    }
}

impl RootInCtx {
    pub fn push<'a, I>(&self) -> Result<InCtx<I>, MsgErr>
    where
        Substance: ToSubstance<I>,
    {
        let input = match self.wave.to_substance_ref() {
            Ok(input) => input,
            Err(err) => return Err(err.into()),
        };
        Ok(InCtx {
            root: self,
            input,
            logger: self.logger.clone(),
            transmitter: Cow::Borrowed(&self.transmitter),
        })
    }
}

pub struct InCtx<'a, I> {
    root: &'a RootInCtx,
    pub transmitter: Cow<'a, ProtoTransmitter>,
    pub input: &'a I,
    pub logger: SpanLogger,
}

impl<'a, I> Deref for InCtx<'a, I> {
    type Target = I;

    fn deref(&self) -> &Self::Target {
        self.input
    }
}

impl<'a, I> InCtx<'a, I> {
    pub fn new(
        root: &'a RootInCtx,
        input: &'a I,
        tx: Cow<'a, ProtoTransmitter>,
        logger: SpanLogger,
    ) -> Self {
        Self {
            root,
            input,
            logger,
            transmitter: tx,
        }
    }

    pub fn from(&self) -> &Port {
        self.root.wave.from()
    }

    pub fn to(&self) -> &Port {
        &self.root.to
    }

    pub fn push(self) -> InCtx<'a, I> {
        InCtx {
            root: self.root,
            input: self.input,
            logger: self.logger.span(),
            transmitter: self.transmitter.clone(),
        }
    }

    pub fn push_from(self, from: Port) -> InCtx<'a, I> {
        let mut transmitter = self.transmitter.clone();
        transmitter.to_mut().from = SetStrategy::Override(from);
        InCtx {
            root: self.root,
            input: self.input,
            logger: self.logger.clone(),
            transmitter,
        }
    }

    pub fn push_input_ref<I2>(self, input: &'a I2) -> InCtx<'a, I2> {
        InCtx {
            root: self.root,
            input,
            logger: self.logger.clone(),
            transmitter: self.transmitter.clone(),
        }
    }

    pub fn wave(&self) -> &DirectedWave {
        &self.root.wave
    }

    pub async fn ping(&self, req: DirectedProto) -> Result<Wave<Pong>, MsgErr> {
        self.transmitter.direct(req).await
    }
}

impl<'a, I> InCtx<'a, I> {
    pub fn ok_body(self, substance: Substance) -> ReflectedCore {
        self.root.wave.core().ok_body(substance)
    }

    pub fn not_found(self) -> ReflectedCore {
        self.root.wave.core().not_found()
    }

    pub fn forbidden(self) -> ReflectedCore {
        self.root.wave.core().forbidden()
    }

    pub fn bad_request(self) -> ReflectedCore {
        self.root.wave.core().bad_request()
    }

    pub fn err(self, err: MsgErr) -> ReflectedCore {
        self.root.wave.core().err(err)
    }
}

pub trait Reflectable<R> {
    fn forbidden(self, responder: Port) -> R
    where
        Self: Sized,
    {
        self.status(403, responder)
    }

    fn bad_request(self, responder: Port) -> R
    where
        Self: Sized,
    {
        self.status(400, responder)
    }

    fn not_found(self, responder: Port) -> R
    where
        Self: Sized,
    {
        self.status(404, responder)
    }

    fn timeout(self, responder: Port) -> R
    where
        Self: Sized,
    {
        self.status(408, responder)
    }

    fn server_error(self, responder: Port) -> R
    where
        Self: Sized,
    {
        self.status(500, responder)
    }

    fn status(self, status: u16, responder: Port) -> R
    where
        Self: Sized;

    fn fail<M: ToString>(self, status: u16, message: M, responder: Port) -> R
    where
        Self: Sized;

    fn err(self, err: MsgErr, responder: Port) -> R
    where
        Self: Sized;

    fn ok(self, responder: Port) -> R
    where
        Self: Sized,
    {
        self.status(200, responder)
    }

    fn ok_body(self, body: Substance, responder: Port) -> R
    where
        Self: Sized;

    fn core(self, core: ReflectedCore, responder: Port) -> R
    where
        Self: Sized;

    fn result<C: Into<ReflectedCore>>(self, result: Result<C, MsgErr>, responder: Port) -> R
    where
        Self: Sized,
    {
        match result {
            Ok(core) => self.core(core.into(), responder),
            Err(err) => self.core(err.into(), responder),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectWaveStub {
    pub id: WaveId,
    pub agent: Agent,
    pub handling: Handling,
    pub from: Port,
    pub to: Recipients,
    pub span: Option<TrailSpanId>,
}

impl Into<WaitTime> for &DirectWaveStub {
    fn into(self) -> WaitTime {
        self.handling.wait.clone()
    }
}

pub type Ripple = RippleDef<Recipients>;
pub type SingularRipple = RippleDef<Port>;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct RippleDef<T: ToRecipients + Clone> {
    pub to: T,
    pub core: DirectedCore,
    pub bounce_backs: BounceBacks,
    pub history: HashSet<Point>,
}

impl Ripple {
    pub fn new<T>( core: DirectedCore, to: T,bounce_backs: BounceBacks) -> Self where T: ToRecipients {
        Self { to: to.to_recipients(), core, bounce_backs, history: HashSet::new() }
    }
}

impl<T> RippleDef<T>
where
    T: ToRecipients + Clone,
{
    pub fn replace_to<T2: ToRecipients + Clone>(self, to: T2) -> RippleDef<T2> {
        RippleDef {
            to,
            core: self.core,
            bounce_backs: self.bounce_backs,
            history: self.history,
        }
    }
}

impl Wave<SingularRipple> {
    pub fn to_singular_ultra(self) -> SingularUltraWave {
        SingularUltraWave::Ripple(self)
    }

    pub fn to_multiple(self) -> Wave<Ripple> {
        let ripple = self
            .variant
            .clone()
            .replace_to(self.variant.to.clone().to_recipients());
        self.replace(ripple)
    }
}

impl Wave<SingularRipple> {
    pub fn as_multi(&self, recipients: Recipients) -> Wave<Ripple> {
        let ripple = self.variant.clone().replace_to(recipients);
        self.clone().replace(ripple)
    }
}

impl Wave<Ripple> {
    pub fn as_single(&self, port: Port) -> Wave<SingularRipple> {
        let ripple = self.variant.clone().replace_to(port);
        self.clone().replace(ripple)
    }

    pub fn to_singular_directed(self) -> Result<SingularDirectedWave, MsgErr> {
        let to = self.to.clone().to_single()?;
        Ok(self.as_single(to).to_singular_directed())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum BounceBacks {
    None,
    Single,
    Count(usize),
    Timer(WaitTime),
}

impl<S, T> ToSubstance<S> for RippleDef<T>
where
    Substance: ToSubstance<S>,
    T: ToRecipients + Clone,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.core.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.core.to_substance_ref()
    }
}

impl<T> RippleDef<T>
where
    T: ToRecipients + Clone,
{
    pub fn require_method<M: Into<Method> + ToString + Clone>(
        self,
        method: M,
    ) -> Result<RippleDef<T>, MsgErr> {
        if self.core.method == method.clone().into() {
            Ok(self)
        } else {
            Err(MsgErr::new(
                400,
                format!("Bad Request: expecting method: {}", method.to_string()).as_str(),
            ))
        }
    }

    pub fn require_body<B>(self) -> Result<B, MsgErr>
    where
        B: TryFrom<Substance, Error = MsgErr>,
    {
        match B::try_from(self.body.clone()) {
            Ok(body) => Ok(body),
            Err(err) => Err(MsgErr::bad_request()),
        }
    }
}

impl<T> Deref for RippleDef<T>
where
    T: ToRecipients + Clone,
{
    type Target = DirectedCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl<T> DerefMut for RippleDef<T>
where
    T: ToRecipients + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Signal {
    pub to: Port,
    pub core: DirectedCore,
}

impl WaveVariant for Signal {
    fn kind(&self) -> WaveKind {
        WaveKind::Signal
    }
}

impl Signal {
    pub fn new(to: Port, core: DirectedCore) -> Self {
        Self { to, core }
    }

    pub fn bounce_backs(&self) -> BounceBacks {
        BounceBacks::None
    }
}

impl Deref for Signal {
    type Target = DirectedCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for Signal {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Ping {
    pub to: Port,
    pub core: DirectedCore,
}





impl Wave<Ping> {
    pub fn to_singular_directed(self) -> SingularDirectedWave {
        SingularDirectedWave::Ping(self)
    }

    pub fn with_core(mut self, core: DirectedCore ) -> Self {
        self.variant.core = core;
        self
    }
}

impl<S> ToSubstance<S> for Ping
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.core.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.core.to_substance_ref()
    }
}

impl Deref for Ping {
    type Target = DirectedCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for Ping {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.core
    }
}

impl Into<DirectedProto> for Wave<Ping> {
    fn into(self) -> DirectedProto {
        DirectedProto {
            to: Some(self.to.clone().to_recipients()),
            core: DirectedCore::default(),
            id: self.id,
            from: Some(self.from),
            handling: Some(self.handling),
            scope: Some(self.scope),
            agent: Some(self.agent),
            kind: None,
            bounce_backs: None,
            track: self.track,
        }
    }
}

impl Ping {
    pub fn require_method<M: Into<Method> + ToString + Clone>(
        self,
        method: M,
    ) -> Result<Ping, MsgErr> {
        if self.core.method == method.clone().into() {
            Ok(self)
        } else {
            Err(MsgErr::new(
                400,
                format!("Bad Request: expecting method: {}", method.to_string()).as_str(),
            ))
        }
    }

    pub fn require_body<B>(self) -> Result<B, MsgErr>
    where
        B: TryFrom<Substance, Error = MsgErr>,
    {
        match B::try_from(self.clone().core.body) {
            Ok(body) => Ok(body),
            Err(err) => Err(MsgErr::bad_request()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct WaveXtra<V> {
    pub wave: Wave<V>,
    pub session: Session,
}

impl<V> WaveXtra<V> {
    pub fn new(wave: Wave<V>, session: Session) -> Self {
        Self { wave, session }
    }
}

impl TryFrom<Ping> for RawCommand {
    type Error = MsgErr;

    fn try_from(request: Ping) -> Result<Self, Self::Error> {
        request.core.body.try_into()
    }
}

impl TryFrom<Pong> for Substance {
    type Error = MsgErr;

    fn try_from(response: Pong) -> Result<Self, Self::Error> {
        Ok(response.core.body)
    }
}

impl TryInto<Bin> for Pong {
    type Error = MsgErr;

    fn try_into(self) -> Result<Bin, Self::Error> {
        match self.core.body {
            Substance::Bin(bin) => Ok(bin),
            _ => Err(MsgErr::err400()),
        }
    }
}

impl Into<DirectedCore> for RawCommand {
    fn into(self) -> DirectedCore {
        DirectedCore::substance(
            MsgMethod::new("ExecCommand").unwrap().into(),
            Substance::RawCommand(self),
        )
    }
}

impl Ping {
    pub fn new<P: ToPort>(core: DirectedCore, to: P) -> Self {
        Self {
            to: to.to_port(),
            core,
        }
    }
}

#[derive(Clone)]
pub enum ReflectedKind {
    Pong,
    Echo,
}

#[derive(Clone)]
pub enum DirectedKind {
    Ping,
    Ripple,
    Signal,
}

#[derive(Clone)]
pub struct ReflectedProto {
    pub id: WaveId,
    pub intended: Option<Recipients>,
    pub from: Option<Port>,
    pub to: Option<Port>,
    pub body: Option<Substance>,
    pub status: Option<StatusCode>,
    pub handling: Option<Handling>,
    pub scope: Option<Scope>,
    pub agent: Option<Agent>,
    pub reflection_of: Option<WaveId>,
    pub kind: Option<ReflectedKind>,
    pub track: bool,
}

impl ReflectedProto {
    pub fn new() -> Self {
        Self {
            id: WaveId::new(WaveKind::Echo),
            intended: None,
            from: None,
            to: None,
            body: None,
            status: None,
            handling: None,
            scope: None,
            agent: None,
            reflection_of: None,
            kind: None,
            track: false,
        }
    }

    pub fn kind(&mut self, kind: ReflectedKind) {
        self.kind.replace(kind);
    }

    pub fn fill<V>(&mut self, wave: &Wave<V>) {
        self.reflection_of = Some(wave.id.clone());
        self.fill_to(&wave.from);
        self.fill_handling(&wave.handling);
        self.fill_scope(&wave.scope);
        self.fill_agent(&wave.agent);
        self.reflection_of = Some(wave.id.clone());
    }

    pub fn fill_kind(&mut self, kind: ReflectedKind) {
        if self.kind.is_none() {
            self.kind.replace(kind);
        }
    }

    pub fn fill_intended<I: ToRecipients + Clone>(&mut self, intended: I) {
        if self.intended.is_none() {
            self.intended.replace(intended.to_recipients());
        }
    }

    pub fn fill_to(&mut self, to: &Port) {
        if self.to.is_none() {
            self.to.replace(to.clone());
        }
    }

    pub fn fill_from(&mut self, from: &Port) {
        if self.from.is_none() {
            self.from.replace(from.clone());
        }
    }

    pub fn fill_scope(&mut self, scope: &Scope) {
        if self.scope.is_none() {
            self.scope.replace(scope.clone());
        }
    }

    pub fn fill_agent(&mut self, agent: &Agent) {
        if self.agent.is_none() {
            self.agent.replace(agent.clone());
        }
    }

    pub fn fill_handling(&mut self, handling: &Handling) {
        if self.handling.is_none() {
            self.handling.replace(handling.clone());
        }
    }

    pub fn fill_status(&mut self, status: &StatusCode) {
        if self.status.is_none() {
            self.status.replace(status.clone());
        }
    }

    pub fn body(&mut self, body: Substance) -> Result<(), MsgErr> {
        self.body.replace(body);
        Ok(())
    }

    pub fn intended<I: ToRecipients + Clone>(&mut self, intended: I) {
        self.intended.replace(intended.to_recipients());
    }

    pub fn reflection_of(&mut self, id: WaveId) {
        self.reflection_of.replace(id);
    }

    pub fn status(&mut self, status: u16) {
        self.status
            .replace(StatusCode::from_u16(status).unwrap_or(StatusCode::from_u16(500).unwrap()));
    }

    pub fn to(&mut self, to: Port) {
        self.to.replace(to.clone());
    }

    pub fn scope(&mut self, scope: Scope) {
        self.scope.replace(scope);
    }

    pub fn agent(&mut self, agent: Agent) {
        self.agent.replace(agent);
    }

    pub fn handling(&mut self, handling: Handling) {
        self.handling.replace(handling);
    }

    pub fn from(&mut self, from: Port) {
        self.from.replace(from);
    }

    pub fn build(self) -> Result<ReflectedWave, MsgErr> {
        let mut core = ReflectedCore::new();
        core.body = self.body.or_else(|| Some(Substance::Empty)).unwrap();
        core.status = self
            .status
            .or_else(|| Some(StatusCode::from_u16(200u16).unwrap()))
            .unwrap();
        match self.kind.ok_or("missing ReflectedWave Kind")? {
            ReflectedKind::Pong => {
                let mut pong = Wave::new(
                    Pong::new(
                        core,
                        self.to.ok_or("ReflectedProto missing to")?,
                        self.intended.ok_or("Reflected Proto Missing intended")?,
                        self.reflection_of.ok_or("response to expected")?,
                    ),
                    self.from.ok_or("expected from")?,
                );
                pong.track = self.track;
                Ok(pong.to_reflected())
            }
            ReflectedKind::Echo => {
                let mut echo = Wave::new(
                    Echo::new(
                        core,
                        self.to.ok_or("ReflectedProto missing to")?,
                        self.intended.ok_or("Reflected Proto Missing intended")?,
                        self.reflection_of.ok_or("response to expected")?,
                    ),
                    self.from.ok_or("expected from")?,
                );
                echo.track = self.track;
                Ok(echo.to_reflected())
            }
        }
    }
}

#[derive(Clone)]
pub struct DirectedProto {
    pub id: WaveId,
    pub from: Option<Port>,
    pub to: Option<Recipients>,
    pub core: DirectedCore,
    pub handling: Option<Handling>,
    pub scope: Option<Scope>,
    pub agent: Option<Agent>,
    pub kind: Option<DirectedKind>,
    pub bounce_backs: Option<BounceBacks>,
    pub track: bool,
}
impl Trackable for DirectedProto {
    fn track_id(&self) -> String {
        self.id.to_short_string()
    }

    fn track_method(&self) -> String {
        self.core.method.to_deep_string()
    }

    fn track_payload(&self) -> String {
        self.core.body.to_string()
    }

    fn track_from(&self) -> String {
        match &self.from {
            None => "None".to_string(),
            Some(from) => from.to_string(),
        }
    }

    fn track_to(&self) -> String {
        match &self.to {
            None => "None".to_string(),
            Some(to) => to.to_string(),
        }
    }

    fn track(&self) -> bool {
        self.track
    }
}

impl DirectedProto {
    pub fn build(self) -> Result<DirectedWave, MsgErr> {
        let kind = self.kind.ok_or::<MsgErr>(
            "kind must be set for DirectedProto to create the proper DirectedWave".into(),
        )?;

        let mut wave = match kind {
            DirectedKind::Ping => {
                let mut wave = Wave::new(
                    Ping {
                        to: self
                            .to
                            .ok_or(MsgErr::new(500u16, "must set 'to'"))?
                            .single_or()?,
                        core: self.core,
                    },
                    self.from.ok_or(MsgErr::new(500u16, "must set 'from'"))?,
                );
                wave.agent = self.agent.unwrap_or_else(|| Agent::Anonymous);
                wave.handling = self.handling.unwrap_or_else(|| Handling::default());
                wave.scope = self.scope.unwrap_or_else(|| Scope::None);
                wave.track = self.track;
                wave.to_directed()
            }
            DirectedKind::Ripple => {
                let mut wave = Wave::new(
                    Ripple {
                        to: self.to.ok_or(MsgErr::new(500u16, "must set 'to'"))?,
                        core: self.core,
                        bounce_backs: self.bounce_backs.ok_or("BounceBacks must be set")?,
                        history: HashSet::default(),
                    },
                    self.from.ok_or(MsgErr::new(500u16, "must set 'from'"))?,
                );
                wave.agent = self.agent.unwrap_or_else(|| Agent::Anonymous);
                wave.handling = self.handling.unwrap_or_else(|| Handling::default());
                wave.scope = self.scope.unwrap_or_else(|| Scope::None);
                wave.track = self.track;
                wave.to_directed()
            }
            DirectedKind::Signal => {
                let mut wave = Wave::new(
                    Signal {
                        to: self
                            .to
                            .ok_or(MsgErr::new(500u16, "must set 'to'"))?
                            .single_or()?,
                        core: self.core,
                    },
                    self.from.ok_or(MsgErr::new(500u16, "must set 'from'"))?,
                );
                wave.agent = self.agent.unwrap_or_else(|| Agent::Anonymous);
                wave.handling = self.handling.unwrap_or_else(|| Handling::default());
                wave.scope = self.scope.unwrap_or_else(|| Scope::None);
                wave.track = self.track;
                wave.to_directed()
            }
        };

        Ok(wave)
    }

    pub fn fill(&mut self, wave: &UltraWave) {
        self.fill_handling(wave.handling());
        self.fill_scope(wave.scope());
        self.fill_agent(wave.agent());
    }

    pub fn fill_kind(&mut self, kind: DirectedKind) {
        if self.kind.is_none() {
            self.kind.replace(kind);
        }
    }

    pub fn fill_to<R: ToRecipients + Clone>(&mut self, to: R) {
        if self.to.is_none() {
            self.to.replace(to.to_recipients());
        }
    }

    pub fn fill_from<P: ToPort>(&mut self, from: P) {
        if self.from.is_none() {
            self.from.replace(from.to_port());
        }
    }

    pub fn fill_scope(&mut self, scope: &Scope) {
        if self.scope.is_none() {
            self.scope.replace(scope.clone());
        }
    }

    pub fn fill_agent(&mut self, agent: &Agent) {
        if self.agent.is_none() {
            self.agent.replace(agent.clone());
        }
    }

    pub fn fill_handling(&mut self, handling: &Handling) {
        if self.handling.is_none() {
            self.handling.replace(handling.clone());
        }
    }

    pub fn agent(&mut self, agent: Agent) {
        self.agent.replace(agent);
    }

    pub fn scope(&mut self, scope: Scope) {
        self.scope.replace(scope);
    }

    pub fn handling(&mut self, handling: Handling) {
        self.handling.replace(handling);
    }

    pub fn kind(&mut self, kind: DirectedKind) {
        self.kind.replace(kind);
    }

    pub fn body(&mut self, body: Substance) {
        self.core.body = body;
    }

    pub fn core(&mut self, core: DirectedCore) -> Result<(), MsgErr> {
        self.core = core;
        Ok(())
    }

    pub fn method<M: Into<Method>>(&mut self, method: M) {
        self.core.method = method.into();
    }

    pub fn to<P: ToRecipients + Clone>(&mut self, to: P) {
        self.to.replace(to.to_recipients());
    }

    pub fn from<P: ToPort>(&mut self, from: P) {
        self.from.replace(from.to_port());
    }
}

impl DirectedProto {
    pub fn ping() -> Self {
        Self {
            id: WaveId::new(WaveKind::Ping),
            kind: Some(DirectedKind::Ping),
            ..DirectedProto::default()
        }
    }

    pub fn signal() -> Self {
        Self {
            id: WaveId::new(WaveKind::Signal),
            kind: Some(DirectedKind::Signal),
            ..DirectedProto::default()
        }
    }

    pub fn ripple() -> Self {
        Self {
            id: WaveId::new(WaveKind::Ripple),
            kind: Some(DirectedKind::Ripple),
            ..DirectedProto::default()
        }
    }

    pub fn to_with_method<P: ToRecipients + Clone>(to: P, method: Method) -> Self {
        Self {
            id: WaveId::new(WaveKind::Ping),
            to: Some(to.to_recipients()),
            kind: Some(DirectedKind::Ping),
            core: DirectedCore::new(method),
            ..DirectedProto::default()
        }
    }

    pub fn from_core(core: DirectedCore) -> Self {
        Self {
            id: WaveId::new(WaveKind::Ping),
            kind: Some(DirectedKind::Ping),
            core,
            ..DirectedProto::default()
        }
    }

    pub fn sys<M: Into<SysMethod>, P: ToRecipients + Clone>(to: P, method: M) -> Self {
        let method: SysMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }

    pub fn msg<M: Into<MsgMethod>, P: ToRecipients + Clone>(to: P, method: M) -> Self {
        let method: MsgMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }

    pub fn http<M: Into<HttpMethod>, P: ToRecipients + Clone>(to: P, method: M) -> Self {
        let method: HttpMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }

    pub fn cmd<M: Into<CmdMethod>, P: ToRecipients + Clone>(to: P, method: M) -> Self {
        let method: CmdMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }
}

impl Default for DirectedProto {
    fn default() -> Self {
        Self {
            id: WaveId::new(WaveKind::Ping),
            core: DirectedCore::default(),
            from: None,
            to: None,
            handling: None,
            scope: None,
            agent: None,
            kind: None,
            bounce_backs: None,
            track: false,
        }
    }
}

pub type Echoes = Vec<Wave<Echo>>;

impl FromReflectedAggregate for () {
    fn from_reflected_aggregate(agg: ReflectedAggregate) -> Result<Self, MsgErr>
    where
        Self: Sized,
    {
        match agg {
            ReflectedAggregate::None => Ok(()),
            _ => Err(MsgErr::bad_request()),
        }
    }
}

impl FromReflectedAggregate for Echoes {
    fn from_reflected_aggregate(agg: ReflectedAggregate) -> Result<Self, MsgErr>
    where
        Self: Sized,
    {
        match agg {
            ReflectedAggregate::Multi(reflected) => {
                let mut echoes = Echoes::new();
                for r in reflected {
                    let echo: Wave<Echo> = r.to_echo()?;
                    echoes.push(echo);
                }
                Ok(echoes)
            }
            _ => Err(MsgErr::bad_request()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Echo {
    /// this is meant to be the intended request recipient, which may not be the point responding
    /// to this message in the case it was intercepted and filtered at some point
    pub to: Port,
    pub intended: Recipients,
    pub core: ReflectedCore,
    pub reflection_of: WaveId,
}

impl<S> ToSubstance<S> for Echo
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.core.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.core.to_substance_ref()
    }
}

impl Echo {
    pub fn is_ok(&self) -> bool {
        self.core.is_ok()
    }

    pub fn core<E>(result: Result<Wave<Pong>, E>) -> ReflectedCore {
        match result {
            Ok(reflected) => reflected.variant.core,
            Err(err) => ReflectedCore::server_error(),
        }
    }

    pub fn as_result<E: From<&'static str>, P: TryFrom<Substance>>(self) -> Result<P, E> {
        self.core.as_result()
    }
}

impl Echo {
    pub fn new(core: ReflectedCore, to: Port, intended: Recipients, reflection_of: WaveId) -> Self {
        Self {
            to,
            intended,
            core,
            reflection_of,
        }
    }

    pub fn ok_or(self) -> Result<Self, MsgErr> {
        if self.core.status.is_success() {
            Ok(self)
        } else {
            if let Substance::Text(error) = self.core.body {
                Err(error.into())
            } else {
                Err(format!("error code: {}", self.core.status).into())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Pong {
    /// this is meant to be the intended request recipient, which may not be the point responding
    /// to this message in the case it was intercepted and filtered at some point
    pub to: Port,
    pub intended: Recipients,
    pub core: ReflectedCore,
    pub reflection_of: WaveId,
}

impl FromReflectedAggregate for Wave<Pong> {
    fn from_reflected_aggregate(agg: ReflectedAggregate) -> Result<Self, MsgErr>
    where
        Self: Sized,
    {
        match agg {
            ReflectedAggregate::Single(reflected) => match reflected {
                ReflectedWave::Pong(pong) => Ok(pong),
                _ => Err(MsgErr::bad_request()),
            },
            _ => Err(MsgErr::bad_request()),
        }
    }
}

impl<S> ToSubstance<S> for Pong
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.core.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.core.to_substance_ref()
    }
}

impl Pong {
    pub fn is_ok(&self) -> bool {
        self.core.is_ok()
    }

    pub fn core<E>(result: Result<Wave<Pong>, E>) -> ReflectedCore {
        match result {
            Ok(reflected) => reflected.variant.core,
            Err(err) => ReflectedCore::server_error(),
        }
    }

    pub fn as_result<E: From<&'static str>, P: TryFrom<Substance>>(self) -> Result<P, E> {
        self.core.as_result()
    }

    pub fn ok_or(&self) -> Result<(), MsgErr> {
        if self.is_ok() {
            Ok(())
        } else {
            if let Substance::Errors(errs) = &self.core.body  {
                Err(format!("{} : {}", self.core.status.to_string(), errs.to_string()).into())
            } else {
                Err(self.core.status.to_string().into())
            }
        }
    }
}

impl Pong {
    pub fn new(core: ReflectedCore, to: Port, intended: Recipients, reflection_of: WaveId) -> Self {
        Self {
            to,
            intended,
            core,
            reflection_of,
        }
    }
}

pub struct RecipientSelector<'a> {
    pub to: &'a Port,
    pub wave: &'a DirectedWave,
}

impl<'a> RecipientSelector<'a> {
    pub fn new(to: &'a Port, wave: &'a Wave<DirectedWave>) -> Self {
        Self { to, wave }
    }
}

pub type DirectedWave = DirectedWaveDef<Recipients>;
pub type SingularDirectedWave = DirectedWaveDef<Port>;

impl Trackable for DirectedWave {
    fn track_id(&self) -> String {
        self.id().to_short_string()
    }

    fn track_method(&self) -> String {
        match self {
            Self::Ping(ping) => ping.core.method.to_deep_string(),
            Self::Ripple(ripple) => ripple.core.method.to_deep_string(),
            Self::Signal(signal) => signal.core.method.to_deep_string(),
        }
    }

    fn track_payload(&self) -> String {
        match self {
            Self::Ping(ping) => ping.core.body.kind().to_string(),
            Self::Ripple(ripple) => ripple.core.body.kind().to_string(),
            Self::Signal(signal) => signal.core.body.kind().to_string(),
        }
    }

    fn track_from(&self) -> String {
        self.from().to_string()
    }

    fn track_to(&self) -> String {
        self.to().to_string()
    }

    fn track(&self) -> bool {
        match self {
            Self::Ping(ping) => ping.track,
            Self::Ripple(ripple) => ripple.track,
            Self::Signal(signal) => signal.track,
        }
    }
    fn track_payload_fmt(&self) -> String {
        match self {
            Self::Signal(signal) => signal.track_payload_fmt(),
            Self::Ping(ping) => ping.track_payload_fmt(),
            Self::Ripple(_) => self.track_payload(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum DirectedWaveDef<T>
where
    T: ToRecipients + Clone,
{
    Ping(Wave<Ping>),
    Ripple(Wave<RippleDef<T>>),
    Signal(Wave<Signal>),
}

impl<T> DirectedWaveDef<T>
where
    T: ToRecipients + Clone,
{
    pub fn kind(&self) -> WaveKind {
        match self {
            DirectedWaveDef::Ping(_) => WaveKind::Ping,
            DirectedWaveDef::Ripple(_) => WaveKind::Ripple,
            DirectedWaveDef::Signal(_) => WaveKind::Signal,
        }
    }
}

impl<W> Spannable for DirectedWaveDef<W>
where
    W: ToRecipients + Clone,
{
    fn span_id(&self) -> String {
        self.id().to_string()
    }

    fn span_type(&self) -> &'static str {
        "Wave"
    }
}

impl DirectedWave {
    pub fn to(&self) -> Recipients {
        match self {
            Self::Ping(ping) => ping.to.clone().to_recipients(),
            Self::Ripple(ripple) => ripple.to.clone(),
            Self::Signal(signal) => signal.to.clone().to_recipients(),
        }
    }

    pub fn hops(&self) -> u16 {
        match self {
            DirectedWave::Ping(ping) => ping.hops.clone(),
            DirectedWave::Ripple(ripple) => ripple.hops.clone(),
            DirectedWave::Signal(signal) => signal.hops.clone(),
        }
    }

    pub fn reflection(&self) -> Result<Reflection, MsgErr> {
        Ok(Reflection {
            kind: match self {
                DirectedWave::Ping(_) => ReflectedKind::Pong,
                DirectedWave::Ripple(_) => ReflectedKind::Echo,
                DirectedWave::Signal(_) => return Err("signals do not have a reflected".into()),
            },
            to: self.reflect_to().clone(),
            intended: self.to(),
            reflection_of: self.id().clone(),
            track: self.track(),
        })
    }

    pub fn to_signal(self) -> Result<Wave<Signal>, MsgErr> {
        match self {
            DirectedWave::Signal(signal) => Ok(signal),
            _ => Err("not a signal wave".into()),
        }
    }

    pub fn to_call(&self, to: Port) -> Result<Call, MsgErr> {
        let kind = match &self.core().method {
            Method::Cmd(method) => CallKind::Cmd(CmdCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
            Method::Sys(method) => CallKind::Sys(SysCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
            Method::Http(method) => CallKind::Http(HttpCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
            Method::Msg(method) => CallKind::Msg(MsgCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
        };

        Ok(Call {
            point: to.point,
            kind,
        })
    }
}

impl Trackable for SingularDirectedWave {
    fn track_id(&self) -> String {
        self.id().to_short_string()
    }

    fn track_method(&self) -> String {
        match self {
            Self::Ping(ping) => ping.core.method.to_deep_string(),
            Self::Ripple(ripple) => ripple.core.method.to_deep_string(),
            Self::Signal(signal) => signal.core.method.to_deep_string(),
        }
    }

    fn track_payload(&self) -> String {
        match self {
            Self::Ping(ping) => ping.core.body.kind().to_string(),
            Self::Ripple(ripple) => ripple.core.body.kind().to_string(),
            Self::Signal(signal) => signal.core.body.kind().to_string(),
        }
    }

    fn track_from(&self) -> String {
        self.from().to_string()
    }

    fn track_to(&self) -> String {
        self.to().to_string()
    }

    fn track(&self) -> bool {
        match self {
            Self::Ping(ping) => ping.track,
            Self::Ripple(ripple) => ripple.track,
            Self::Signal(signal) => signal.track,
        }
    }
    fn track_payload_fmt(&self) -> String {
        match self {
            Self::Signal(signal) => signal.track_payload_fmt(),
            Self::Ping(ping) => ping.track_payload_fmt(),
            Self::Ripple(_) => self.track_payload(),
        }
    }
}
impl SingularDirectedWave {
    pub fn to(&self) -> Port {
        match self {
            Self::Ping(ping) => ping.to.clone(),
            Self::Ripple(ripple) => ripple.to.clone(),
            Self::Signal(signal) => signal.to.clone(),
        }
    }

    pub fn to_call(&self) -> Result<Call, MsgErr> {
        let kind = match &self.core().method {
            Method::Cmd(method) => CallKind::Cmd(CmdCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
            Method::Sys(method) => CallKind::Sys(SysCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
            Method::Http(method) => CallKind::Http(HttpCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
            Method::Msg(method) => CallKind::Msg(MsgCall::new(
                method.clone(),
                Subst::new(self.core().uri.path())?,
            )),
        };

        Ok(Call {
            point: self.to().clone().to_point(),
            kind,
        })
    }

    pub fn reflection(&self) -> Result<Reflection, MsgErr> {
        Ok(Reflection {
            kind: match self {
                SingularDirectedWave::Ping(_) => ReflectedKind::Pong,
                SingularDirectedWave::Ripple(_) => ReflectedKind::Echo,
                SingularDirectedWave::Signal(_) => {
                    return Err("signals do not have a reflected".into())
                }
            },
            to: self.from().clone(),
            intended: self.to().to_recipients(),
            reflection_of: self.id().clone(),
            track: self.track(),
        })
    }

    pub fn to_ultra(self) -> UltraWave {
        match self {
            SingularDirectedWave::Ping(ping) => UltraWave::Ping(ping),
            SingularDirectedWave::Signal(signal) => UltraWave::Signal(signal),
            SingularDirectedWave::Ripple(ripple) => UltraWave::Ripple(ripple.to_multiple()),
        }
    }
}

impl<T> DirectedWaveDef<T>
where
    T: ToRecipients + Clone,
{
    pub fn id(&self) -> &WaveId {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.id,
            DirectedWaveDef::Ripple(ripple) => &ripple.id,
            DirectedWaveDef::Signal(signal) => &signal.id,
        }
    }

    pub fn agent(&self) -> &Agent {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.agent,
            DirectedWaveDef::Ripple(ripple) => &ripple.agent,
            DirectedWaveDef::Signal(signal) => &signal.agent,
        }
    }

    pub fn scope(&self) -> &Scope {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.scope,
            DirectedWaveDef::Ripple(ripple) => &ripple.scope,
            DirectedWaveDef::Signal(signal) => &signal.scope,
        }
    }

    pub fn handling(&self) -> &Handling {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.handling,
            DirectedWaveDef::Ripple(ripple) => &ripple.handling,
            DirectedWaveDef::Signal(signal) => &signal.handling,
        }
    }

    pub fn err(&self, err: MsgErr, responder: Port) -> Bounce<ReflectedWave> {
        match self {
            DirectedWaveDef::Ping(ping) => {
                Bounce::Reflected(ping.err(err, responder).to_reflected())
            }
            DirectedWaveDef::Ripple(ripple) => {
                Bounce::Reflected(ripple.err(err, responder).to_reflected())
            }
            DirectedWaveDef::Signal(_) => Bounce::Absorbed,
        }
    }

    pub fn bounce_backs(&self) -> BounceBacks {
        match self {
            DirectedWaveDef::Ping(ping) => ping.bounce_backs(),
            DirectedWaveDef::Ripple(ripple) => ripple.bounce_backs(),
            DirectedWaveDef::Signal(signal) => signal.bounce_backs(),
        }
    }

    pub fn from(&self) -> &Port {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.from,
            DirectedWaveDef::Ripple(ripple) => &ripple.from,
            DirectedWaveDef::Signal(signal) => &signal.from,
        }
    }

    pub fn via(&self) -> &Option<Port> {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.via,
            DirectedWaveDef::Ripple(ripple) => &ripple.via,
            DirectedWaveDef::Signal(signal) => &signal.via,
        }
    }

    pub fn reflect_to(&self) -> &Port {
        self.via().as_ref().unwrap_or(self.from())
    }

    pub fn take_via(&mut self) -> Option<Port> {
        match self {
            DirectedWaveDef::Ping(ping) => ping.via.take(),
            DirectedWaveDef::Ripple(ripple) => ripple.via.take(),
            DirectedWaveDef::Signal(signal) => signal.via.take(),
        }
    }

    pub fn replace_via(&mut self, port: Port) -> Option<Port>{
        match self {
            DirectedWaveDef::Ping(ping) => ping.via.replace(port),
            DirectedWaveDef::Ripple(ripple) => ripple.via.replace(port),
            DirectedWaveDef::Signal(signal) => signal.via.replace(port),
        }
    }

    pub fn body(&self) -> &Substance {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.core.body,
            DirectedWaveDef::Ripple(ripple) => &ripple.core.body,
            DirectedWaveDef::Signal(signal) => &signal.core.body,
        }
    }

    pub fn core(&self) -> &DirectedCore {
        match self {
            DirectedWaveDef::Ping(ping) => &ping.core,
            DirectedWaveDef::Ripple(ripple) => &ripple.core,
            DirectedWaveDef::Signal(signal) => &signal.core,
        }
    }
}

pub struct Reflection {
    pub kind: ReflectedKind,
    pub to: Port,
    pub intended: Recipients,
    pub reflection_of: WaveId,
    pub track: bool,
}

impl Reflection {
    pub fn make(self, core: ReflectedCore, from: Port) -> ReflectedWave {
        let mut wave = Wave::new(
            Pong::new(core, self.to, self.intended, self.reflection_of),
            from,
        );
        wave.track = self.track;
        wave.to_reflected()
    }
}

impl<S> ToSubstance<S> for DirectedWave
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        match self {
            DirectedWave::Ping(ping) => ping.to_substance(),
            DirectedWave::Ripple(ripple) => ripple.to_substance(),
            DirectedWave::Signal(signal) => signal.to_substance(),
        }
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        match self {
            DirectedWave::Ping(ping) => ping.to_substance_ref(),
            DirectedWave::Ripple(ripple) => ripple.to_substance_ref(),
            DirectedWave::Signal(signal) => signal.to_substance_ref(),
        }
    }
}

impl DirectedWave {
    pub fn to_ultra(self) -> UltraWave {
        match self {
            DirectedWave::Ping(ping) => UltraWave::Ping(ping),
            DirectedWave::Ripple(ripple) => UltraWave::Ripple(ripple),
            DirectedWave::Signal(signal) => UltraWave::Signal(signal),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ReflectedWave {
    Pong(Wave<Pong>),
    Echo(Wave<Echo>),
}

impl Trackable for ReflectedWave {
    fn track_id(&self) -> String {
        self.id().to_string()
    }

    fn track_method(&self) -> String {
        self.core().status.to_string()
    }

    fn track_payload(&self) -> String {
        self.core().body.kind().to_string()
    }

    fn track_from(&self) -> String {
        self.from().to_string()
    }

    fn track_to(&self) -> String {
        self.to().to_string()
    }

    fn track(&self) -> bool {
        match self {
            ReflectedWave::Pong(pong) => pong.track,
            ReflectedWave::Echo(echo) => echo.track
        }
    }
}

impl<S> ToSubstance<S> for ReflectedWave
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        match self {
            ReflectedWave::Pong(pong) => pong.to_substance(),
            ReflectedWave::Echo(echo) => echo.to_substance(),
        }
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        match self {
            ReflectedWave::Pong(pong) => pong.to_substance_ref(),
            ReflectedWave::Echo(echo) => echo.to_substance_ref(),
        }
    }
}

pub trait ToReflected {
    fn to_reflected(self) -> ReflectedWave;
    fn from_reflected(reflected: ReflectedWave) -> Result<Self, MsgErr>
    where
        Self: Sized;
}

impl ReflectedWave {

    pub fn from(&self) -> &Port {
        match self {
            ReflectedWave::Pong(pong) => &pong.from,
            ReflectedWave::Echo(echo) => &echo.from
        }
    }

    pub fn to(&self) -> &Port {
        match self {
            ReflectedWave::Pong(pong) => &pong.to,
            ReflectedWave::Echo(echo) => &echo.to
        }
    }


    pub fn id(&self) -> &WaveId {
        match self {
            ReflectedWave::Pong(pong) => &pong.id,
            ReflectedWave::Echo(echo) => &echo.id,
        }
    }

    pub fn to_ultra(self) -> UltraWave {
        match self {
            ReflectedWave::Pong(pong) => UltraWave::Pong(pong),
            ReflectedWave::Echo(echo) => UltraWave::Echo(echo),
        }
    }

    pub fn reflection_of(&self) -> &WaveId {
        match self {
            ReflectedWave::Pong(pong) => &pong.reflection_of,
            ReflectedWave::Echo(echo) => &echo.reflection_of,
        }
    }

    pub fn core(&self) -> &ReflectedCore {
        match self {
            ReflectedWave::Pong(pong) => &pong.core,
            ReflectedWave::Echo(echo) => &echo.core,
        }
    }

    pub fn to_echo(self) -> Result<Wave<Echo>, MsgErr> {
        match self {
            ReflectedWave::Echo(echo) => Ok(echo),
            _ => Err(MsgErr::bad_request()),
        }
    }

    pub fn to_pong(self) -> Result<Wave<Pong>, MsgErr> {
        match self {
            ReflectedWave::Pong(pong) => Ok(pong),
            _ => Err(MsgErr::bad_request()),
        }
    }
}

impl ReflectedWave {
    pub fn is_success(&self) -> bool {
        match self {
            ReflectedWave::Pong(pong) => return pong.core.status.is_success(),
            ReflectedWave::Echo(echo) => return echo.core.status.is_success(),
        }
    }

    pub fn success_or(&self) -> Result<(), MsgErr> {
        if self.is_success() {
            Ok(())
        } else {
            match self {
                ReflectedWave::Pong(pong) => Err(MsgErr::Status {
                    status: pong.core.status.as_u16(),
                    message: "error".to_string(),
                }),
                ReflectedWave::Echo(echo) => Err(MsgErr::Status {
                    status: echo.core.status.as_u16(),
                    message: "error".to_string(),
                }),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Recipients {
    Single(Port),
    Multi(Vec<Port>),
    Watchers(Watch),
    Stars,
}

impl ToString for Recipients {
    fn to_string(&self) -> String {
        match self {
            Recipients::Single(port) => port.to_string(),
            Recipients::Multi(_) => "Multi".to_string(),
            Recipients::Watchers(_) => "Watchers".to_string(),
            Recipients::Stars => "Stars".to_string(),
        }
    }
}

impl ToRecipients for Recipients {
    fn to_recipients(self) -> Recipients {
        self
    }
}

impl Recipients {
    pub fn to_single(self) -> Result<Port, MsgErr> {
        match self {
            Recipients::Single(port) => Ok(port),
            _ => Err(MsgErr::from_500(
                "cannot convert a multiple recipient into a single",
            )),
        }
    }
    pub fn is_match(&self, point: &Point) -> bool {
        match self {
            Recipients::Single(port) => port.point == *point,
            Recipients::Multi(ports) => {
                for port in ports {
                    if port.point == *point {
                        return true;
                    }
                }
                false
            }
            Recipients::Watchers(_) => false,
            Recipients::Stars => {
                if let RouteSeg::Star(_) = point.route {
                    if point.segments.len() == 1
                        && *point.segments.first().unwrap() == PointSeg::Space("star".to_string())
                    {
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }

    pub fn split(map: HashMap<Point, Vec<Port>>) -> HashMap<Point, Recipients> {
        let mut rtn = HashMap::new();
        for (point, value) in map {
            rtn.insert(point, Recipients::Multi(value));
        }
        rtn
    }
}

impl ToRecipients for &Recipients {
    fn to_recipients(self) -> Recipients {
        self.clone()
    }
}

pub trait ToRecipients {
    fn to_recipients(self) -> Recipients;
}

impl Recipients {
    pub fn select_ports(&self, point: &Point) -> Vec<&Port> {
        let mut rtn = vec![];
        match self {
            Recipients::Single(port) => {
                if port.point == *point {
                    rtn.push(port);
                }
            }
            Recipients::Multi(ports) => {
                for port in ports {
                    if port.point == *point {
                        rtn.push(port);
                    }
                }
            }
            Recipients::Watchers(_) => {}
            Recipients::Stars => {}
        }
        rtn
    }

    pub fn is_single(&self) -> bool {
        match self {
            Recipients::Single(_) => true,
            Recipients::Multi(_) => false,
            Recipients::Watchers(_) => false,
            Recipients::Stars => false,
        }
    }

    pub fn is_multi(&self) -> bool {
        match self {
            Recipients::Single(_) => false,
            Recipients::Multi(_) => true,
            Recipients::Watchers(_) => false,
            Recipients::Stars => false,
        }
    }

    pub fn is_stars(&self) -> bool {
        match self {
            Recipients::Single(_) => false,
            Recipients::Multi(_) => false,
            Recipients::Watchers(_) => false,
            Recipients::Stars => true,
        }
    }

    pub fn is_watch(&self) -> bool {
        match self {
            Recipients::Single(_) => false,
            Recipients::Multi(_) => false,
            Recipients::Watchers(_) => true,
            Recipients::Stars => false,
        }
    }

    pub fn unwrap_single(self) -> Port {
        self.single_or().expect("single")
    }

    pub fn single_or(self) -> Result<Port, MsgErr> {
        if let Recipients::Single(rtn) = self {
            Ok(rtn)
        } else {
            Err("not a single".into())
        }
    }
}

pub type IpAddr = String;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Origin {
    Ip(IpAddr),
    Point(Point),
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Crypt<S> {
    pub payload: S,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SessionId {
    pub origin: Crypt<Origin>,
    pub uuid: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Wave<V> {
    pub id: WaveId,
    pub session: Option<SessionId>,
    pub variant: V,
    pub agent: Agent,
    pub handling: Handling,
    pub scope: Scope,
    pub from: Port,
    pub via: Option<Port>,
    pub hops: u16,
    pub track: bool,
}

impl<S, V> ToSubstance<S> for Wave<V>
where
    V: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.variant.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.variant.to_substance_ref()
    }
}

impl<V> Wave<V> {
    pub fn inc_hops(&mut self) {
        self.hops = self.hops + 1;
    }
}

impl Wave<Ripple> {
    pub fn to_ultra(self) -> UltraWave {
        UltraWave::Ripple(self)
    }

    pub fn to_directed(self) -> DirectedWave {
        DirectedWave::Ripple(self)
    }

    pub fn with_core(mut self, core: DirectedCore ) -> Self {
        self.variant.core = core;
        self
    }
}

impl<T> Wave<RippleDef<T>>
where
    T: ToRecipients + Clone,
{
    pub fn err(&self, err: MsgErr, responder: Port) -> Wave<Echo> {
        Wave::new(
            Echo::new(
                self.variant.err(err),
                self.from.clone(),
                self.to.clone().to_recipients(),
                self.id.clone(),
            ),
            responder,
        )
    }
}

impl Trackable for Wave<Signal> {
    fn track_id(&self) -> String {
        self.id.to_short_string()
    }

    fn track_method(&self) -> String {
        self.method.to_deep_string()
    }

    fn track_payload(&self) -> String {
        self.core.body.kind().to_string()
    }

    fn track_from(&self) -> String {
        self.from.to_string()
    }

    fn track_to(&self) -> String {
        self.to.to_string()
    }

    fn track(&self) -> bool {
        self.track
    }

    fn track_payload_fmt(&self) -> String {
        match &self.core.body {
            Substance::UltraWave(wave) => {
                format!("UltraWave({})", wave.track_key_fmt())
            }
            _ => self.track_payload(),
        }
    }
}

impl Wave<Signal> {
    pub fn to_ultra(self) -> UltraWave {
        UltraWave::Signal(self)
    }

    pub fn to_directed(self) -> DirectedWave {
        DirectedWave::Signal(self)
    }

    pub fn with_core(mut self, core: DirectedCore ) -> Self {
        self.variant.core = core;
        self
    }

    pub fn wrap_in_hop(self, from: Port, to: Port) -> DirectedProto {
        let mut signal = DirectedProto::ping();
        signal.kind(DirectedKind::Signal);
        signal.from(from);
        signal.agent(self.agent.clone());
        signal.handling(self.handling.clone());
        signal.method(SysMethod::Hop);
        signal.track = self.track;
        signal.body(Substance::UltraWave(Box::new(self.to_ultra())));
        signal.to(to);
        signal
    }

    pub fn unwrap_from_hop(self) -> Result<Wave<Signal>, MsgErr> {
        if self.method != Method::Sys(SysMethod::Hop) {
            return Err(MsgErr::from_500("expected signal wave to have method Hop"));
        }
        if let Substance::UltraWave(wave) = &self.body {
            Ok((*wave.clone()).to_signal()?)
        } else {
            Err(MsgErr::from_500(
                "expected body substance to be of type UltraWave for a transport signal",
            ))
        }
    }

    pub fn unwrap_from_transport(self) -> Result<UltraWave, MsgErr> {
        if self.method != Method::Sys(SysMethod::Transport) {
            return Err(MsgErr::from_500(
                "expected signal wave to have method Transport",
            ));
        }
        if let Substance::UltraWave(wave) = &self.body {
            Ok(*wave.clone())
        } else {
            Err(MsgErr::from_500(
                "expected body substance to be of type UltraWave for a transport signal",
            ))
        }
    }

    pub fn to_singular_directed(self) -> SingularDirectedWave {
        SingularDirectedWave::Signal(self)
    }
}

impl<S> ToSubstance<S> for Signal
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.core.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.core.to_substance_ref()
    }
}

impl Trackable for Wave<Ping> {
    fn track_id(&self) -> String {
        self.id.to_short_string()
    }

    fn track_method(&self) -> String {
        self.method.to_deep_string()
    }

    fn track_payload(&self) -> String {
        self.core.body.kind().to_string()
    }

    fn track_from(&self) -> String {
        self.from.to_string()
    }

    fn track_to(&self) -> String {
        self.to.to_string()
    }

    fn track(&self) -> bool {
        self.track
    }

    fn track_payload_fmt(&self) -> String {
        match &self.core.body {
            Substance::UltraWave(wave) => {
                format!("UltraWave({})", wave.track_key_fmt())
            }
            _ => self.track_payload(),
        }
    }
}

impl Wave<Ping> {
    pub fn to_ultra(self) -> UltraWave {
        UltraWave::Ping(self)
    }

    pub fn to_directed(self) -> DirectedWave {
        DirectedWave::Ping(self)
    }

    pub fn err(&self, err: MsgErr, responder: Port) -> Wave<Pong> {
        Wave::new(
            Pong::new(
                self.variant.err(err),
                self.from.clone(),
                self.to.clone().to_recipients(),
                self.id.clone(),
            ),
            responder,
        )
    }

    pub fn bounce_backs(&self) -> BounceBacks {
        BounceBacks::Single
    }
}

impl Wave<Pong> {
    pub fn to_ultra(self) -> UltraWave {
        UltraWave::Pong(self)
    }

    pub fn to_reflected(self) -> ReflectedWave {
        ReflectedWave::Pong(self)
    }
}

impl ToReflected for Wave<Pong> {
    fn to_reflected(self) -> ReflectedWave {
        ReflectedWave::Pong(self)
    }

    fn from_reflected(reflected: ReflectedWave) -> Result<Self, MsgErr> {
        match reflected {
            ReflectedWave::Pong(pong) => Ok(pong),
            _ => Err(MsgErr::bad_request()),
        }
    }
}

impl ToReflected for Wave<Echo> {
    fn to_reflected(self) -> ReflectedWave {
        ReflectedWave::Echo(self)
    }

    fn from_reflected(reflected: ReflectedWave) -> Result<Self, MsgErr> {
        match reflected {
            ReflectedWave::Echo(echo) => Ok(echo),
            _ => Err(MsgErr::bad_request()),
        }
    }
}

impl Wave<Echo> {
    pub fn to_ultra(self) -> UltraWave {
        UltraWave::Echo(self)
    }

    pub fn to_reflected(self) -> ReflectedWave {
        ReflectedWave::Echo(self)
    }
}

impl TryFrom<ReflectedWave> for Wave<Pong> {
    type Error = MsgErr;

    fn try_from(wave: ReflectedWave) -> Result<Self, Self::Error> {
        match wave {
            ReflectedWave::Pong(pong) => Ok(pong),
            _ => Err(MsgErr::bad_request()),
        }
    }
}

impl<V> Wave<V> {
    pub fn new(variant: V, from: Port) -> Self
    where
        V: WaveVariant,
    {
        Self {
            id: WaveId::new(variant.kind().clone()),
            session: None,
            agent: Default::default(),
            handling: Default::default(),
            scope: Default::default(),
            variant,
            from,
            hops: 0,
            track: false,
            via: None
        }
    }

    pub fn replace<V2>(self, variant: V2) -> Wave<V2>
    where
        V2: WaveVariant,
    {
        Wave {
            id: self.id,
            session: self.session,
            agent: self.agent,
            handling: self.handling,
            scope: self.scope,
            variant,
            from: self.from,
            hops: self.hops,
            track: false,
            via: self.via
        }
    }
}

pub trait WaveVariant {
    fn kind(&self) -> WaveKind;
}

impl WaveVariant for Ping {
    fn kind(&self) -> WaveKind {
        WaveKind::Ping
    }
}

impl WaveVariant for Pong {
    fn kind(&self) -> WaveKind {
        WaveKind::Pong
    }
}

impl<T> WaveVariant for RippleDef<T>
where
    T: ToRecipients + Clone,
{
    fn kind(&self) -> WaveKind {
        WaveKind::Ripple
    }
}

impl WaveVariant for Echo {
    fn kind(&self) -> WaveKind {
        WaveKind::Echo
    }
}

impl Wave<Ping> {
    pub fn pong(&self) -> ReflectedProto {
        let mut pong = ReflectedProto::new();
        pong.kind(ReflectedKind::Pong);
        pong.fill(self);
        pong
    }
}

impl<T> Wave<RippleDef<T>>
where
    T: ToRecipients + Clone,
{
    pub fn echo(&self) -> ReflectedProto {
        let mut echo = ReflectedProto::new();
        echo.kind(ReflectedKind::Echo);
        echo.fill(self);
        echo
    }

    pub fn bounce_backs(&self) -> BounceBacks {
        self.bounce_backs.clone()
    }
}

impl Wave<SingularRipple> {
    pub fn to_singular_directed(self) -> SingularDirectedWave {
        SingularDirectedWave::Ripple(self)
    }
}

impl DirectedWave {
    pub fn reflected_proto(&self) -> BounceProto {
        match self {
            DirectedWave::Ping(ping) => BounceProto::Reflected(ping.pong()),
            DirectedWave::Ripple(ripple) => BounceProto::Reflected(ripple.echo()),
            DirectedWave::Signal(_) => BounceProto::Absorbed,
        }
    }
}

impl<V> Deref for Wave<V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.variant
    }
}

impl<V> DerefMut for Wave<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.variant
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Agent {
    Anonymous,
    HyperUser,
    Point(Point),
}

impl ToPoint for Agent {
    fn to_point(&self) -> Point {
        match self {
            Agent::Anonymous => ANONYMOUS.clone(),
            Agent::HyperUser => HYPERUSER.clone(),
            Agent::Point(point) => point.clone(),
        }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Self::Anonymous
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub attributes: HashMap<String, String>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            id: uuid(),
            attributes: HashMap::new(),
        }
    }
    pub fn get_preferred_username(&self) -> Option<String> {
        self.attributes
            .get(&"preferred_username".to_string())
            .cloned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Scope {
    Full,
    None,
    Grants(HashSet<ScopeGrant>),
}

impl Scope {
    pub fn has_grant(&self, grant: &ScopeGrant) -> Result<(), ()> {
        match self {
            Scope::Full => Ok(()),
            Scope::None => Err(()),
            Scope::Grants(grants) if grants.contains(grant) => Ok(()),
            _ => Err(()),
        }
    }

    pub fn enumerated_grants(&self) -> HashSet<ScopeGrant> {
        match self {
            Scope::Full => HashSet::new(),
            Scope::None => HashSet::new(),
            Scope::Grants(grants) => grants.clone(),
        }
    }
}

impl From<HashSet<ScopeGrant>> for Scope {
    fn from(grants: HashSet<ScopeGrant>) -> Self {
        Scope::Grants(grants)
    }
}

impl ops::BitAnd<Scope> for Scope {
    type Output = Scope;

    fn bitand(self, rhs: Scope) -> Self::Output {
        if self == Self::Full && rhs == Self::Full {
            Self::Full
        } else if self == Self::None || rhs == Self::None {
            Self::None
        } else {
            let mut grants = self.enumerated_grants();
            grants.retain(|grant| rhs.has_grant(grant).is_ok());
            grants.into()
        }
    }
}

impl ops::BitOr<Scope> for Scope {
    type Output = Scope;

    fn bitor(self, rhs: Scope) -> Self::Output {
        if self == Self::Full || rhs == Scope::Full {
            Self::Full
        } else {
            let left = self.enumerated_grants();
            let right = rhs.enumerated_grants();
            Self::Grants(left.union(&right).cloned().collect())
        }
    }
}

impl Scope {
    /*
    pub fn mask( &self, on: &AddressKindPath ) -> Access {
        match self {
            Scope::Full => {
                access.clone()
            }
            Scope::None => {
                Access::none()
            }
            Scope::Grants(grants) => {
                let mut access  = access.clone();
                let mut privileges = EnumeratedPrivileges::none();
                let mut permissions = Permissions::full();
                for grant in grants {
                   if grant.on.matches(on) {
                       match &grant.aspect {
                           ScopeGrantAspect::Perm(and) => permissions.and(and),
                           ScopeGrantAspect::Priv(and) =>  privileges.insert(and.clone())
                       }
                   }
               }
            }
        }
    }

     */
}

impl Default for Scope {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ScopeGrant {
    pub on: Selector,
    pub kind: ScopeGrantKind,
    pub aspect: ScopeGrantAspect,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ScopeGrantKind {
    Or,
    And,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ScopeGrantAspect {
    Perm(Permissions),
    Priv(Privilege),
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Handling {
    pub kind: HandlingKind,
    pub priority: Priority,
    pub retries: Retries,
    pub wait: WaitTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum HandlingKind {
    Durable,   // Mesh will guarantee delivery eventually once Request call has returned
    Queued,    // Slower but more reliable delivery, message can be lost if a star crashes, etc
    Immediate, // Wave should never touch a filesystem, it will be in memory for its entire journey for immediate processing
}

impl Default for Handling {
    fn default() -> Self {
        Self {
            kind: HandlingKind::Queued,
            priority: Default::default(),
            retries: Default::default(),
            wait: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum WaitTime {
    High,
    Med,
    Low,
}

impl Default for WaitTime {
    fn default() -> Self {
        WaitTime::Low
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Retries {
    None,
    Max,
    Medium,
    Min,
}

impl Default for Retries {
    fn default() -> Self {
        Retries::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum Priority {
    Hyper,
    Super,
    High,
    Med,
    Low,
}

impl Default for Priority {
    fn default() -> Self {
        Self::Med
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Karma {
    Hyper,
    Super,
    High,
    Med,
    Low,
    None,
}

impl Default for Karma {
    fn default() -> Self {
        Self::High
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Urgency {
    High,
    Low,
}

#[async_trait]
pub trait Router: Send + Sync {
    async fn route(&self, wave: UltraWave);
    fn route_sync(&self, wave: UltraWave);
}

#[derive(Clone)]
pub struct TxRouter {
    pub tx: mpsc::Sender<UltraWave>,
}

impl TxRouter {
    pub fn new(tx: mpsc::Sender<UltraWave>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl Router for TxRouter {
    async fn route(&self, wave: UltraWave) {
        self.tx.send(wave).await;
    }

    fn route_sync(&self, wave: UltraWave) {
        self.tx.try_send(wave);
    }
}

#[async_trait]
impl Router for BroadTxRouter {
    async fn route(&self, wave: UltraWave) {
        self.tx.send(wave);
    }

    fn route_sync(&self, wave: UltraWave) {
        self.tx.send(wave);
    }
}

#[derive(Clone)]
pub struct BroadTxRouter {
    pub tx: broadcast::Sender<UltraWave>,
}

impl BroadTxRouter {
    pub fn new(tx: broadcast::Sender<UltraWave>) -> Self {
        Self { tx }
    }
}

pub struct DirectedHandlerShell<D>
where
    D: DirectedHandler,
{
    logger: PointLogger,
    handler: D,
    port: Port,
    builder: ProtoTransmitterBuilder,
}

impl<D> DirectedHandlerShell<D>
where
    D: DirectedHandler,
{
    pub fn new(
        handler: D,
        builder: ProtoTransmitterBuilder,
        port: Port,
        logger: RootLogger,
    ) -> Self {
        let logger = logger.point(port.point.clone());
        Self {
            handler,
            builder,
            port,
            logger,
        }
    }

    pub async fn handle(&self, wave: DirectedWave) {
        let logger = self
            .logger
            .point(self.port.clone().to_point())
            .spanner(&wave);
        let mut transmitter = self.builder.clone().build();
        let reflection = wave.reflection();
        let ctx = RootInCtx::new(wave, self.port.clone(), logger, transmitter);
        match self.handler.handle(ctx).await {
            CoreBounce::Absorbed => {}
            CoreBounce::Reflected(reflected) => {
                let wave = reflection.unwrap().make(reflected, self.port.clone());
                let wave = wave.to_ultra();
                let transmitter = self.builder.clone().build();
                transmitter.route(wave).await;
            }
        }
    }
}




pub trait TransportPlanner {
    fn dest(&self, port: Port) -> Port;
}

pub struct InternalPipeline<H> {
    pub selector: RouteSelector,
    pub handler: H,
}

impl<H> InternalPipeline<H> {
    pub fn new(selector: RouteSelector, mut handler: H) -> Self {
        Self { selector, handler }
    }
}

#[async_trait]
pub trait DirectedHandlerSelector {
    fn select<'a>(&self, select: &'a RecipientSelector<'a>) -> Result<&dyn DirectedHandler, ()>;
}

#[async_trait]
pub trait DirectedHandler: Send+Sync {
    async fn handle(&self, ctx: RootInCtx) -> CoreBounce;
}

pub type CoreBounce = Bounce<ReflectedCore>;

pub enum Bounce<W> {
    Absorbed,
    Reflected(W),
}

impl<W> Bounce<W> {
    pub fn to_core_bounce(self) -> CoreBounce
    where
        W: TryInto<ReflectedCore, Error = MsgErr>,
    {
        match self {
            Bounce::Absorbed => Bounce::Absorbed,
            Bounce::Reflected(reflected) => match reflected.try_into() {
                Ok(reflected) => CoreBounce::Reflected(reflected),
                Err(err) => CoreBounce::Reflected(err.as_reflected_core()),
            },
        }
    }
}

impl Into<CoreBounce> for Bounce<ReflectedWave> {
    fn into(self) -> CoreBounce {
        match self {
            Bounce::Absorbed => CoreBounce::Absorbed,
            Bounce::Reflected(reflected) => CoreBounce::Reflected(reflected.core().clone()),
        }
    }
}

pub enum BounceProto {
    Absorbed,
    Reflected(ReflectedProto),
}

/*
#[derive(Clone)]
pub struct PointRequestHandler<H> {
    point: Point,
    pipelines: Arc<RwLock<Vec<InternalPipeline<H>>>>,
}

impl<H> PointRequestHandler<H> {
    pub fn new(point: Point) -> Self {
        Self {
            point,
            pipelines: Arc::new(RwLock::new(vec![])),
        }
    }

    pub async fn add(&self, selector: RouteSelector, handler: H) {
        let mut write = self.pipelines.write().await;
        let pipeline = InternalPipeline { selector, handler };
        write.push(pipeline);
    }

    pub async fn remove_topic(&mut self, topic: Option<ValuePattern<Topic>>) {
        let mut write = self.pipelines.write().await;
        write.retain(|p| p.selector.topic != topic)
    }
}

#[async_trait]
impl DirectedHandlerSelector for PointRequestHandler<AsyncRequestHandlerRelay> {
    async fn select<'a>(
        &self,
        select: &'a RecipientSelector<'a>,
    ) -> Result<&dyn DirectedHandler, ()> {
        let read = self.pipelines.read().await;
        for pipeline in read.iter() {
            if pipeline.selector.is_match(select).is_ok() {
                return pipeline.handler.select(request).await;
            }
        }
        Err(())
    }

    /*
    async fn handle(&self, ctx: RootInCtx) -> ReflectedWave
    where
        V: DirectedWaveVariant,
    {
        for port in ctx.wave.to().select_ports(&self.point) {
            let select = RecipientSelector::new(port, &ctx.wave);
            let read = self.pipelines.read().await;
            for pipeline in read.iter() {
                if pipeline.selector.is_match(&ctx.wave).is_ok() {
                    return pipeline.handler.handle(ctx).await;
                }
            }
        }
        ctx.not_found()
    }

     */
}


 */

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
    Eq,
    PartialEq,
)]
pub enum MethodKind {
    Sys,
    Cmd,
    Msg,
    Http,
}

impl ValueMatcher<MethodKind> for MethodKind {
    fn is_match(&self, x: &MethodKind) -> Result<(), ()> {
        if self == x {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl From<Result<ReflectedCore, MsgErr>> for ReflectedCore {
    fn from(result: Result<ReflectedCore, MsgErr>) -> Self {
        match result {
            Ok(response) => response,
            Err(err) => err.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ReflectedCore {
    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,

    #[serde(with = "http_serde::status_code")]
    pub status: StatusCode,

    pub body: Substance,
}

impl<S> ToSubstance<S> for ReflectedCore
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.body.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.body.to_substance_ref()
    }
}

impl ReflectedCore {
    pub fn ok_html(html: &str) -> Self {
        let bin = Arc::new(html.to_string().into_bytes());
        ReflectedCore::ok_body(Substance::Bin(bin))
    }

    pub fn new() -> Self {
        ReflectedCore {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn ok() -> Self {
        Self::ok_body(Substance::Empty)
    }

    pub fn ok_body(body: Substance) -> Self {
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body,
        }
    }

    pub fn timeout() -> Self {
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(408u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn server_error() -> Self {
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn status(status: u16) -> Self {
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::from_u16(500).unwrap()),
            body: Substance::Empty,
        }
    }

    pub fn not_found() -> Self {
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(404u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn forbidden() -> Self {
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(403u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn bad_request() -> Self {
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(400u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn fail(status: u16, message: &str) -> Self {
        let errors = Errors::default(message.clone());
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(status)
                .or_else(|_| StatusCode::from_u16(500u16))
                .unwrap(),
            body: Substance::Errors(errors),
        }
    }

    pub fn err(err: MsgErr) -> Self {
        let errors = Errors::default(err.to_string().as_str());
        Self {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(err.status())
                .unwrap_or(StatusCode::from_u16(500u16).unwrap()),
            body: Substance::Errors(errors),
        }
    }

    pub fn with_new_substance(self, substance: Substance) -> Self {
        Self {
            headers: self.headers,
            status: self.status,
            body: substance,
        }
    }

    pub fn is_ok(&self) -> bool {
        self.status.is_success()
    }

    pub fn into_reflection<P>(self, intended: Port, to: P, reflection_of: WaveId) -> Pong
    where
        P: ToPort,
    {
        Pong {
            to: to.to_port(),
            intended: intended.to_recipients(),
            core: self,
            reflection_of: reflection_of,
        }
    }
}

impl ReflectedCore {
    pub fn as_result<E: From<&'static str>, P: TryFrom<Substance>>(self) -> Result<P, E> {
        if self.status.is_success() {
            match P::try_from(self.body) {
                Ok(substance) => Ok(substance),
                Err(err) => Err(E::from("error")),
            }
        } else {
            Err(E::from("error"))
        }
    }
}

impl TryInto<http::response::Builder> for ReflectedCore {
    type Error = MsgErr;

    fn try_into(self) -> Result<http::response::Builder, Self::Error> {
        let mut builder = http::response::Builder::new();

        for (name, value) in self.headers {
            match name {
                Some(name) => {
                    builder = builder.header(name.as_str(), value.to_str()?.to_string().as_str());
                }
                None => {}
            }
        }

        Ok(builder.status(self.status))
    }
}

impl TryInto<http::Response<Bin>> for ReflectedCore {
    type Error = MsgErr;

    fn try_into(self) -> Result<http::Response<Bin>, Self::Error> {
        let mut builder = http::response::Builder::new();

        for (name, value) in self.headers {
            match name {
                Some(name) => {
                    builder = builder.header(name.as_str(), value.to_str()?.to_string().as_str());
                }
                None => {}
            }
        }

        let response = builder.status(self.status).body(self.body.to_bin()?)?;
        Ok(response)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Autobox)]
pub enum Method {
    Sys(SysMethod),
    Cmd(CmdMethod),
    Http(HttpMethod),
    Msg(MsgMethod),
}

impl Method {
    pub fn to_deep_string(&self) -> String {
        match self {
            Method::Sys(x) => format!("Sys<{}>",x.to_string()),
            Method::Cmd(x) => format!("Cmd<{}>",x.to_string()),
            Method::Http(x) => format!("Http<{}>",x.to_string()),
            Method::Msg(x) => format!("Msg<{}>",x.to_string()),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum MethodPattern {
    Sys(ValuePattern<SysMethod>),
    Cmd(ValuePattern<CmdMethod>),
    Http(ValuePattern<HttpMethod>),
    Msg(ValuePattern<MsgMethod>),
}

impl ToString for MethodPattern {
    fn to_string(&self) -> String {
        match self {
            MethodPattern::Cmd(c) => {
                format!("Cmd<{}>", c.to_string())
            }
            MethodPattern::Http(c) => {
                format!("Http<{}>", c.to_string())
            }
            MethodPattern::Msg(c) => {
                format!("Msg<{}>", c.to_string())
            }
            MethodPattern::Sys(c) => {
                format!("Sys<{}>", c.to_string())
            }
        }
    }
}

impl ValueMatcher<Method> for MethodPattern {
    fn is_match(&self, x: &Method) -> Result<(), ()> {
        match self {
            MethodPattern::Sys(pattern) => {
                if let Method::Sys(v) = x {
                    pattern.is_match(v)
                } else {
                    Err(())
                }
            }
            MethodPattern::Cmd(pattern) => {
                if let Method::Cmd(v) = x {
                    pattern.is_match(v)
                } else {
                    Err(())
                }
            }
            MethodPattern::Http(pattern) => {
                if let Method::Http(v) = x {
                    pattern.is_match(v)
                } else {
                    Err(())
                }
            }
            MethodPattern::Msg(pattern) => {
                if let Method::Msg(v) = x {
                    pattern.is_match(v)
                } else {
                    Err(())
                }
            }
        }
    }
}

impl ValueMatcher<Method> for Method {
    fn is_match(&self, x: &Method) -> Result<(), ()> {
        if x == self {
            Ok(())
        } else {
            Err(())
        }
    }
}

impl Method {
    pub fn kind(&self) -> MethodKind {
        match self {
            Method::Cmd(_) => MethodKind::Cmd,
            Method::Http(_) => MethodKind::Http,
            Method::Msg(_) => MethodKind::Msg,
            Method::Sys(_) => MethodKind::Sys,
        }
    }
}

impl ToString for Method {
    fn to_string(&self) -> String {
        match self {
            Method::Cmd(cmd) => format!("Cmd<{}>", cmd.to_string()),
            Method::Http(method) => format!("Http<{}>", method.to_string()),
            Method::Msg(msg) => format!("Msg<{}>", msg.to_string()),
            Method::Sys(sys) => format!("Sys<{}>", sys.to_string()),
        }
    }
}

impl Into<DirectedCore> for Method {
    fn into(self) -> DirectedCore {
        DirectedCore {
            headers: Default::default(),
            method: self,
            uri: Uri::from_static("/"),
            body: Substance::Empty,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct DirectedCore {
    #[serde(with = "http_serde::header_map")]
    pub headers: HeaderMap,
    pub method: Method,
    #[serde(with = "http_serde::uri")]
    pub uri: Uri,
    pub body: Substance,
}

impl<S> ToSubstance<S> for DirectedCore
where
    Substance: ToSubstance<S>,
{
    fn to_substance(self) -> Result<S, MsgErr> {
        self.body.to_substance()
    }

    fn to_substance_ref(&self) -> Result<&S, MsgErr> {
        self.body.to_substance_ref()
    }
}

impl DirectedCore {
    pub fn new(method: Method) -> Self {
        Self {
            method,
            headers: HeaderMap::new(),
            uri: Default::default(),
            body: Default::default(),
        }
    }

    pub fn msg<M: Into<MsgMethod>>(method: M) -> Self {
        let method: MsgMethod = method.into();
        let method: Method = method.into();
        Self::new(method)
    }

    pub fn http<M: Into<HttpMethod>>(method: M) -> Self {
        let method: HttpMethod = method.into();
        let method: Method = method.into();
        Self::new(method)
    }

    pub fn cmd<M: Into<CmdMethod>>(method: M) -> Self {
        let method: CmdMethod = method.into();
        let method: Method = method.into();
        Self::new(method)
    }
}

impl TryFrom<Ping> for DirectedCore {
    type Error = MsgErr;

    fn try_from(request: Ping) -> Result<Self, Self::Error> {
        Ok(request.core)
    }
}

impl DirectedCore {
    pub fn kind(&self) -> MethodKind {
        self.method.kind()
    }
}

impl Into<DirectedCore> for Command {
    fn into(self) -> DirectedCore {
        DirectedCore {
            body: Substance::Command(Box::new(self)),
            method: Method::Msg(MsgMethod::new("Command").unwrap()),
            ..Default::default()
        }
    }
}

impl TryFrom<http::Request<Bin>> for DirectedCore {
    type Error = MsgErr;

    fn try_from(request: http::Request<Bin>) -> Result<Self, Self::Error> {
        Ok(Self {
            headers: request.headers().clone(),
            method: Method::Http(request.method().clone().try_into()?),
            uri: request.uri().clone(),
            body: Substance::Bin(request.body().clone()),
        })
    }
}

impl TryInto<http::Request<Bin>> for DirectedCore {
    type Error = MsgErr;

    fn try_into(self) -> Result<http::Request<Bin>, MsgErr> {
        let mut builder = http::Request::builder();
        for (name, value) in self.headers {
            match name {
                Some(name) => {
                    builder = builder.header(name.as_str(), value.to_str()?.to_string().as_str());
                }
                None => {}
            }
        }
        match self.method {
            Method::Http(method) => {
                builder = builder.method(method).uri(self.uri);
                Ok(builder.body(self.body.to_bin()?)?)
            }
            _ => Err("cannot convert to http response".into()),
        }
    }
}

impl Default for DirectedCore {
    fn default() -> Self {
        Self {
            headers: Default::default(),
            method: Method::Http(HttpMethod::Get),
            uri: Uri::from_static("/"),
            body: Substance::Empty,
        }
    }
}

impl DirectedCore {
    pub fn with_body(self, body: Substance) -> Self {
        Self {
            headers: self.headers,
            uri: self.uri,
            method: self.method,
            body,
        }
    }

    pub fn server_error(&self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(500u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn timeout(&self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(408u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn not_found(&self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(404u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn forbidden(&self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(403u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn bad_request(&self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(400u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn substance(method: Method, body: Substance) -> DirectedCore {
        DirectedCore {
            method,
            body,
            ..Default::default()
        }
    }

    pub fn ok(&self) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn ok_body(&self, body: Substance) -> ReflectedCore {
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body,
        }
    }

    pub fn fail<M: ToString>(&self, status: u16, message: M) -> ReflectedCore {
        let errors = Errors::default(message.to_string().as_str());
        ReflectedCore {
            headers: Default::default(),
            status: StatusCode::from_u16(status)
                .or_else(|_| StatusCode::from_u16(500u16))
                .unwrap(),
            body: Substance::Errors(errors),
        }
    }

    pub fn err<E: StatusErr>(&self, error: E) -> ReflectedCore {
        let errors = Errors::default(error.message().as_str());
        let status = match StatusCode::from_u16(error.status()) {
            Ok(status) => status,
            Err(_) => StatusCode::from_u16(500u16).unwrap(),
        };
        println!("----->   returning STATUS of {}", status.as_str());
        ReflectedCore {
            headers: Default::default(),
            status,
            body: Substance::Errors(errors),
        }
    }
}

impl Into<ReflectedCore> for Port {
    fn into(self) -> ReflectedCore {
        ReflectedCore::ok_body(Substance::Port(self))
    }
}

impl TryFrom<ReflectedCore> for Port {
    type Error = MsgErr;

    fn try_from(core: ReflectedCore) -> Result<Self, Self::Error> {
        if !core.status.is_success() {
            Err(MsgErr::new(core.status.as_u16(), "error"))
        } else {
            match core.body {
                Substance::Port(port) => Ok(port),
                substance => {
                    Err(format!("expecting Port received {}", substance.kind().to_string()).into())
                }
            }
        }
    }
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum CmdMethod {
    Read,
    Update,
    Bounce,
    Knock,
    Greet,
    Command,
    RawCommand,
}

impl ValueMatcher<CmdMethod> for CmdMethod {
    fn is_match(&self, x: &CmdMethod) -> Result<(), ()> {
        if *x == *self {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum SysMethod {
    Command,
    Assign,
    AssignPort,
    Knock,
    Hop,
    Transport,
    HyperWave,
    Search,
}

impl ValueMatcher<SysMethod> for SysMethod {
    fn is_match(&self, x: &SysMethod) -> Result<(), ()> {
        if *x == *self {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[derive(Clone)]
pub enum SetStrategy<T> {
    None,
    Fill(T),
    Override(T),
}

impl<T> SetStrategy<T> {
    pub fn unwrap(self) -> Result<T, MsgErr> {
        match self {
            SetStrategy::None => Err("cannot unwrap a SetStrategy::None".into()),
            SetStrategy::Fill(t) => Ok(t),
            SetStrategy::Override(t) => Ok(t),
        }
    }
}

impl SetStrategy<Port> {
    pub fn with_topic(self, topic: Topic) -> Result<Self, MsgErr> {
        match self {
            SetStrategy::None => Err("cannot set topic if Strategy is None".into()),
            SetStrategy::Fill(port) => Ok(SetStrategy::Fill(port.with_topic(topic))),
            SetStrategy::Override(port) => Ok(SetStrategy::Override(port.with_topic(topic))),
        }
    }
}

pub enum ReflectedAggregate {
    None,
    Single(ReflectedWave),
    Multi(Vec<ReflectedWave>),
}

pub trait FromReflectedAggregate {
    fn from_reflected_aggregate(agg: ReflectedAggregate) -> Result<Self, MsgErr>
    where
        Self: Sized;
}

impl TryInto<Wave<Pong>> for ReflectedAggregate {
    type Error = MsgErr;
    fn try_into(self) -> Result<Wave<Pong>, Self::Error> {
        match self {
            Self::Single(reflected) => match reflected {
                ReflectedWave::Pong(pong) => Ok(pong),
                _ => Err(MsgErr::bad_request()),
            },
            _ => Err(MsgErr::bad_request()),
        }
    }
}

impl TryInto<Vec<Wave<Echo>>> for ReflectedAggregate {
    type Error = MsgErr;
    fn try_into(self) -> Result<Vec<Wave<Echo>>, Self::Error> {
        match self {
            Self::Single(reflected) => match reflected {
                ReflectedWave::Echo(echo) => Ok(vec![echo]),
                _ => Err(MsgErr::bad_request()),
            },
            ReflectedAggregate::None => Ok(vec![]),
            ReflectedAggregate::Multi(waves) => {
                let mut echoes = vec![];
                for w in waves {
                    echoes.push(w.to_echo()?);
                }
                Ok(echoes)
            }
        }
    }
}

#[derive(Clone)]
pub struct Exchanger {
    pub port: Port,
    pub multis: Arc<DashMap<WaveId, mpsc::Sender<ReflectedWave>>>,
    pub singles: Arc<DashMap<WaveId, oneshot::Sender<ReflectedAggregate>>>,
    pub timeouts: Timeouts,
}

impl Exchanger {
    pub fn new(port: Port, timeouts: Timeouts) -> Self {
        Self {
            port,
            singles: Arc::new(DashMap::new()),
            multis: Arc::new(DashMap::new()),
            timeouts,
        }
    }

    pub fn with_port(&self, port: Port) -> Self {
        Self {
            port,
            singles: self.singles.clone(),
            multis: self.multis.clone(),
            timeouts: self.timeouts.clone(),
        }
    }

    pub async fn reflected(&self, reflect: ReflectedWave) -> Result<(),MsgErr>{
        if let Some(multi) = self.multis.get(reflect.reflection_of()) {
            multi.value().send(reflect).await;
        } else if let Some((_, tx)) = self.singles.remove(reflect.reflection_of()) {
            tx.send(ReflectedAggregate::Single(reflect));
        } else {
            let reflect = reflect.to_ultra();
            let kind = match &reflect {
                UltraWave::Ping(_) => "Ping",
                UltraWave::Pong(_) => "Pong",
                UltraWave::Ripple(_) => "Ripple",
                UltraWave::Echo(_) => "Echo",
                UltraWave::Signal(_) => "Signal"
            };
            let reflect = reflect.to_reflected()?;

            return Err(MsgErr::from_500(format!("Not expecting reflected message from: {} to: {} KIND: {} STATUS: {}", reflect.from().to_string(), reflect.to().to_string(), kind, reflect.core().status.to_string())));
        }
        Ok(())
    }

    pub async fn exchange(&self, directed: &DirectedWave) -> oneshot::Receiver<ReflectedAggregate> {
        let (tx, rx) = oneshot::channel();

        let mut reflected = match directed.reflected_proto() {
            BounceProto::Absorbed => {
                return rx;
            }
            BounceProto::Reflected(reflected) => reflected,
        };

        reflected.from(self.port.clone());

        let reflection = directed.reflection().unwrap();

        let timeout = self.timeouts.from(directed.handling().wait.clone());
        self.singles.insert(directed.id().clone(), tx);
        match directed.bounce_backs() {
            BounceBacks::None => {
                panic!("we already dealt with this")
            }
            BounceBacks::Single => {
                let singles = self.singles.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(timeout)).await;
                    let id = reflected.reflection_of.as_ref().unwrap();
                    if let Some((_, tx)) = singles.remove(id) {
                        reflected.status = Some(StatusCode::from_u16(408).unwrap());
                        reflected.body = Some(Substance::Empty);
                        reflected.intended = Some(reflection.intended);
                        let reflected = reflected.build().unwrap();
                        tx.send(ReflectedAggregate::Single(reflected));
                    }
                });
            }
            BounceBacks::Count(count) => {
                let (tx, mut rx) = mpsc::channel(count);
                self.multis.insert(directed.id().clone(), tx);
                let singles = self.singles.clone();
                let id = directed.id().clone();
                tokio::spawn(async move {
                    let mut agg = vec![];
                    loop {
                        if let Some(reflected) = rx.recv().await {
                            agg.push(reflected);
                            if count == agg.len() {
                                if let Some((_, tx)) = singles.remove(&id) {
                                    tx.send(ReflectedAggregate::Multi(agg));
                                    break;
                                }
                            }
                        } else {
                            // this would occur in a timeout scenario
                            if let Some((_, tx)) = singles.remove(&id) {
                                reflected.status = Some(StatusCode::from_u16(408).unwrap());
                                reflected.body = Some(Substance::Empty);
                                reflected.intended = Some(reflection.intended);
                                let reflected = reflected.build().unwrap();
                                tx.send(ReflectedAggregate::Multi(vec![reflected]));
                                break;
                            }
                        }
                    }
                });

                let id = directed.id().clone();
                let multis = self.multis.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(timeout)).await;
                    // all we have to do is remove it, the multi loop will take care of the rest
                    multis.remove(&id);
                });
            }
            BounceBacks::Timer(wait) => {
                let (tx, mut rx) = mpsc::channel(32);
                self.multis.insert(directed.id().clone(), tx);
                let singles = self.singles.clone();
                let id = directed.id().clone();
                tokio::spawn(async move {
                    let mut agg = vec![];
                    loop {
                        if let Some(reflected) = rx.recv().await {
                            agg.push(reflected);
                        } else {
                            // this would occur in a timeout scenario
                            if let Some((_, tx)) = singles.remove(&id) {
                                tx.send(ReflectedAggregate::Multi(agg));
                                break;
                            }
                        }
                    }
                });

                let id = directed.id().clone();
                let multis = self.multis.clone();
                let timeout = self.timeouts.from(wait);
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(timeout)).await;
                    // all we have to do is remove it, the multi loop will take care of the rest
                    multis.remove(&id);
                });
            }
        }

        rx
    }
}

impl Default for Exchanger {
    fn default() -> Self {
        Self::new(Point::root().to_port(), Default::default())
    }
}

#[derive(Clone)]
pub struct ProtoTransmitterBuilder {
    pub agent: SetStrategy<Agent>,
    pub scope: SetStrategy<Scope>,
    pub handling: SetStrategy<Handling>,
    pub from: SetStrategy<Port>,
    pub to: SetStrategy<Recipients>,
    pub router: Arc<dyn Router>,
    pub exchanger: Exchanger,
}

impl ProtoTransmitterBuilder {
    pub fn new(router: Arc<dyn Router>, exchanger: Exchanger) -> ProtoTransmitterBuilder {
        Self {
            from: SetStrategy::None,
            to: SetStrategy::None,
            agent: SetStrategy::Fill(Agent::Anonymous),
            scope: SetStrategy::Fill(Scope::None),
            handling: SetStrategy::Fill(Handling::default()),
            router,
            exchanger,
        }
    }

    pub fn build(self) -> ProtoTransmitter {
        ProtoTransmitter {
            agent: self.agent,
            scope: self.scope,
            handling: self.handling,
            from: self.from,
            to: self.to,
            router: self.router,
            exchanger: self.exchanger,
        }
    }
}

#[derive(Clone)]
pub struct ProtoTransmitter {
    agent: SetStrategy<Agent>,
    scope: SetStrategy<Scope>,
    handling: SetStrategy<Handling>,
    from: SetStrategy<Port>,
    to: SetStrategy<Recipients>,
    router: Arc<dyn Router>,
    exchanger: Exchanger,
}

impl ProtoTransmitter {
    pub fn new(router: Arc<dyn Router>, exchanger: Exchanger) -> ProtoTransmitter {
        Self {
            from: SetStrategy::None,
            to: SetStrategy::None,
            agent: SetStrategy::Fill(Agent::Anonymous),
            scope: SetStrategy::Fill(Scope::None),
            handling: SetStrategy::Fill(Handling::default()),
            router,
            exchanger,
        }
    }

    pub fn from_topic(&mut self, topic: Topic) -> Result<(), MsgErr> {
        self.from = match self.from.clone() {
            SetStrategy::None => {
                return Err(MsgErr::from_500(
                    "cannot set Topic without first setting Port",
                ));
            }
            SetStrategy::Fill(from) => SetStrategy::Fill(from.with_topic(topic)),
            SetStrategy::Override(from) => SetStrategy::Override(from.with_topic(topic)),
        };
        Ok(())
    }

    pub async fn direct<D, W>(&self, wave: D) -> Result<W, MsgErr>
    where
        W: FromReflectedAggregate,
        D: Into<DirectedProto>,
    {
        let mut wave: DirectedProto = wave.into();

        match &self.from {
            SetStrategy::None => {}
            SetStrategy::Fill(from) => wave.fill_from(from.clone()),
            SetStrategy::Override(from) => wave.from(from.clone()),
        }

        match &self.to {
            SetStrategy::None => {}
            SetStrategy::Fill(to) => wave.fill_to(to.clone()),
            SetStrategy::Override(to) => wave.to(to),
        }

        match &self.agent {
            SetStrategy::None => {}
            SetStrategy::Fill(agent) => wave.fill_agent(agent),
            SetStrategy::Override(agent) => wave.agent(agent.clone()),
        }

        match &self.scope {
            SetStrategy::None => {}
            SetStrategy::Fill(scope) => wave.fill_scope(scope),
            SetStrategy::Override(scope) => wave.scope(scope.clone()),
        }

        match &self.handling {
            SetStrategy::None => {}
            SetStrategy::Fill(handling) => wave.fill_handling(handling),
            SetStrategy::Override(handling) => wave.handling(handling.clone()),
        }

        let directed = wave.build()?;

        match directed.bounce_backs() {
            BounceBacks::None => {
                self.router.route(directed.to_ultra()).await;
                FromReflectedAggregate::from_reflected_aggregate(ReflectedAggregate::None)
            }
            _ => {
                let reflected_rx = self.exchanger.exchange(&directed).await;
                self.router.route(directed.to_ultra()).await;
                let reflected_agg = reflected_rx.await?;
                FromReflectedAggregate::from_reflected_aggregate(reflected_agg)
            }
        }
    }

    pub fn route_sync(&self, wave: UltraWave) {
        self.router.route_sync(wave)
    }

    pub async fn route(&self, wave: UltraWave) {
        self.router.route(wave).await
    }

    pub async fn reflect<W>(&self, wave: W) -> Result<(), MsgErr>
    where
        W: Into<ReflectedProto>,
    {
        let mut wave: ReflectedProto = wave.into();

        match &self.from {
            SetStrategy::None => {}
            SetStrategy::Fill(from) => wave.fill_from(from),
            SetStrategy::Override(from) => wave.from(from.clone()),
        }

        match &self.agent {
            SetStrategy::None => {}
            SetStrategy::Fill(agent) => wave.fill_agent(agent),
            SetStrategy::Override(agent) => wave.agent(agent.clone()),
        }

        match &self.scope {
            SetStrategy::None => {}
            SetStrategy::Fill(scope) => wave.fill_scope(scope),
            SetStrategy::Override(scope) => wave.scope(scope.clone()),
        }

        match &self.handling {
            SetStrategy::None => {}
            SetStrategy::Fill(handling) => wave.fill_handling(handling),
            SetStrategy::Override(handling) => wave.handling(handling.clone()),
        }

        let wave = wave.build()?;
        let wave = wave.to_ultra();
        self.router.route(wave).await;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct HyperWave {
    pub from: Point,
    pub wave: UltraWave,
}

impl HyperWave {
    pub fn to(&self) -> Recipients {
        self.wave.to()
    }

    pub fn from(&self) -> &Port {
        self.wave.from()
    }
}

#[derive(Clone)]
pub struct Delivery {
    pub to: Port,
    pub wave: DirectedWave,
}

impl Delivery {
    pub fn new(to: Port, wave: DirectedWave) -> Self {
        Self { to, wave }
    }
}

impl Deref for Delivery {
    type Target = DirectedWave;

    fn deref(&self) -> &Self::Target {
        &self.wave
    }
}
