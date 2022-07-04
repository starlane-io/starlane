use crate::bin::Bin;
use crate::cli::RawCommand;
use crate::command::Command;
use crate::config::config::bind::RouteSelector;
use crate::error::{MsgErr, StatusErr};
use crate::http::HttpMethod;
use crate::id::id::{Layer, Point, Port, PortSelector, Sub, ToPoint, ToPort, Topic, Uuid};
use crate::log::{LogSpan, LogSpanEvent, PointLogger, SpanLogger, TrailSpanId};
use crate::msg::MsgMethod;
use crate::parse::model::Subst;
use crate::parse::sub;
use crate::particle::particle::{Details, Status};
use crate::quota::Timeouts;
use crate::security::{Permissions, Privilege, Privileges};
use crate::selector::selector::Selector;
use crate::substance::substance::{
    Call, CallKind, Errors, HttpCall, MsgCall, MultipartFormBuilder, SubstanceKind, ToRequestCore,
    Token,
};
use crate::substance::substance::{Substance, ToSubstance};
use crate::sys::AssignmentKind;
use crate::util::{uuid, ValueMatcher, ValuePattern};
use alloc::borrow::Cow;
use core::borrow::Borrow;
use cosmic_macros_primitive::Autobox;
use cosmic_nom::{Res, SpanExtra};
use dashmap::DashMap;
use http::{HeaderMap, StatusCode, Uri};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::marker::PhantomData;
use std::ops;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, RwLock};
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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum UltraWave {
    Ping(Wave<Ping>),
    Pong(Wave<Pong>),
    Ripple(Wave<Ripple>),
    Echo(Wave<Echo>),
    Signal(Wave<Signal>),
}

impl UltraWave {
    pub fn to_substance(self) -> Substance {
        Substance::UltraWave(Box::new(self))
    }

    pub fn to_directed(self) -> Result<DirectedWave,MsgErr> {
        match self {
            UltraWave::Ping(ping) => Ok(ping.to_directed()),
            UltraWave::Ripple(ripple) => Ok(ripple.to_directed()),
            UltraWave::Signal(signal) => Ok(signal.to_directed()),
            _ => Err(MsgErr::bad_request())
        }
    }

    pub fn to_signal(self) -> Result<Wave<Signal>,MsgErr> {
        match self {
            UltraWave::Signal(signal) => Ok(signal),
            _ => Err(MsgErr::bad_request())
        }
    }


    pub fn method(&self) -> Option<&Method> {
        match self {
            UltraWave::Ping(ping) => Some(&ping.method),
            UltraWave::Ripple(ripple) =>  Some(&ripple.method),
            UltraWave::Signal(signal) =>  Some(&signal.method),
            _ => None
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

    pub fn to_ripple(self) -> Result<Wave<Ripple>,MsgErr> {
        match self {
            UltraWave::Ripple(ripple) => Ok(ripple),
            _ => Err("not a ripple".into())
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
}

impl ToString for WaveId {
    fn to_string(&self) -> String {
        format!("<Wave<{}>>/{}", self.kind.to_string(), self.uuid)
    }
}

pub struct RootInCtx {
    pub to: Port,
    pub wave: DirectedWave,
    pub session: Option<Session>,
    pub logger: SpanLogger,
    pub tx: ProtoTransmitter,
}

impl RootInCtx {
    pub fn new(wave: DirectedWave, to: Port, logger: SpanLogger, tx: ProtoTransmitter) -> Self {
        Self {
            wave,
            to,
            logger,
            session: None,
            tx,
        }
    }

    pub fn status(self, status: u16, from: Port) -> Bounce<ReflectedWave> {
        match self.wave {
            DirectedWave::Ping(ping) => Bounce::Reflected(ReflectedWave::Pong(Wave::new(
                Pong::new(
                    ReflectedCore::status(status),
                    ping.from.clone(),
                    self.to.clone(),
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
            tx: Cow::Borrowed(&self.tx),
        })
    }
}

pub struct InCtx<'a, I> {
    root: &'a RootInCtx,
    pub tx: Cow<'a, ProtoTransmitter>,
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
            tx,
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
            tx: self.tx.clone(),
        }
    }

    pub fn push_from(self, from: Port) -> InCtx<'a, I> {
        let mut tx = self.tx.clone();
        tx.to_mut().from = SetStrategy::Override(from);
        InCtx {
            root: self.root,
            input: self.input,
            logger: self.logger.clone(),
            tx,
        }
    }

    pub fn push_input_ref<I2>(self, input: &'a I2) -> InCtx<'a, I2> {
        InCtx {
            root: self.root,
            input,
            logger: self.logger.clone(),
            tx: self.tx.clone(),
        }
    }

    pub fn wave(&self) -> &DirectedWave {
        &self.root.wave
    }

    pub async fn ping(&self, req: DirectedProto) -> Result<Wave<Pong>, MsgErr> {
        self.tx.direct(req).await
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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Ripple {
    pub to: Recipients,
    pub core: DirectedCore,
    pub bounce_backs: BounceBacks,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum BounceBacks {
    None,
    Single,
    Count(usize),
    Timer(WaitTime),
}

impl<S> ToSubstance<S> for Ripple
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

impl Ripple {
    pub fn require_method<M: Into<Method> + ToString + Clone>(
        self,
        method: M,
    ) -> Result<Ripple, MsgErr> {
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

impl Deref for Ripple {
    type Target = DirectedCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl DerefMut for Ripple {
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
            core: Some(self.core.clone()),
            id: self.id,
            from: Some(self.from),
            handling: Some(self.handling),
            scope: Some(self.scope),
            agent: Some(self.agent),
            kind: None,
            bounce_backs: None
        }
    }
}

impl Ping {
    pub fn to_call(&self) -> Result<Call, MsgErr> {
        let kind = match &self.core.method {
            Method::Cmd(_) => {
                unimplemented!()
            }
            Method::Sys(_) => {
                unimplemented!()
            }
            Method::Http(method) => CallKind::Http(HttpCall::new(
                method.clone(),
                Subst::new(self.core.uri.path())?,
            )),
            Method::Msg(method) => CallKind::Msg(MsgCall::new(
                method.clone(),
                Subst::new(self.core.uri.path())?,
            )),
        };

        Ok(Call {
            point: self.to.clone().to_point(),
            kind: kind.clone(),
        })
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

    pub fn fill_intended<I: ToRecipients>(&mut self, intended: I) {
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
                let pong = Wave::new(
                    Pong::new(
                        core,
                        self.to.ok_or("ReflectedProto missing to")?,
                        self.intended
                            .ok_or("Reflected Proto Missing intended")?
                            .single_or()?,
                        self.reflection_of.ok_or("response to expected")?,
                    ),
                    self.from.ok_or("expected from")?,
                );
                Ok(pong.to_reflected())
            }
            ReflectedKind::Echo => {
                let echo = Wave::new(
                    Echo::new(
                        core,
                        self.to.ok_or("ReflectedProto missing to")?,
                        self.intended.ok_or("Reflected Proto Missing intended")?,
                        self.reflection_of.ok_or("response to expected")?,
                    ),
                    self.from.ok_or("expected from")?,
                );
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
    pub core: Option<DirectedCore>,
    pub handling: Option<Handling>,
    pub scope: Option<Scope>,
    pub agent: Option<Agent>,
    pub kind: Option<DirectedKind>,
    pub bounce_backs: Option<BounceBacks>,
}

impl DirectedProto {
    pub fn build(self) -> Result<DirectedWave, MsgErr> {
        let kind = self
            .kind
            .ok_or::<MsgErr>("kind must be set for DirectedProto to create the proper DirectedWave".into())?;

        let mut wave = match kind {
            DirectedKind::Ping => {
                let mut wave = Wave::new(
                    Ping {
                        to: self
                            .to
                            .ok_or(MsgErr::new(500u16, "must set 'to'"))?
                            .single_or()?,
                        core: self
                            .core
                            .ok_or(MsgErr::new(500u16, "request core must be set"))?,
                    },
                    self.from.ok_or(MsgErr::new(500u16, "must set 'from'"))?,
                );
                wave.agent = self.agent.unwrap_or_else(|| Agent::Anonymous);
                wave.handling = self.handling.unwrap_or_else(|| Handling::default());
                wave.scope = self.scope.unwrap_or_else(|| Scope::None);
                wave.to_directed()
            }
            DirectedKind::Ripple => {
                let mut wave = Wave::new(
                    Ripple {
                        to: self.to.ok_or(MsgErr::new(500u16, "must set 'to'"))?,
                        core: self
                            .core
                            .ok_or(MsgErr::new(500u16, "request core must be set"))?,
                        bounce_backs: self.bounce_backs.ok_or("BounceBacks must be set")?,
                    },
                    self.from.ok_or(MsgErr::new(500u16, "must set 'from'"))?,
                );
                wave.agent = self.agent.unwrap_or_else(|| Agent::Anonymous);
                wave.handling = self.handling.unwrap_or_else(|| Handling::default());
                wave.scope = self.scope.unwrap_or_else(|| Scope::None);
                wave.to_directed()
            }
            DirectedKind::Signal => {
                let mut wave = Wave::new(
                    Signal {
                        to: self
                            .to
                            .ok_or(MsgErr::new(500u16, "must set 'to'"))?
                            .single_or()?,
                        core: self
                            .core
                            .ok_or(MsgErr::new(500u16, "request core must be set"))?,
                    },
                    self.from.ok_or(MsgErr::new(500u16, "must set 'from'"))?,
                );
                wave.agent = self.agent.unwrap_or_else(|| Agent::Anonymous);
                wave.handling = self.handling.unwrap_or_else(|| Handling::default());
                wave.scope = self.scope.unwrap_or_else(|| Scope::None);
                wave.to_directed()
            }
        };

        Ok(wave)
    }

    pub fn fill_kind(&mut self, kind: DirectedKind) {
        if self.kind.is_none() {
            self.kind.replace(kind);
        }
    }

    pub fn fill_to<R: ToRecipients>(&mut self, to: R) {
        if self.to.is_none() {
            self.to.replace(to.to_recipients());
        }
    }

    pub fn fill_from<P: ToPort>(&mut self, from: P) {
        if self.from.is_none() {
            self.from.replace(from.to_port());
        }
    }

    pub fn fill_core(&mut self, core: DirectedCore) {
        if self.core.is_none() {
            self.core.replace(core);
        }
    }

    pub fn fill_scope(&mut self, scope: Scope) {
        if self.scope.is_none() {
            self.scope.replace(scope);
        }
    }

    pub fn fill_agent(&mut self, agent: Agent) {
        if self.agent.is_none() {
            self.agent.replace(agent);
        }
    }

    pub fn fill_handling(&mut self, handling: Handling) {
        if self.handling.is_none() {
            self.handling.replace(handling);
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

    pub fn body(&mut self, body: Substance) -> Result<(), MsgErr> {
        self.core
            .as_mut()
            .ok_or(MsgErr::new(500u16, "core must be set before body"))?
            .body = body;
        Ok(())
    }

    pub fn core(&mut self, core: DirectedCore) -> Result<(), MsgErr> {
        self.core.replace(core);
        Ok(())
    }

    pub fn method<M: Into<Method>>(&mut self, method: M) -> Result<(), MsgErr> {
        let method: Method = method.into();
        if self.core.is_none() {
            self.core = Some(method.into());
        } else {
            self.core.as_mut().unwrap().method = method;
        }
        Ok(())
    }

    pub fn to<P: ToRecipients>(&mut self, to: P) {
        self.to.replace(to.to_recipients());
    }

    pub fn from<P: ToPort>(&mut self, from: P) {
        self.from.replace(from.to_port());
    }
}

impl DirectedProto {
    pub fn new() -> Self {
        Self {
            id: WaveId::new(WaveKind::Ping),
            from: None,
            to: None,
            core: None,
            handling: None,
            scope: None,
            agent: None,
            kind: None,
            bounce_backs: None
        }
    }

    pub fn to_with_method<P: ToRecipients>(to: P, method: Method) -> Self {
        Self {
            id: WaveId::new(WaveKind::Ping),
            from: None,
            to: Some(to.to_recipients()),
            core: Some(DirectedCore::new(method)),
            handling: None,
            scope: None,
            agent: None,
            kind: None,
            bounce_backs: None
        }
    }

    pub fn from_core(core: DirectedCore) -> Self {
        Self {
            id: WaveId::new(WaveKind::Ping),
            from: None,
            to: None,
            core: Some(core),
            handling: None,
            scope: None,
            agent: None,
            kind: None,
            bounce_backs: None
        }
    }

    pub fn sys<M: Into<SysMethod>, P: ToRecipients>(to: P, method: M) -> Self {
        let method: SysMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }

    pub fn msg<M: Into<MsgMethod>, P: ToRecipients>(to: P, method: M) -> Self {
        let method: MsgMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }

    pub fn http<M: Into<HttpMethod>, P: ToRecipients>(to: P, method: M) -> Self {
        let method: HttpMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }

    pub fn cmd<M: Into<CmdMethod>, P: ToRecipients>(to: P, method: M) -> Self {
        let method: CmdMethod = method.into();
        let method: Method = method.into();
        Self::to_with_method(to, method)
    }
}

pub struct Echoes {
    vec: Vec<Wave<Echo>>,
}

impl Echoes {
    pub fn new() -> Self {
        Self { vec: vec![] }
    }
}

impl Deref for Echoes {
    type Target = Vec<Wave<Echo>>;

    fn deref(&self) -> &Self::Target {
        &self.vec
    }
}

impl DerefMut for Echoes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.vec
    }
}


impl FromReflectedAggregate for (){
    fn from_reflected_aggregate(agg: ReflectedAggregate) -> Result<Self, MsgErr> where Self: Sized {
        match agg {
            ReflectedAggregate::None => Ok(()),
            _ => Err(MsgErr::bad_request())
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
    pub intended: Port,
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
}

impl Pong {
    pub fn new(core: ReflectedCore, to: Port, intended: Port, reflection_of: WaveId) -> Self {
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

pub struct RecipientSelector<'a> {
    pub to: &'a Port,
    pub wave: &'a DirectedWave,
}

impl<'a> RecipientSelector<'a> {
    pub fn new(to: &'a Port, wave: &'a Wave<DirectedWave>) -> Self {
        Self { to, wave }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum DirectedWave {
    Ping(Wave<Ping>),
    Ripple(Wave<Ripple>),
    Signal(Wave<Signal>),
}

impl DirectedWave {
    pub fn id(&self) -> &WaveId {
        match self {
            DirectedWave::Ping(ping) => &ping.id,
            DirectedWave::Ripple(ripple) => &ripple.id,
            DirectedWave::Signal(signal) => &signal.id,
        }
    }

    pub fn agent(&self) -> &Agent {
        match self {
            DirectedWave::Ping(ping) => &ping.agent,
            DirectedWave::Ripple(ripple) => &ripple.agent,
            DirectedWave::Signal(signal) => &signal.agent,
        }
    }

    pub fn scope(&self) -> &Scope {
        match self {
            DirectedWave::Ping(ping) => &ping.scope,
            DirectedWave::Ripple(ripple) => &ripple.scope,
            DirectedWave::Signal(signal) => &signal.scope,
        }
    }

    pub fn handling(&self) -> &Handling {
        match self {
            DirectedWave::Ping(ping) => &ping.handling,
            DirectedWave::Ripple(ripple) => &ripple.handling,
            DirectedWave::Signal(signal) => &signal.handling,
        }
    }

    pub fn to(&self) -> Recipients {
        match self {
            DirectedWave::Ping(ping) => ping.to.clone().to_recipients(),
            DirectedWave::Ripple(ripple) => ripple.to.clone(),
            DirectedWave::Signal(signal) => signal.to.clone().to_recipients(),
        }
    }

    pub fn reflection(&self) -> Reflection {
        Reflection {
            from: self.from().clone(),
            to: self.to(),
            reflection_of: self.id().clone(),
        }
    }

    pub fn err(&self, err: MsgErr, responder: Port) -> Bounce<ReflectedWave> {
        match self {
            DirectedWave::Ping(ping) => Bounce::Reflected(ping.err(err, responder).to_reflected()),
            DirectedWave::Ripple(ripple) => {
                Bounce::Reflected(ripple.err(err, responder).to_reflected())
            }
            DirectedWave::Signal(_) => Bounce::Absorbed,
        }
    }

    pub fn to_call(&self) -> Result<Call, MsgErr> {
        match self {
            DirectedWave::Ping(ping) => ping.to_call(),
            _ => Err(MsgErr::not_found()),
        }
    }

    pub fn bounce_backs(&self) -> BounceBacks {
        match self {
            DirectedWave::Ping(ping) => ping.bounce_backs(),
            DirectedWave::Ripple(ripple) => ripple.bounce_backs(),
            DirectedWave::Signal(signal) => signal.bounce_backs(),
        }
    }
}

pub struct Reflection {
    pub from: Port,
    pub to: Recipients,
    pub reflection_of: WaveId,
}

impl Reflection {
    pub fn make(self, core: ReflectedCore, from: Port, intended: Port) -> ReflectedWave {
        Wave::new(
            Pong::new(core, self.from, intended, self.reflection_of),
            from,
        )
        .to_reflected()
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
    pub fn from(&self) -> &Port {
        match self {
            DirectedWave::Ping(ping) => &ping.from,
            DirectedWave::Ripple(ripple) => &ripple.from,
            DirectedWave::Signal(signal) => &signal.from,
        }
    }
    pub fn to_ultra(self) -> UltraWave {
        match self {
            DirectedWave::Ping(ping) => UltraWave::Ping(ping),
            DirectedWave::Ripple(ripple) => UltraWave::Ripple(ripple),
            DirectedWave::Signal(signal) => UltraWave::Signal(signal),
        }
    }

    pub fn body(&self) -> &Substance {
        match self {
            DirectedWave::Ping(ping) => &ping.core.body,
            DirectedWave::Ripple(ripple) => &ripple.core.body,
            DirectedWave::Signal(signal) => &signal.core.body,
        }
    }

    pub fn core(&self) -> &DirectedCore {
        match self {
            DirectedWave::Ping(ping) => &ping.core,
            DirectedWave::Ripple(ripple) => &ripple.core,
            DirectedWave::Signal(signal) => &signal.core,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum ReflectedWave {
    Pong(Wave<Pong>),
    Echo(Wave<Echo>),
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
}

impl ToRecipients for Recipients {
    fn to_recipients(self) -> Recipients {
        self
    }


}

impl Recipients {
    pub fn split(map: HashMap<Point, Vec<Port>> ) -> HashMap<Point,Recipients> {
        let mut rtn = HashMap::new();
        for (point,value) in map {
            rtn.insert( point, Recipients::Multi(value) );
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
        }
        rtn
    }

    pub fn is_single(&self) -> bool {
        match self {
            Recipients::Single(_) => true,
            Recipients::Multi(_) => false,
        }
    }

    pub fn is_multi(&self) -> bool {
        match self {
            Recipients::Single(_) => false,
            Recipients::Multi(_) => true,
        }
    }

    pub fn unwrap_single(self) -> Port {
        self.single_or().expect("single")
    }

    pub fn unwrap_multi(self) -> Vec<Port> {
        match self {
            Recipients::Single(port) => vec![port],
            Recipients::Multi(ports) => ports,
        }
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
    pub star_hops: u16,
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
impl Wave<Ripple> {
    pub fn to_ultra(self) -> UltraWave {
        UltraWave::Ripple(self)
    }

    pub fn to_directed(self) -> DirectedWave {
        DirectedWave::Ripple(self)
    }

    pub fn err(&self, err: MsgErr, responder: Port) -> Wave<Echo> {
        Wave::new(
            Echo::new(
                self.variant.err(err),
                self.from.clone(),
                self.to.clone(),
                self.id.clone(),
            ),
            responder,
        )
    }
}

impl Wave<Signal> {
    pub fn to_ultra(self) -> UltraWave {
        UltraWave::Signal(self)
    }

    pub fn to_directed(self) -> DirectedWave {
        DirectedWave::Signal(self)
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
                self.to.clone(),
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
            star_hops: 0,
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

impl WaveVariant for Ripple {
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

impl Wave<Ripple> {
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
pub trait DirectedHandler {
    async fn handle(&self, ctx: RootInCtx) -> CoreBounce;
}

pub enum CoreBounce {
    Absorbed,
    Reflected(ReflectedCore),
}

pub enum Bounce<W> {
    Absorbed,
    Reflected(W),
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
        ReflectedCore::ok(Substance::Bin(bin))
    }

    pub fn new() -> Self {
        ReflectedCore {
            headers: HeaderMap::new(),
            status: StatusCode::from_u16(200u16).unwrap(),
            body: Substance::Empty,
        }
    }

    pub fn ok(body: Substance) -> Self {
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
        return self.status.is_success();
    }

    pub fn into_reflection<P>(self, intended: Port, to: P, reflection_of: WaveId) -> Pong
    where
        P: ToPort,
    {
        Pong {
            to: to.to_port(),
            intended,
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
            method: Method::Msg(Default::default()),
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
        ReflectedCore::ok(Substance::Port(self))
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
    EntryReq,
    Transport,
    HyperWave,
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

    pub fn with_port(&self, port: Port) -> Exchanger {
        Exchanger {
            port,
            singles: self.singles.clone(),
            multis: self.multis.clone(),
            timeouts: self.timeouts.clone(),
        }
    }

    pub async fn reflected(&self, reflect: ReflectedWave) {
        if let Some(multi) = self.multis.get(reflect.reflection_of()) {
            multi.value().send(reflect).await;
        } else if let Some((_, tx)) = self.singles.remove(reflect.reflection_of()) {
            tx.send(ReflectedAggregate::Single(reflect));
        }
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

        let timeout = self.timeouts.from(directed.handling().wait.clone());
        self.singles.insert(directed.id().clone(), tx);
        match directed.bounce_backs() {
            BounceBacks::None => {
                panic!("we already dealt with this")
            }
            BounceBacks::Single => {
                let singles = self.singles.clone();
                tokio::spawn(async move {
                    tokio::time::sleep_until(Instant::now() + Duration::from_millis(timeout)).await;
                    let id = reflected.reflection_of.as_ref().unwrap();
                    if let Some((_, tx)) = singles.remove(id) {
                        reflected.status = Some(StatusCode::from_u16(408).unwrap());
                        reflected.body = Some(Substance::Empty);
                        let reflected = reflected.build().unwrap();
                        tx.send(ReflectedAggregate::Single(reflected));
                    }
                });
            }
            BounceBacks::Count(count) => {
                let (tx, mut rx) = mpsc::channel(32);
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
                    tokio::time::sleep_until(Instant::now() + Duration::from_millis(timeout)).await;
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
                    tokio::time::sleep_until(Instant::now() + Duration::from_millis(timeout)).await;
                    // all we have to do is remove it, the multi loop will take care of the rest
                    multis.remove(&id);
                });
            }
        }

        rx
    }
}

#[derive(Clone)]
pub struct ProtoTransmitter {
    pub agent: SetStrategy<Agent>,
    pub scope: SetStrategy<Scope>,
    pub handling: SetStrategy<Handling>,
    pub from: SetStrategy<Port>,
    pub to: SetStrategy<Recipients>,
    pub router: Arc<dyn Router>,
    pub exchanger: Exchanger,
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
            SetStrategy::Fill(agent) => wave.fill_agent(agent.clone()),
            SetStrategy::Override(agent) => wave.agent(agent.clone()),
        }

        match &self.scope {
            SetStrategy::None => {}
            SetStrategy::Fill(scope) => wave.fill_scope(scope.clone()),
            SetStrategy::Override(scope) => wave.scope(scope.clone()),
        }

        match &self.handling {
            SetStrategy::None => {}
            SetStrategy::Fill(handling) => wave.fill_handling(handling.clone()),
            SetStrategy::Override(handling) => wave.handling(handling.clone()),
        }

        let directed = wave.build()?;

        match directed.bounce_backs() {
            BounceBacks::None => FromReflectedAggregate::from_reflected_aggregate(ReflectedAggregate::None),
            _ => {
                let reflected_rx = self.exchanger.exchange(&directed).await;
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
