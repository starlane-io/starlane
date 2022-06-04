use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::message::delivery::Delivery;
use crate::particle::{ArtifactSubKind, ParticleRecord};
use crate::star::core::resource::driver::ResourceCoreDriverApi;
use crate::star::StarSkel;
use http::{HeaderMap, StatusCode, Uri};
use mesh_artifact_api::Artifact;
use mesh_portal::version::latest::config::bind::{
    BindConfig, Pipeline, PipelineStep, PipelineStop, StepKind,
};
use mesh_portal::version::latest::entity::request::get::{Get, GetOp};
use mesh_portal::version::latest::entity::request::{Method, Rc, RequestCore};
use mesh_portal::version::latest::entity::response::ResponseCore;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Agent, Message, Request, Response};
use mesh_portal::version::latest::payload::{Call, CallKind, Payload};
use mesh_portal::version::latest::log::{PointLogger, SpanLogger};
use mesh_portal_versions::error::MsgErr;
use regex::{CaptureMatches, Regex};
use std::collections::HashMap;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use anyhow::anyhow;
use mesh_portal::version::latest::msg::MsgMethod;
use mesh_portal::version::latest::security::Access;
use mesh_portal_versions::version::v0_0_1::config::config::bind::{MessageKind, PipelineStepVar, PipelineStopVar};
use mesh_portal_versions::version::v0_0_1::log::RootLogger;
use mesh_portal_versions::version::v0_0_1::parse::{Env, MapResolver, MultiVarResolver, PointCtxResolver, RegexCapturesResolver };
use mesh_portal_versions::version::v0_0_1::parse::model::PipelineVar;
use mesh_portal_versions::version::v0_0_1::selector::{PayloadBlock, PayloadBlockVar};
use mesh_portal_versions::version::v0_0_1::selector::selector::PipelineKind;
use mesh_portal_versions::version::v0_0_1::util::{ToResolved, ValueMatcher};
use nom::combinator::map_res;
use tokio::io::AsyncBufReadExt;
use tokio::sync::Mutex;
use mesh_portal_versions::version::v0_0_1::id::id::ToPoint;
use mesh_portal_versions::version::v0_0_1::messaging::CmdMethod;
use crate::cache::{ArtifactItem, Cacheable, CachedConfig};

/// The idea here is to eventually move this funcitionality into it's own crate 'mesh-bindex'
/// this mod basically enforces the bind

#[derive(Clone)]
pub struct BindEx {
    pub bind_config_cache: Arc<dyn BindConfigCache>,
    pub router: Arc<dyn BindExRouter>,
    pub pipeline_executors: Arc<Mutex<HashMap<String, PipeEx>>>,
    pub logger: RootLogger,
    pub registry: Arc<dyn RegistryApi>,
}

fn request_id(request: &Request) -> String {
    format!("{}{}", request.to.to_string(), request.id)
}

fn request_id_from_response(response: &Response) -> String {
    format!("{}{}", response.from.to_string(), response.response_to)
}

impl BindEx {
    pub async fn handle_request(&self, delivery: Delivery<Request>) -> anyhow::Result<()>{

        info!("BindEx: handle_request");
        let logger = self.logger.point(delivery.to.clone());
        let mut logger = logger.span();
        logger.set_span_attr("message-id", &delivery.id );
        let access = self.registry.access(&delivery.agent,&delivery.to);

        match access {
            Ok(access) => {
                if !access.permissions().particle.execute {
                    let err_msg = format!("execute permission required to send requests to {}", delivery.to.to_string() );
                    logger.error( err_msg.as_str() );
                    delivery.err( 403, err_msg.as_str() );
                    return Ok(());
                }
            }
            Err(err) => {
                error!("{}", err.to_string() )
            }
        }

        let bind = self.bind_config_cache.get_bind_config(&delivery.to).await?;
        let route = bind.select(&delivery.item)?;

        let regex = route.selector.path.clone();

        let env = {
            let path_regex_capture_resolver = RegexCapturesResolver::new(regex, delivery.item.core.uri.path().to_string())?;
            let mut env = Env::new(delivery.item.to.clone().to_point() );
            env.add_var_resolver(Arc::new(path_regex_capture_resolver));
            env.set_var( "self.bundle", bind.bundle()?.to_string().as_str() );
            env.set_var( "self.bind", bind.point().clone().to_string().as_str() );
            env
        };

        let request_id = request_id(&delivery.item);

        let pipeline = route.block.clone();

        let call = delivery.to_call()?;
        let logger = logger.span();
        let mut pipex = PipeEx::new(delivery, self.clone(), pipeline, env,logger.clone());
        let action = match pipex.next() {
            Ok(action) => action,
            Err(err) => {
                let err_msg = format!("Binder: pipeline error for call {}", call.to_string());
                logger.error(err_msg.as_str());
                pipex.fail(500, err_msg.as_str() );
                return Ok(());
            }
        };

        if let PipeAction::Respond = action {
            pipex.respond();
            return Ok(());
        }

        {
            let mut lock = self.pipeline_executors.lock().await;
            lock.insert(request_id.clone(), pipex);
        }

        let action = RequestAction{
            request_id,
            action
        };

        self.handle_action(action);
        Ok(())
    }

    pub async fn handle_response(&self, response: Response) -> anyhow::Result<()> {
        let request_id = request_id_from_response(&response);
        let mut pipex = {
            let mut lock = self.pipeline_executors.lock().await;
            lock.remove(&request_id )
        };

        if let None = pipex {
            let err_msg = format!(
                "Binder: cannot locate a pipeline executor for processing request: {}",
                response.response_to
            );
            self.logger.point(response.to.clone()).span()
                .error(err_msg.clone());
            error!("{}", err_msg);
            return Err(anyhow!(err_msg));
        }

        let mut pipex = pipex.expect("pipeline executor");

        let action = pipex.handle_response(response)?;

        if let PipeAction::Respond = action {
            pipex.respond();
            return  Ok(());
        }

        {
            let mut lock = self.pipeline_executors.lock().await;
            lock.insert(request_id.clone(), pipex);
        }

        let action = RequestAction{
            request_id,
            action
        };

        self.handle_action(action);

        Ok(())
    }

    async fn handle_action( &self, action: RequestAction ) -> anyhow::Result<()> {
        match action.action {
            PipeAction::CoreRequest(request) => {
                self.router.route_to_particle_core(Message::Request(request));
            }
            PipeAction::MeshRequest(request) => {
                self.router.route_to_mesh(Message::Request(request));
            }
            PipeAction::Respond => {
                let pipex = {
                  let mut lock = self.pipeline_executors.lock().await;
                  lock.remove(&action.request_id)
                };

                match pipex {
                    None => {
                        error!("no pipeline set for requst_id: {}",action.request_id);
                    }
                    Some(pipex) => {
                        pipex.respond();
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct PipeEx {
    pub logger: SpanLogger,
    pub traversal: Traverser,
    pub binder: BindEx,
    pub pipeline: PipelineVar,
    pub env: Env
}

impl PipeEx {
    pub fn new(
        delivery: Delivery<Request>,
        binder: BindEx,
        pipeline: PipelineVar,
        env: Env,
        logger: SpanLogger
    ) -> Self {
        let traversal = Traverser::new(delivery);
        Self {
            traversal,
            binder,
            pipeline,
            env,
            logger
        }
    }
}

impl PipeEx {
    pub fn next(&mut self) -> anyhow::Result<PipeAction> {
        match self.pipeline.consume() {
            Some(segment) => {
                self.execute_step(&segment.step)?;
                Ok(self.execute_stop(&segment.stop)?)
            }
            None => {
                Ok(PipeAction::Respond)
            }
        }
    }

    pub fn handle_response(&mut self, response: Response) -> anyhow::Result<PipeAction> {
        self.traversal.push(Message::Response(response));
        self.next()
    }

    fn respond(self) {
        self.traversal.respond();
    }

    fn fail(self, status: u16, error: &str) {
        self.traversal.fail(status, error);
    }

    fn execute_stop(&mut self, stop: &PipelineStopVar) -> Result<PipeAction, Error> {
        match stop {
            PipelineStopVar::Internal => {
                let request = self.traversal.request();
                Ok(PipeAction::CoreRequest(request))
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
                let mut core: RequestCore = method.into();
                core.body = self.traversal.body.clone();
                core.headers = self.traversal.headers.clone();
                core.uri = Uri::from_str(path.as_str())?;
                let request = Request::new(core, self.traversal.to(), call.point);
                Ok(PipeAction::MeshRequest(request))
            }
            PipelineStopVar::Respond => Ok(PipeAction::Respond),
            PipelineStopVar::Point(point) => {
                let uri = self.traversal.uri.clone();
                let point:Point = point.clone().to_resolved(&self.env)?;
                let method = Method::Cmd(CmdMethod::Read);
                let core = method.into();
                let request = Request::new(core, self.traversal.to(), point);
                Ok(PipeAction::MeshRequest(request))
            }
        }
    }

    fn execute_step(&self, step: &PipelineStepVar) -> Result<(), Error> {
        match &step.entry {
            StepKind::Request => {
                for block in &step.blocks {
                    self.execute_block(block)?;
                }
            }
            StepKind::Response => {}
        }
        Ok(())
    }

    fn execute_block(&self, block: &PayloadBlockVar) -> Result<(), Error> {
        let block:PayloadBlock  = block.clone().to_resolved(&self.env)?;
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


pub struct Traverser {
    pub initial_request: Delivery<Request>,
    pub method: Method,
    pub body: Payload,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub status: StatusCode,
}

impl Traverser {
    pub fn new(initial_request: Delivery<Request>) -> Self {
        Self {
            method: initial_request.core.method.clone(),
            body: initial_request.item.core.body.clone(),
            uri: initial_request.item.core.uri.clone(),
            headers: initial_request.item.core.headers.clone(),
            initial_request,
            status: StatusCode::from_u16(200).unwrap(),
        }
    }

    pub fn request_core(&self) -> RequestCore {
        RequestCore {
            headers: self.headers.clone(),
            method: self.method.clone(),
            uri: self.uri.clone(),
            body: self.body.clone(),
        }
    }

    pub fn to(&self) -> Point {
        self.initial_request.to.clone().to_point()
    }

    pub fn from(&self) -> Point {
        self.initial_request.from.clone().to_point()
    }

    pub fn request(&self) -> Request {
        Request::new(self.request_core(), self.from(), self.to())
    }

    pub fn response_core(&self) -> ResponseCore {
        ResponseCore {
            headers: self.headers.clone(),
            body: self.body.clone(),
            status: self.status.clone(),
        }
    }

    pub fn response(&self) -> Response {
        Response::new(
            self.response_core(),
            self.to(),
            self.from(),
            self.initial_request.id.clone(),
        )
    }

    pub fn push(&mut self, message: Message) {
        match message {
            Message::Request(request) => {
                self.method = request.core.method;
                self.uri = request.core.uri;
                self.headers = request.core.headers;
                self.body = request.core.body;
            }
            Message::Response(response) => {
                self.headers = response.core.headers;
                self.body = response.core.body;
                self.status = response.core.status;
            }
        }
    }

    pub fn respond(self) {
        let core = self.response_core();
        self.initial_request.respond(core);
    }

    pub fn fail(self, status: u16, error: &str) {
        self.initial_request.err(status, error);
    }
}

#[async_trait]
pub trait BindConfigCache: Send+Sync {
    async fn get_bind_config(&self, particle: &Point) -> anyhow::Result<ArtifactItem<CachedConfig<BindConfig>>>;
}


pub trait BindExRouter: Send+Sync {
    fn route_to_mesh(&self, message: Message);
    fn route_to_particle_core(&self, message: Message);
}

pub trait RegistryApi: Send+Sync {
    fn access(&self, to: &Agent, on: &Point) -> anyhow::Result<Access>;
}


struct RequestAction {
    pub request_id: String,
    pub action: PipeAction
}

pub enum PipeAction {
    CoreRequest(Request),
    MeshRequest(Request),
    Respond,
}

pub struct BindExSpanner {
  pub request_id: String
}

impl BindExSpanner {
   pub fn handle_request( &self, request: Request ) {

   }

   pub fn handle_response( &self, response: Response ) {

   }
}

pub struct RequestSpanner {
    pub request: Request,
    pub spanner: BindExSpanner
}

mod tmp {
    use crate::artifact::ArtifactRef;
    use crate::bindex::BindConfigCache;
    use crate::particle::ArtifactSubKind;
    use crate::star::StarSkel;
    use mesh_artifact_api::Artifact;
    use mesh_portal::version::latest::config::bind::BindConfig;
    use mesh_portal::version::latest::id::Point;
    use mesh_portal_versions::version::v0_0_1::log::Log;
    use std::str::FromStr;
    use anyhow::Context;
    use crate::cache::{ArtifactItem, CachedConfig};

    pub struct TempBindConfigCache {
        pub skel: StarSkel,
    }

    #[async_trait]
    impl BindConfigCache for TempBindConfigCache {

        async fn get_bind_config(&self, particle: &Point) -> anyhow::Result<ArtifactItem<CachedConfig<BindConfig>>> {

            let registry = self.skel.machine.registry.clone();
            let record = registry.locate(particle).await?;
            let bind = record.details.properties.get("bind").context("bind property not set")?;
            let bind_point = Point::from_str(bind.value.as_str())?;

            let mut cache = self
                .skel
                .machine
                .get_proto_artifact_caches_factory()
                .await?
                .create();
            let artifact = ArtifactRef::new(bind_point.clone(), ArtifactSubKind::Bind);
            cache.cache(vec![artifact.clone()]).await?;
            let cache = cache.to_caches().await?;
            let bind_config= cache
                .bind_configs
                .get(&artifact.point)
                .context("could not cache bind")?;
            Ok(bind_config)
        }
    }
}
