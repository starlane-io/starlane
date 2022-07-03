use crate::star::StarSkel;
use anyhow::anyhow;
use dashmap::DashMap;

use cosmic_api::error::MsgErr;
use cosmic_api::config::config::bind::{
    BindConfig, PipelineStepVar, PipelineStopVar, WaveKind,
};
use cosmic_api::id::id::{Layer, Point, Port, ToPoint, ToPort, TraversalLayer, Uuid};
use cosmic_api::id::{ArtifactSubKind, TraversalInjection};
use cosmic_api::id::Traversal;
use cosmic_api::log::{PointLogger, RootLogger, SpanLogger};
use cosmic_api::parse::model::PipelineVar;
use cosmic_api::parse::{
    Env, MapResolver, MultiVarResolver, PointCtxResolver, RegexCapturesResolver,
};
use cosmic_api::security::Access;
use cosmic_api::selector::selector::PipelineKind;
use cosmic_api::selector::{PayloadBlock, PayloadBlockVar};
use cosmic_api::substance::substance::{Call, CallKind, Substance};
use cosmic_api::sys::ParticleRecord;
use cosmic_api::util::{ToResolved, ValueMatcher};
use cosmic_api::wave::{Agent, CmdMethod, Method, DirectedCore, Ping, Reflectable, ReflectedCore, Pong, Wave, Exchanger, UltraWave, DirectedWave, ReflectedWave};
use regex::{CaptureMatches, Regex};

use std::collections::HashMap;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use http::{HeaderMap, StatusCode, Uri};
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use cosmic_api::RegistryApi;

#[derive(Clone)]
pub struct FieldEx {
    pub port: Port,
    pub skel: StarSkel,
    pub state: FieldState,
    pub logger: SpanLogger
}



impl FieldEx {
    pub fn new(point: Point, skel: StarSkel, state: FieldState, logger: SpanLogger ) -> Self {
        let port = point.to_port().with_layer(Layer::Field);
        Self { port, skel, state, logger }
    }

    async fn handle_action(&self, action: RequestAction) -> anyhow::Result<()> {
        match action.action {
            PipeAction::CoreDirected(mut request) => {
                self.traverse_next(request.wrap() ).await;
            }
            PipeAction::FabricDirected(mut request) => {
                self.traverse_next(request.wrap() ).await;
            }
            PipeAction::Respond => {
                let pipex = self.state.pipe_exes.remove(&action.request_id);

                match pipex {
                    None => {
                        self.logger.error(format!("no pipeline set for request_id: {}", action.request_id));
                    }
                    Some((_, mut pipex)) => {
                        self.skel.traverse_to_next_tx.send(pipex.reflect().to_ultra() ).await;
                    }
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl TraversalLayer for FieldEx {

    fn port(&self) -> &Port{
        &self.state.port
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        self.skel.traverse_to_next_tx.send(traversal).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }

    async fn directed_core_bound(&self, mut directed: Traversal<DirectedWave>) -> Result<(), MsgErr> {
        directed.logger.set_span_attr("message-id", &directed.id().to_string() );
        let access = self.skel.registry.access(directed.agent(), &directed.to).await;

        match access {
            Ok(access) => {
                if !access.permissions().particle.execute {
                    let err_msg = format!(
                        "execute permission required to send requests to {}",
                        directed.to.to_string()
                    );
                    directed.logger.error(err_msg.as_str());
                    self.skel
                        .fabric_tx
                        .send(directed.err(err_msg.into(), self.port().clone() ).to_ultra() );
                    return Ok(());
                }
            }
            Err(err) => {
                directed.logger.error(format!("{}", err.to_string()));
            }
        }

        let bind = self.skel.machine.artifacts.bind(&directed.to).await?;
        let route = bind.select(&directed.payload )?;

        let regex = route.selector.path.clone();

        let env = {
            let path_regex_capture_resolver =
                RegexCapturesResolver::new(regex, directed.core().uri.path().to_string())?;
            let mut env = Env::new(directed.to.clone().to_point());
            env.add_var_resolver(Arc::new(path_regex_capture_resolver));
            env.set_var("self.bundle", bind.bundle().clone().into());
            env.set_var("self.bind", bind.point().clone().into());
            env
        };

        let directed_id = directed.id().to_string();

        let pipeline = route.block.clone();

        let call = directed.to_call()?;
        let logger = directed.logger.span();
        let mut pipex = PipeEx::new(directed,self.clone(),  pipeline, env, logger.clone());
        let action = match pipex.next() {
            Ok(action) => action,
            Err(err) => {
                let err_msg = format!("Binder: pipeline error for call {}", call.to_string());
                logger.error(err_msg.as_str());
                self.skel
                    .traverse_to_next_tx
                    .send(pipex.fail(500, err_msg.as_str()).to_ultra())
                    .await;
                return Ok(());
            }
        };

        if let PipeAction::Respond = action {
            self.skel.traverse_to_next_tx.send(pipex.reflect().to_ultra()).await;
            return Ok(());
        }

        self.state.pipe_exes.insert(directed_id.clone(), pipex);

        let action = RequestAction { request_id: directed_id, action };

        self.handle_action(action);
        Ok(())
    }

    async fn reflected_core_bound(&self, mut traversal: Traversal<ReflectedWave>) -> Result<(), MsgErr> {
        let reflected_id = traversal.reflection_of().to_string();
        let mut pipex = self.state.pipe_exes.remove(&reflected_id);

        if let None = pipex {
            let err_msg = format!(
                "Binder: cannot locate a pipeline executor for processing request: {}",
                traversal.reflection_of().to_string()
            );
            traversal.logger.span().error(err_msg.clone());
            return Err(err_msg.into());
        }

        let (_, mut pipex) = pipex.expect("pipeline executor");

        let action = pipex.handle_reflected(traversal.payload)?;

        if let PipeAction::Respond = action {
            self.skel.traverse_to_next_tx.send(pipex.reflect().to_ultra() ).await;
            return Ok(());
        }

        self.state.pipe_exes.insert(reflected_id.clone(), pipex);

        let action = RequestAction { request_id: reflected_id, action };

        self.handle_action(action);

        Ok(())
    }

    async fn inject(&self, wave: UltraWave) {
        let inject = TraversalInjection::new( self.state.port.clone(), wave );
        self.skel.inject_tx.send(inject).await;
    }
}

pub struct PipeEx {
    pub logger: SpanLogger,
    pub traversal: PipeTraversal,
    pub field: FieldEx,
    pub pipeline: PipelineVar,
    pub env: Env,
}

impl PipeEx {
    pub fn new(
        traversal: Traversal<DirectedWave>,
        binder: FieldEx,
        pipeline: PipelineVar,
        env: Env,
        logger: SpanLogger,
    ) -> Self {
        let traversal = PipeTraversal::new(binder.port.clone(), traversal);
        Self {
            traversal: traversal,
            field: binder,
            pipeline,
            env,
            logger,
        }
    }
}

impl PipeEx {
    pub fn next(&mut self) -> Result<PipeAction,MsgErr> {
        match self.pipeline.consume() {
            Some(segment) => {
                self.execute_step(&segment.step)?;
                Ok(self.execute_stop(&segment.stop)?)
            }
            None => Ok(PipeAction::Respond),
        }
    }

    pub fn handle_reflected(&mut self, reflected: ReflectedWave ) -> Result<PipeAction,MsgErr> {
        self.traversal.push(reflected.to_ultra() );
        self.next()
    }

    fn reflect(self) -> Traversal<ReflectedWave> {
        self.traversal.reflect()
    }

    fn fail(self, status: u16, error: &str) -> Traversal<ReflectedWave> {
        self.traversal.fail(status, error)
    }

    fn execute_stop(&mut self, stop: &PipelineStopVar) -> Result<PipeAction, MsgErr> {
        match stop {
            PipelineStopVar::Internal => {
                let request = self.traversal.direct();
                Ok(PipeAction::CoreDirected(request))
            }
            PipelineStopVar::Call(call) => {
                let call: Call = call.clone().to_resolved(&self.env)?;
                let (method, path) = match &call.kind {
                    CallKind::Msg(msg) => {
                        let path = msg.path.clone().to_resolved(&self.env)?;
                        (Method::Msg(msg.method.clone()), path)
                    }
                    CallKind::Http(http) => {
                        let path = http.path.clone().to_resolved(&self.env)?;
                        (Method::Http(http.method.clone()), path)
                    }
                };
                let mut core: DirectedCore = method.into();
                core.body = self.traversal.body.clone();
                core.headers = self.traversal.headers.clone();
                core.uri = Uri::from_str(path.as_str())?;
                let ping = self.traversal.initial.clone().with(Wave::new(Ping::new(
                    core,
                    self.traversal.to().clone(),
                ), self.field.port.clone() ));
                Ok(PipeAction::FabricDirected(ping.to_directed()))
            }
            PipelineStopVar::Respond => Ok(PipeAction::Respond),
            PipelineStopVar::Point(point) => {
                let uri = self.traversal.uri.clone();
                let point: Point = point.clone().to_resolved(&self.env)?;
                let method = Method::Cmd(CmdMethod::Read);
                let mut core:DirectedCore = method.into();
                core.uri = uri;

                let request = self.traversal.initial.clone().with(Wave::new(Ping::new(
                    core,
                    self.traversal.to().clone(),
                ),point.to_port()));
                Ok(PipeAction::FabricDirected(request.to_directed()))
            }
        }
    }

    fn execute_step(&self, step: &PipelineStepVar) -> Result<(), MsgErr> {
        match &step.entry {
            WaveKind::Request => {
                for block in &step.blocks {
                    self.execute_block(block)?;
                }
            }
            WaveKind::Response => {}
        }
        Ok(())
    }

    fn execute_block(&self, block: &PayloadBlockVar) -> Result<(), MsgErr> {
        let block: PayloadBlock = block.clone().to_resolved(&self.env)?;
        match block {
            PayloadBlock::RequestPattern(pattern) => {
                pattern.is_match(&self.traversal.body)?;
            }
            PayloadBlock::ResponsePattern(pattern) => {
                pattern.is_match(&self.traversal.body)?;
            }
        }
        Ok(())
    }
}

pub struct PipeTraversal {
    pub port: Port,
    pub initial: Traversal<DirectedWave>,
    pub method: Method,
    pub body: Substance,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub status: StatusCode,
}

impl PipeTraversal {
    pub fn new(port: Port, initial_request: Traversal<DirectedWave>) -> Self {
        Self {
            port,
            method: initial_request.core().method.clone(),
            body: initial_request.core().body.clone(),
            uri: initial_request.core().uri.clone(),
            headers: initial_request.core().headers.clone(),
            initial: initial_request,
            status: StatusCode::from_u16(200).unwrap(),
        }
    }

    pub fn request_core(&self) -> DirectedCore {
        DirectedCore {
            headers: self.headers.clone(),
            method: self.method.clone(),
            uri: self.uri.clone(),
            body: self.body.clone(),
        }
    }

    pub fn to(&self) -> &Port{
        &self.initial.to
    }

    pub fn from(&self) -> &Port{
        self.initial.from()
    }

    pub fn direct(&self) -> Traversal<DirectedWave> {
        self.initial
            .clone()
            .with(Wave::new(Ping::new(self.request_core(), self.from().clone()), self.port.clone() )).to_directed()
    }

    pub fn response_core(&self) -> ReflectedCore {
        ReflectedCore {
            headers: self.headers.clone(),
            body: self.body.clone(),
            status: self.status.clone(),
        }
    }

    pub fn response(&self) -> Pong {
        Pong::new(
            self.response_core().clone(),
            self.to().clone().to_port(),
            self.from().clone().to_port(),
            self.initial.id().clone(),
        )
    }

    pub fn push(&mut self, wave: UltraWave) {
        match wave {
            UltraWave::Ping(ping) => {
                let ping = ping.variant;;
                let core = ping.core;
                self.method = core.method;
                self.uri = core.uri;
                self.headers = core.headers;
                self.body = core.body;
            }
            UltraWave::Pong(pong) => {
                let pong = pong.variant;;
                let core = pong.core;
                self.headers = core.headers;
                self.body = core.body;
                self.status = core.status;
            }
            UltraWave::Ripple(ripple) => {
                let ripple = ripple.variant;;
                let core = ripple.core;
                self.method = core.method;
                self.uri = core.uri;
                self.headers = core.headers;
                self.body = core.body;
            }
            UltraWave::Echo(echo) => {
                let echo = echo.variant;
                let core = echo.core;
                self.headers = core.headers;
                self.body = core.body;
                self.status = core.status;
            }
        }
    }

    pub fn reflect(self) -> Traversal<ReflectedWave> {
        let core = self.response_core();
        let reflection = self.initial.payload.reflection();
        let reflected = reflection.make( core, self.port.clone(), self.initial.to.clone() );
        self.initial.with(reflected)
    }

    pub fn fail(self, status: u16, error: &str) -> Traversal<ReflectedWave> {
        let core = ReflectedCore::status(status);
        let reflection = self.initial.payload.reflection();
        let reflected = reflection.make( core, self.port.clone(), self.initial.to.clone() );
        self.initial.with(reflected )
    }
}

struct RequestAction {
    pub request_id: String,
    pub action: PipeAction,
}

pub enum PipeAction {
    CoreDirected(Traversal<DirectedWave>),
    FabricDirected(Traversal<DirectedWave>),
    Respond,
}

/// The idea here is to eventually move this funcitionality into it's own crate 'mesh-bindex'
/// this mod basically enforces the bind

#[derive(Clone)]
pub struct FieldState {
    port: Port,
    pipe_exes: Arc<DashMap<String, PipeEx>>,
}

impl FieldState {
    pub fn new(port: Port) -> Self {
        Self {
            port,
            pipe_exes: Arc::new(DashMap::new()),
        }
    }
}
