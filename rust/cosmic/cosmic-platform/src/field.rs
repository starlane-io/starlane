use crate::star::StarSkel;
use anyhow::anyhow;
use dashmap::DashMap;

use cosmic_api::config::config::bind::{BindConfig, PipelineStepVar, PipelineStopVar, WaveKind};
use cosmic_api::error::MsgErr;
use cosmic_api::id::id::{
    BaseKind, Kind, Layer, Point, Port, ToBaseKind, ToPoint, ToPort, TraversalLayer, Uuid,
};
use cosmic_api::id::Traversal;
use cosmic_api::id::{ArtifactSubKind, TraversalInjection};
use cosmic_api::log::{PointLogger, RootLogger, SpanLogger, Trackable, Tracker};
use cosmic_api::parse::model::PipelineVar;
use cosmic_api::parse::{
    bind_config, Env, MapResolver, MultiVarResolver, PointCtxResolver, RegexCapturesResolver,
};
use cosmic_api::security::Access;
use cosmic_api::selector::selector::PipelineKind;
use cosmic_api::selector::{PayloadBlock, PayloadBlockVar};
use cosmic_api::substance::substance::{Call, CallKind, Substance};
use cosmic_api::sys::ParticleRecord;
use cosmic_api::util::{log, ToResolved, ValueMatcher};
use cosmic_api::wave::{Agent, Bounce, CmdMethod, DirectedCore, DirectedWave, Exchanger, Method, Ping, Pong, Reflectable, ReflectedCore, ReflectedWave, Ripple, Signal, SingularDirectedWave, ToRecipients, UltraWave, Wave};
use regex::{CaptureMatches, Regex};

use crate::{PlatErr, Platform, RegistryApi};
use cosmic_api::ArtRef;
use http::{HeaderMap, StatusCode, Uri};
use std::collections::HashMap;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};
use cosmic_api::particle::particle::Property;


#[derive(Clone)]
pub struct Field<P>
where
    P: Platform + 'static,
{
    pub port: Port,
    pub skel: StarSkel<P>,
    pub state: FieldState<P>,
    pub logger: SpanLogger,
}

impl<P> Field<P>
where
    P: Platform + 'static,
{
    pub fn new(point: Point, skel: StarSkel<P>, state: FieldState<P>, logger: SpanLogger) -> Self {
        let port = point.to_port().with_layer(Layer::Field);
        Self {
            port,
            skel,
            state,
            logger,
        }
    }

    async fn handle_action(&self, action: Action) -> Result<(), MsgErr> {

        let track = action.track();

        match action.action {
            PipeAction::CoreDirected(mut directed) => {
                self.traverse_next(directed.wrap()).await;
            }
            PipeAction::FabricDirected(mut directed) => {
                self.traverse_next(directed.wrap()).await;
            }
            PipeAction::Reflected => {
                let pipex = self.state.pipe_exes.remove(&action.reflection_of);

                match pipex {
                    None => {
                        self.logger.error(format!(
                            "no pipeline set for directed_id: {}",
                            action.reflection_of
                        ));
                    }
                    Some((_, mut pipex)) => {
                        self.skel
                            .traverse_to_next_tx
                            .send(pipex.reflect().to_ultra())
                            .await;
                    }
                }
            }
        }
        Ok(())
    }


}

#[async_trait]
impl<P> TraversalLayer for Field<P>
where
    P: Platform + 'static,
{
    fn port(&self) -> Port {
        self.state.point.clone().to_port().with_layer(Layer::Field)
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        self.skel.traverse_to_next_tx.send(traversal).await;
    }

    fn exchanger(&self) -> &Exchanger {
        &self.skel.exchanger
    }

    async fn directed_core_bound(
        &self,
        mut directed: Traversal<DirectedWave>,
    ) -> Result<(), MsgErr> {
        directed
            .logger
            .set_span_attr("message-id", &directed.id().to_string());

        self.skel.logger.track(&directed, || {
            Tracker::new("field:directed_core_bound", "Receive")
        });

        let access = self
            .skel
            .registry
            .access(&directed.agent().clone().to_point(), &directed.to)
            .await;

        match access {
            Ok(access) => {
                if !access.permissions().particle.execute {
                    let err_msg = format!(
                        "execute permission required to send requests to {}",
                        directed.to.to_string()
                    );
                    directed.logger.error(err_msg.as_str());
                    match directed.err(err_msg.into(), self.port().clone()) {
                        Bounce::Absorbed => {
                            self.skel.logger.track(&directed, || {
                                Tracker::new("field:directed_core_bound", "Absorbed")
                            });
                        }
                        Bounce::Reflected(reflected) => {
                            self.skel.logger.track(&directed, || {
                                Tracker::new("field:directed_core_bound", "Bounced")
                            });

                            self.skel.gravity_tx.send(reflected.to_ultra()).await;
                        }
                    }

                    return Ok(());
                }
            }
            Err(err) => {
                directed.logger.error(format!("{}", err.to_string()));
            }
        }

        let record = self.skel.registry.locate(&directed.to.point).await.map_err(|e|e.to_cosmic_err())?;

        let properties = self.skel.registry.get_properties(&directed.to.point).await.map_err(|e|e.to_cosmic_err())?;
        let bind_property = properties.get("bind");

        self.skel.logger.track(&directed, || {
            Tracker::new("field:directed_core_bound", "PreBind")
        });

        let bind = match bind_property {
            None => {
                self.skel.logger.track(&directed, || {
                    Tracker::new("field:directed_core_bound", "GetBindFromDriver")
                });

                let driver = self.skel.drivers.get(&record.details.stub.kind).await?;

                self.skel.logger.track(&directed, || {
                    Tracker::new("field:directed_core_bound", "GetBindFromItem")
                });
                driver.bind(&directed.to.point).await.map_err(|e|e.to_cosmic_err())?
            }
            Some(bind) => {
                let bind = Point::from_str(bind.value.as_str())?;
                self.skel.machine.artifacts.bind(&bind).await?
            }
        };


        self.skel.logger.track(&directed, || {
            Tracker::new("field:directed_core_bound", "GotStaticBind")
        });

        let route = bind.select(&directed.payload)?;

        self.skel.logger.track(&directed, || {
            Tracker::new("field:directed_core_bound", "RouteSelected")
        });

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

let method = directed.payload.core().method.clone();
        let directed_id = directed.id().to_string();

        let pipeline = route.block.clone();

        let to = directed.to.clone();
        let call = directed.to_call(to)?;
        let logger = directed.logger.span();

        self.skel.logger.track(&directed, || {
            Tracker::new("field:directed_core_bound", "PipeEx")
        });
        let track = directed.track();

        let mut pipex = PipeEx::new(directed, self.clone(), pipeline, env, logger.clone());
        let action = match pipex.next() {
            Ok(action) => action,
            Err(err) => {
                let err_msg = format!("Field: pipeline error for call {}", call.to_string());
                logger.error(err_msg.as_str());
                self.skel
                    .traverse_to_next_tx
                    .send(pipex.fail(500, err_msg.as_str()).to_ultra())
                    .await;
                return Ok(());
            }
        };

        if let PipeAction::Reflected = action {
            self.skel
                .traverse_to_next_tx
                .send(pipex.reflect().to_ultra())
                .await;
            return Ok(());
        }

self.logger.info(format!("inserting pipeline executor for directed: {} & action {} & method {} pipex.traversal.method {}", directed_id.to_string(), action.to_string(), method.to_string(), pipex.traversal.method.to_string() ));
        self.state.pipe_exes.insert(directed_id.clone(), pipex);

        let action = Action {
            reflection_of: directed_id,
            action,
            track
        };

        self.handle_action(action).await?;

        Ok(())
    }

    async fn reflected_core_bound(
        &self,
        mut traversal: Traversal<ReflectedWave>,
    ) -> Result<(), MsgErr> {

        let reflection_of = traversal.reflection_of().to_string();
        let mut pipex = self.state.pipe_exes.remove(&reflection_of);

        if let None = pipex {
            let err_msg = format!(
                "Field: cannot locate a pipeline executor for processing reflection of directed message: {}",
                traversal.reflection_of().to_string()
            );
            self.logger.error( err_msg.clone() );
//            traversal.logger.span().error(err_msg.clone());
            return Err(err_msg.into());
        } else {
println!("~~FOUND reflected pipex!")
        }

        let (_, mut pipex) = pipex.expect("pipeline executor");

        let track = traversal.track();

        let action = pipex.handle_reflected(traversal.payload)?;

        if let PipeAction::Reflected = action {
            self.skel
                .traverse_to_next_tx
                .send(pipex.reflect().to_ultra())
                .await;
            return Ok(());
        }

        self.state.pipe_exes.insert(reflection_of.clone(), pipex);

        let action = Action {
            reflection_of,
            action,
            track
        };

        self.handle_action(action).await?;

        Ok(())
    }

    async fn inject(&self, wave: UltraWave) {
        let inject = TraversalInjection::new(self.state.point.clone().to_port().with_layer(Layer::Field), wave);
        self.skel.inject_tx.send(inject).await;
    }
}

pub struct PipeEx<P>
where
    P: Platform + 'static,
{
    pub logger: SpanLogger,
    pub traversal: PipeTraversal,
    pub field: Field<P>,
    pub pipeline: PipelineVar,
    pub env: Env,
}

impl<P> PipeEx<P>
where
    P: Platform + 'static,
{
    pub fn new(
        traversal: Traversal<DirectedWave>,
        binder: Field<P>,
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

    pub fn next(&mut self) -> Result<PipeAction, MsgErr> {
        match self.pipeline.consume() {
            Some(segment) => {
                self.execute_step(&segment.step)?;
                Ok(self.execute_stop(&segment.stop)?)
            }
            None => Ok(PipeAction::Reflected),
        }
    }

    pub fn handle_reflected(&mut self, reflected: ReflectedWave) -> Result<PipeAction, MsgErr> {
        self.traversal.push(reflected.to_ultra());
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
                    CallKind::Cmd(cmd) => {
                        let path = cmd.path.clone().to_resolved(&self.env)?;
                        (Method::Cmd(cmd.method.clone()), path)
                    }
                    CallKind::Sys(sys) => {
                        let path = sys.path.clone().to_resolved(&self.env)?;
                        (Method::Sys(sys.method.clone()), path)
                    }
                };
                let mut core: DirectedCore = method.into();
                core.body = self.traversal.body.clone();
                core.headers = self.traversal.headers.clone();
                core.uri = Uri::from_str(path.as_str())?;
                let ping = self.traversal.initial.clone().with(Wave::new(
                    Ping::new(core, self.traversal.to().clone()),
                    self.field.port.clone(),
                ));
                Ok(PipeAction::FabricDirected(ping.to_directed()))
            }
            PipelineStopVar::Respond => Ok(PipeAction::Reflected),
            PipelineStopVar::Point(point) => {
                let uri = self.traversal.uri.clone();
                let point: Point = point.clone().to_resolved(&self.env)?;
                let method = Method::Cmd(CmdMethod::Read);
                let mut core: DirectedCore = method.into();
                core.uri = uri;

                let request = self.traversal.initial.clone().with(Wave::new(
                    Ping::new(core, self.traversal.to().clone()),
                    point.to_port(),
                ));
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

    pub fn directed_core(&self) -> DirectedCore {
        DirectedCore {
            headers: self.headers.clone(),
            method: self.method.clone(),
            uri: self.uri.clone(),
            body: self.body.clone(),
        }
    }

    pub fn to(&self) -> &Port {
        &self.initial.to
    }

    pub fn from(&self) -> &Port {
        self.initial.from()
    }

    pub fn direct(&self) -> Traversal<DirectedWave> {

        match self.initial.payload.clone() {
            DirectedWave::Ping(mut ping) => {
                ping = ping.with_core(self.directed_core());
                ping.track = self.initial.track();
                self.initial.clone().with(ping.to_directed())
            }
            DirectedWave::Ripple(mut ripple) => {
                ripple = ripple.with_core(self.directed_core());
                ripple.track = self.initial.track();
                self.initial.clone().with(ripple.to_directed())
            }
            DirectedWave::Signal(mut signal) => {
                signal = signal.with_core(self.directed_core());
                signal.track = self.initial.track();
                self.initial.clone().with(signal.to_directed())
            }
        }
        /*self.initial
            .clone()
            .with(Wave::new(
                Ping::new(self.directed_core(), self.from().clone()),
                self.port.clone(),
            ))
            .to_directed()

         */
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
            self.from().clone().to_port().to_recipients(),
            self.initial.id().clone(),
        )
    }

    pub fn push(&mut self, wave: UltraWave) {
        match wave {
            UltraWave::Ping(ping) => {
                let ping = ping.variant;
                let core = ping.core;
                self.method = core.method;
                self.uri = core.uri;
                self.headers = core.headers;
                self.body = core.body;
            }
            UltraWave::Pong(pong) => {
                let pong = pong.variant;
                let core = pong.core;
                self.headers = core.headers;
                self.body = core.body;
                self.status = core.status;
            }
            UltraWave::Ripple(ripple) => {
                let ripple = ripple.variant;
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
            UltraWave::Signal(signal) => {
                let signal = signal.variant;
                let core = signal.core;
                self.method = core.method;
                self.uri = core.uri;
                self.headers = core.headers;
                self.body = core.body;
            }
        }
    }

    pub fn reflect(self) -> Traversal<ReflectedWave> {
        let core = self.response_core();
        let reflection = self.initial.payload.reflection().unwrap();
        let reflected = reflection.make(core, self.port.clone());

        self.initial.with(reflected)
    }

    pub fn fail(self, status: u16, error: &str) -> Traversal<ReflectedWave> {
        let core = ReflectedCore::status(status);
        let reflection = self.initial.payload.reflection().unwrap();
        let reflected = reflection.make(core, self.port.clone());
        self.initial.with(reflected)
    }
}

struct Action {
    pub reflection_of: String,
    pub action: PipeAction,
    pub track: bool
}

impl Trackable for Action {
    fn track_id(&self) -> String {
        self.reflection_of.clone()
    }

    fn track_method(&self) -> String {
        self.action.to_string()
    }

    fn track_payload(&self) -> String {
        "?".to_string()
    }

    fn track_from(&self) -> String {
       match &self.action{
           PipeAction::CoreDirected(w) => {
               w.track_from()
           }
           PipeAction::FabricDirected(w) => {
               w.track_from()
           }
           PipeAction::Reflected => {
               "?".to_string()
           }
       }
    }

    fn track_to(&self) -> String {

        match &self.action{
            PipeAction::CoreDirected(w) => {
                w.track_to()
            }
            PipeAction::FabricDirected(w) => {
                w.track_to()
            }
            PipeAction::Reflected => {
                "?".to_string()
            }
        }
    }

    fn track(&self) -> bool {
        self.track
    }
}

#[derive(strum_macros::Display)]
pub enum PipeAction {
    CoreDirected(Traversal<DirectedWave>),
    FabricDirected(Traversal<DirectedWave>),
    Reflected,
}

/// The idea here is to eventually move this funcitionality into it's own crate 'mesh-bindex'
/// this mod basically enforces the bind

#[derive(Clone)]
pub struct FieldState<P>
where
    P: Platform + 'static,
{
    point: Point,
    pipe_exes: Arc<DashMap<String, PipeEx<P>>>,
}

impl<P> FieldState<P>
where
    P: Platform + 'static,
{
    pub fn new(point : Point) -> Self {
        Self {
            point,
            pipe_exes: Arc::new(DashMap::new()),
        }
    }
}
