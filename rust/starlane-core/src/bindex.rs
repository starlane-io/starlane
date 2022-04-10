use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::message::delivery::Delivery;
use crate::resource::{ArtifactKind, ResourceRecord};
use crate::star::core::resource::driver::ResourceCoreDriverApi;
use crate::star::StarSkel;
use http::{HeaderMap, StatusCode, Uri};
use mesh_artifact_api::Artifact;
use mesh_portal::version::latest::config::bind::{
    BindConfig, Pipeline, PipelineStep, PipelineStop, Selector, StepKind,
};
use mesh_portal::version::latest::entity::request::get::{Get, GetOp};
use mesh_portal::version::latest::entity::request::{Action, Rc, RequestCore};
use mesh_portal::version::latest::entity::response::ResponseCore;
use mesh_portal::version::latest::id::Address;
use mesh_portal::version::latest::messaging::{Agent, Message, Request, Response};
use mesh_portal::version::latest::pattern::{Block, HttpPattern, MsgPattern};
use mesh_portal::version::latest::payload::{CallKind, Payload};
use mesh_portal_versions::error::MsgErr;
use mesh_portal_versions::version::v0_0_1::config::bind::PipelineSegment;
use mesh_portal_versions::version::v0_0_1::log::{Log, ParticleLogger};
use mesh_portal_versions::version::v0_0_1::resource::Property;
use regex::Regex;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use mesh_portal::version::latest::security::Access;

/// The idea here is to eventually move this funcitionality into it's own crate 'mesh-bindex'
/// this mod basically enforces the bind

#[derive(Clone)]
pub struct BindEx {
    pub bind_config_cache: Arc<dyn BindConfigCache>,
    pub router: Arc<dyn BindExRouter>,
    pub pipeline_executors: Arc<Mutex<HashMap<String, PipeEx>>>,
    pub logger: Arc<dyn ParticleLogger>,
    pub registry: Arc<dyn RegistryApi>,
}

fn request_id(request: &Request) -> String {
    format!("{}{}", request.to.to_string(), request.id)
}

fn request_id_from_response(response: &Response) -> String {
    format!("{}{}", response.from.to_string(), response.response_to)
}

impl BindEx {
    pub fn hande_request(&self, delivery: Delivery<Request>) -> anyhow::Result<()>{

        let access = self.registry.access(&delivery.agent,&delivery.to);

        match access {
            Ok(access) => {
                if !access.permissions().particle.execute {
                    let err_msg = format!("execute permission required to send requests to {}", delivery.to.to_string() ).as_str();
                    self.logger.log( Log::error(delivery.from.clone(), err_msg ));
                    delivery.err( 403, err_msg );
                    return Ok(());
                }
            }
            Err(err) => {
                error!("{}", err.to_string() )
            }
        }


        let bind = self.bind_config_cache.get_bind_config(&request.to)?;
        println!(
            "received msg action {} ... present selectors: {}",
            msg,
            bind.msg.elements.len()
        );
        let selector = bind.msg.find_match(&delivery.item.core);
        if selector.is_err() {
            let path = delivery.item.core.uri.path().to_string();
            let msg = msg.clone();
            let err_msg = format!(
                "bind selector for {} cannot find Pipeline match for Msg<{}>{}",
                request.to.to_string(),
                msg,
                path
            );
            self.logger
                .log(Log::error(request.from.clone(), err_msg.as_str()));

            delivery.err(404, err_msg.as_str());
            return Ok(());
        }
        let selector = selector.expect("selector");
        println!(
            "executing http pipeline for {}",
            selector.pattern.to_string()
        );
        let regex = match Regex::new(selector.pattern.path_regex.as_str()) {
            Ok(regex) => regex,
            Err(err) => {
                delivery.fail(err.to_string());
                return Ok(());
            }
        };
        let request_id = request_id(&delivery.item);

        let call = delivery.to_call();
        let mut pipex = PipeEx::new(delivery, self.clone(), selector.pipeline, regex);
        let action = match pipex.next() {
            Ok(result) => result,
            Err(err) => {
                let err_msg = format!("Binder: pipeline error for call {}", call.to_string());
                self.logger
                    .log(Log::error(request.from.clone(), err_msg.as_str()));
                pipex.fail(500, err_msg.as_str() );
                return Ok(());
            }
        };

        if let PipeAction::Respond = action {
            pipex.respond();
            return Ok(());
        }

        {
            let mut lock = self.pipeline_executors.lock()?;
            lock.insert(request_id.clone(), pipex);
        }

        let action = RequestAction{
            request_id,
            action
        };

        self.handle_action(action);
        Ok(())
    }

    pub fn handle_response(&self, response: Response) -> anyhow::Result<()> {
        let request_id = request_id_from_response(&response);
        let mut pipex = {
            let mut lock = self.pipeline_executors.lock()?;
            lock.remove(&request_id )
        };

        if let None = pipex {
            let err_msg = format!(
                "Binder: cannot locate a pipeline executor for processing request: {}",
                response.response_to
            );
            self.logger
                .log(Log::error(response.to.clone(), err_msg.as_str()));
            error!("{}", err_msg);
            Err(err_msg.into())
        }

        let mut pipex = pipex.expect("pipeline executor");

        let action = pipex.hanlde_response(response);

        if let PipeAction::Respond = action {
            pipex.respond();
            return  Ok(());
        }

        {
            let mut lock = self.pipeline_executors.lock()?;
            lock.insert(request_id.clone(), pipex);
        }

        let action = RequestAction{
            request_id,
            action
        };

        self.handle_action(action);

        Ok(())
    }

    fn handle_action( &self, action: RequestAction ) -> anyhow::Result<()> {
        match action.action {
            PipeAction::CoreRequest(request) => {
                self.router.send_to_particle_core(Message::Request(request));
            }
            PipeAction::MeshRequest(request) => {
                self.router.send_to_mesh(Message::Request(request));
            }
            PipeAction::Respond => {
                let pipex = {
                  let mut lock = self.pipeline_executors.lock()?;
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
    pub traversal: Traverser,
    pub binder: BindEx,
    pub pipeline: Pipeline,
    pub path_regex: Regex,
}

impl PipeEx {
    pub fn new(
        delivery: Delivery<Request>,
        binder: BindEx,
        pipeline: Pipeline,
        path_regex: Regex,
    ) -> Self {
        let traversal = Traverser::new(delivery);
        Self {
            traversal,
            binder,
            pipeline,
            path_regex,
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
                self.traversal.respond();
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

    fn execute_stop(&mut self, stop: &PipelineStop) -> Result<PipeAction, Error> {
        match stop {
            PipelineStop::Internal => {
                let request = self.traversal.request();
                Ok(PipeAction::CoreRequest(request))
            }
            PipelineStop::Call(call) => {
                let uri = self.traversal.uri.clone();
                let captures = self
                    .path_regex
                    .captures(uri.path())
                    .ok_or("cannot find regex captures")?;
                let address = call.address.clone().to_address(captures)?;

                let captures = self
                    .path_regex
                    .captures(uri.path())
                    .ok_or("cannot find regex captures")?;
                let (action, path) = match &call.kind {
                    CallKind::Msg(msg) => {
                        let mut path = String::new();
                        captures.expand(msg.path.as_str(), &mut path);
                        (Action::Msg(msg.action.clone()), path)
                    }
                    CallKind::Http(http) => {
                        let mut path = String::new();
                        captures.expand(http.path.as_str(), &mut path);
                        (Action::Http(http.method.clone()), path)
                    }
                };
                let mut core: RequestCore = action.into();
                core.body = self.traversal.body.clone();
                core.headers = self.traversal.headers.clone();
                core.uri = Uri::from_str(path.as_str())?;
                let request = Request::new(core, self.traversal.to(), address.clone());
                Ok(PipeAction::MeshRequest(request))
            }
            PipelineStop::Respond => Ok(PipeAction::Respond),
            PipelineStop::CaptureAddress(address) => {
                let uri = self.traversal.uri.clone();
                let captures = self
                    .path_regex
                    .captures(uri.path())
                    .ok_or("cannot find regex captures")?;
                let address = address.clone().to_address(captures)?;
                let action = Action::Rc(Rc::Get(Get {
                    address: address.clone(),
                    op: GetOp::State,
                }));
                let core = action.into();
                let request = Request::new(core, self.traversal.to(), address.clone());
                Ok(PipeAction::MeshRequest(request))
            }
        }
    }

    fn execute_step(&self, step: &PipelineStep) -> Result<(), Error> {
        match &step.kind {
            StepKind::Request => {
                for block in &step.blocks {
                    self.execute_block(block)?;
                }
            }
            StepKind::Response => {}
        }
        Ok(())
    }

    fn execute_block(&self, block: &Block) -> Result<(), Error> {
        match block {
            Block::Upload(_) => {
                return Err("upload block can only be used on the command line".into());
            }
            Block::RequestPattern(pattern) => {
                pattern.is_match(&self.traversal.body)?;
            }
            Block::ResponsePattern(pattern) => {
                pattern.is_match(&self.traversal.body)?;
            }
            Block::CreatePayload(payload) => {
                unimplemented!()
            }
        }
        Ok(())
    }
}

pub struct Traverser {
    pub initial_request: Delivery<Request>,
    pub action: Action,
    pub body: Payload,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub status: StatusCode,
}

impl Traverser {
    pub fn new(initial_request: Delivery<Request>) -> Self {
        Self {
            action: initial_request.core.action.clone(),
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
            action: self.action.clone(),
            uri: self.uri.clone(),
            body: self.body.clone(),
        }
    }

    pub fn to(&self) -> Address {
        self.initial_request.to.clone()
    }

    pub fn from(&self) -> Address {
        self.initial_request.from.clone()
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
                self.action = request.core.action;
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
pub trait BindConfigCache {
    async fn get_bind_config(&self, point: &Address) -> anyhow::Result<Artifact<BindConfig>>;
}

pub trait BindExRouter {
    fn send_to_mesh(&self, message: Message);
    fn send_to_particle_core(&self, message: Message);
}

pub trait RegistryApi {
    fn access( &self, to: &Agent, on: &Address ) -> anyhow::Result<Access>;
}


struct RequestAction {
    pub request_id: String,
    pub action: PipeAction
}

enum PipeAction {
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
    use crate::resource::ArtifactKind;
    use crate::star::StarSkel;
    use mesh_artifact_api::Artifact;
    use mesh_portal::version::latest::config::bind::BindConfig;
    use mesh_portal::version::latest::id::Address;
    use mesh_portal_versions::version::v0_0_1::log::Log;
    use std::str::FromStr;

    pub struct TempBindConfigCache {
        pub skel: StarSkel,
    }

    impl BindConfigCache for TempBindConfigCache {
        async fn get_bind_config(
            &self,
            particle: &Address,
        ) -> anyhow::Result<Artifact<BindConfig>> {
            let registry = self.skel.machine.registry.clone();
            let record = registry.locate(particle).await;
            match record {
                Ok(_) => {}
                Err(err) => self.skel.particle_logger.log(Log::error(
                    particle.clone(),
                    "get BindConfig: could not locate particle.",
                )),
            }
            let record = record?;
            let bind = record.stub.properties.get("bind");

            match bind {
                None => self.skel.particle_logger.log(Log::error(
                    particle.clone(),
                    "get BindConfig: bind property not set.",
                )),
                Some(_) => {}
            }
            let bind = bind.ok_or("bind property not set")?;
            let bind = Address::from_str(bind.value.as_str());

            match bind {
                Ok(_) => {}
                Err(err) => self.skel.particle_logger.log(Log::error(
                    particle.clone(),
                    format!(
                        "get BindConfig: bind property point parsing error: {}",
                        err.to_string()
                    )
                    .as_str(),
                )),
            }
            let bind_address = bind?;

            let mut cache = self
                .skel
                .machine
                .get_proto_artifact_caches_factory()
                .await?
                .create();
            let artifact = ArtifactRef::new(bind_address, ArtifactKind::Bind);
            cache.cache(vec![artifact.clone()]).await?;
            let cache = cache.to_caches().await?;
            let item = cache
                .bind_configs
                .get(&artifact.address)
                .ok_or(format!("could not cache bind {}", artifact.address.to_string()).as_str())?;
            let bind_config = (*item.item.clone()).item.clone();
            let bind_config = Artifact::new(bind_config);
            Ok(bind_config)
        }
    }
}
