use std::cell::Cell;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot};
use std::str::FromStr;
use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::{
    ResourceHostAction, ResourceRegistryRequest, SimpleReply, StarMessage, StarMessagePayload,
};

use crate::message::delivery::Delivery;
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind};
use crate::resource::{ArtifactKind, Kind, ResourceType, BaseKind, FileKind, ResourceLocation, UserBaseKind, ChildResourceRegistryHandler};
use crate::resource::{AssignKind, ResourceAssign, ResourceRecord};
use crate::star::core::resource::registry::{RegError, Registration};
use crate::star::shell::wrangler::{ StarFieldSelection, StarSelector};
use crate::star::{StarCommand, StarKey, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use mesh_portal::version::latest::fail::BadRequest;
use std::future::Future;
use std::sync::Arc;
use http::{HeaderMap, StatusCode, Uri};
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal::version::latest::config::bind::{BindConfig, Pipeline};
use mesh_portal::version::latest::config::Config;
use mesh_portal::version::latest::entity::request::create::{AddressSegmentTemplate, KindTemplate, Strategy};
use mesh_portal::version::latest::entity::request::{Action, Rc, RequestCore};
use mesh_portal::version::latest::entity::request::get::Get;
use mesh_portal::version::latest::fail;
use mesh_portal::version::latest::id::{Address, Meta};
use mesh_portal::version::latest::messaging::{Message, Request, Response};
use mesh_portal::version::latest::payload::{Payload, PayloadMap, Primitive, PrimitiveList};
use mesh_portal::version::latest::resource::{ResourceStub, Status};
use mesh_portal::version::latest::config::bind::{PipelineSegment, PipelineStep, PipelineStop, Selector, StepKind};
use mesh_portal::version::latest::entity::request::get::GetOp;
use mesh_portal::version::latest::entity::request::query::Query;
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::entity::request::set::Set;
use mesh_portal::version::latest::entity::response::{ResponseCore};
use mesh_portal::version::latest::id::Tks;
use mesh_portal::version::latest::pattern::{Block, HttpPattern, MsgPattern};
use mesh_portal::version::latest::payload::CallKind;
use mesh_portal_versions::version::v0_0_1::config::bind::parse::final_http_pipeline;
use mesh_portal::version::latest::entity::request::create::Create;
use regex::Regex;
use serde::de::Unexpected::Str;
use crate::artifact::ArtifactRef;
use crate::cache::{ArtifactCaches, ArtifactItem, CachedConfig};
use crate::config::config::{ContextualConfig, ResourceConfig};
use crate::star::core::resource::driver::{ResourceCoreDriverApi, ResourceCoreDriverComponent};


lazy_static!{

    pub static ref PIPELINE_OVERRIDES: HashMap<Address,Vec<Selector<HttpPattern>>> = {
        let mut map = HashMap::new();
        let mut sel = vec![];
        sel.push(final_http_pipeline( "<Get>/hyperspace/users/(.*) -> hyperspace:users^Http<Get>/$1 => &;" ).unwrap());
        sel.push(final_http_pipeline( "<Post>/hyperspace/users/(.*) -> hyperspace:users^Http<Post>/$1 => &;" ).unwrap());
        map.insert(Address::from_str("localhost").unwrap(),sel);
        map
    };

}

pub enum CoreMessageCall {
    Message(StarMessage),
}

impl Call for CoreMessageCall {}

pub struct MessagingEndpointComponent {
  inner: MessagingEndpointComponentInner
}

#[derive(Clone)]
pub struct MessagingEndpointComponentInner {
    skel: StarSkel,
    resource_core_driver_api: ResourceCoreDriverApi
}

impl MessagingEndpointComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<CoreMessageCall>) {
        let (resource_core_driver_tx, resource_core_driver_rx) = mpsc::channel(1024);
        let resource_core_driver_api = ResourceCoreDriverApi::new(resource_core_driver_tx.clone());
        {
            let skel = skel.clone();
            tokio::spawn(async move {
                ResourceCoreDriverComponent::new(skel, resource_core_driver_tx, resource_core_driver_rx).await;
            });
        }

        let inner = MessagingEndpointComponentInner {
            skel: skel.clone(),
            resource_core_driver_api
        };

        AsyncRunner::new(
            Box::new(Self {
                inner
            }),
            skel.core_messaging_endpoint_tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<CoreMessageCall> for MessagingEndpointComponent {
    async fn process(&mut self, call: CoreMessageCall) {
        let mut inner = self.inner.clone();
        tokio::spawn( async move {
            match call {
                CoreMessageCall::Message(message) => match inner.process_resource_message(message).await
                {
                    Ok(_) => {}
                    Err(err) => {
                        error!("{}", err);
                    }
                },
            }
        });
    }
}

impl MessagingEndpointComponentInner {

    async fn handle_request(&mut self, delivery: Delivery<Request>)
    {
        async fn get_bind_config( end: &mut MessagingEndpointComponentInner, address: Address ) -> Result<ArtifactItem<CachedConfig<BindConfig>>,Error> {
println!("{} getting bind config for {}", end.skel.info.kind.to_string(), address.to_string() );
            let action = Action::Rc( Rc::Get( Get{ address:address.clone(), op: GetOp::Properties(vec!["bind".to_string()])}));
            let core = action.into();
            let request = Request::new( core, address.clone(), address.parent().unwrap() );
            let response = end.skel.messaging_api.request(request).await;

println!("response from Rc GET bind {}", address.to_string() );

            if let Payload::Map(map) = response.core.body {
                if let Payload::Primitive(Primitive::Text(bind_address )) = map.get(&"bind".to_string()  ).ok_or("bind is not set" )?
                {
                    let bind_address = Address::from_str(bind_address.as_str())?;
                    let mut cache = end.skel.machine.get_proto_artifact_caches_factory().await?.create();
                    let artifact = ArtifactRef::new(bind_address, ArtifactKind::Bind);
                    cache.cache(vec![artifact.clone()]).await?;
                    let cache = cache.to_caches().await?;
                    return Ok(cache.bind_configs.get(&artifact.address).ok_or(format!("could not cache bind {}", artifact.address.to_string()).as_str())?);
                }
                else {
                    return Err("unexpected response".into());
                }
            } else {
                return Err("unexpected response".into());
            }
        }

        fn execute(end: &mut MessagingEndpointComponentInner, bind: ArtifactItem<CachedConfig<BindConfig>>, delivery: Delivery<Request> ) -> Result<(),Error> {
            match &delivery.item.core.action {
                Action::Rc(_) => {panic!("Rc should be filtered");}
                Action::Msg(msg) => {
println!("received msg action {} ... present selectors: {}",msg, bind.msg.elements.len() );
                   let selector = bind.msg.find_match(&delivery.item.core );
                   if selector.is_err() {
                       let path = delivery.item.core.uri.path().to_string();
                       let msg = msg.clone();
                       error!("bind selector cannot find Pipelines match for Msg<{}>{}",msg, path );
                       delivery.err(404, format!("bind selector cannot find Pipelines match for Msg<{}>{}",msg, path).as_str() );
                       //delivery.err(404, "bind selector cannot find Pipelines match for Msg<{}>{}" );
                       return Ok(());
                   }
                   let selector = selector.expect("selector");
                    execute_msg_pipeline(selector, delivery,end.skel.clone(),end.resource_core_driver_api.clone() );
                   Ok(())
                }
                Action::Http(http) => {
println!("delivery of http method {} and PATH {} with selectors: {}", http.to_string(), delivery.item.core.uri.path(), bind.http.elements.len() );
                    let selector = bind.http.find_match(&delivery.item.core );
                    if selector.is_err() {
println!("selector.is_err()");
                        delivery.not_found();
                        return Ok(());
                    }
                    let selector = selector.expect("selector");
                    execute_http_pipeline(selector, delivery,end.skel.clone(),end.resource_core_driver_api.clone() );
                    Ok(())
                }
            }
        }

        println!("delivery...to: {}", delivery.to.to_string());
        if let Some(selectors) = PIPELINE_OVERRIDES.get(&delivery.to) {
            println!("CHECKING: SELECTORS for :{}",delivery.item.core.uri.to_string());
            for selector in selectors {
                if selector.is_match(&delivery.item.core).is_ok() {
                    println!("FOUND OVERRIDE SELECTOR");
                    execute_http_pipeline(selector.clone(), delivery, self.skel.clone(), self.resource_core_driver_api.clone());
                    return;
                }
            }
        }


        let to = delivery.to.clone();
        match get_bind_config(self, delivery.to.clone() ).await {
            Ok(bind_config) => {
println!("got bind config for: {}", to.to_string() );
                match execute(self, bind_config, delivery ) {
                    Ok(_) => {}
                    Err(err) => {
                        error!("{}",err.to_string())
                    }
                }
            }
            Err(_) => {
                error!("could not get bind for {}",to.to_string( ));
                delivery.fail("could not get bind config for resource".into());
            }
        }

    }

    pub async fn process_resource_message(&mut self, star_message: StarMessage) -> Result<(), Error> {
        match &star_message.payload {
            StarMessagePayload::Request(request) => match &request.core.action{
                Action::Rc(rc) => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.process_resource_command(delivery).await;
                }
                _ => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.handle_request(delivery).await;
                }
            },

            StarMessagePayload::ResourceHost(action) => {
                match action {
                    ResourceHostAction::Assign(assign) => {
                        self.resource_core_driver_api.assign(assign.clone()).await;
                        let reply = star_message.ok(Reply::Empty);
                        self.skel.messaging_api.star_notify(reply);
                    }
                    ResourceHostAction::Init(_) => {}
                    ResourceHostAction::GetState(address) => {
                        match self.resource_core_driver_api.get(address.clone()).await {
                            Ok(payload) => {
                                let reply = star_message.ok(Reply::Payload(payload));
                                self.skel.messaging_api.star_notify(reply);
                            }
                            Err(err) => {
                                let reply = star_message.fail(Fail::Starlane(StarlaneFailure::Error("could not get state".to_string())));
                                self.skel.messaging_api.star_notify(reply);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn process_resource_command(&mut self, delivery: Delivery<Request>)  {
        let skel = self.skel.clone();
        let resource_core_driver_api = self.resource_core_driver_api.clone();
        tokio::spawn(async move {

            let rc = match &delivery.item.core.action {
                Action::Rc(rc) => {rc}
                _ => { panic!("should not get requests that are not Rc") }
            };


            async fn process(skel: StarSkel, resource_core_driver_api: ResourceCoreDriverApi, rc: &Rc, to: Address) -> Result<Payload, Error> {
                let record = skel.resource_locator_api.locate(to.clone()).await?;
                let kind = Kind::try_from( record.stub.kind )?;
                match kind.resource_type().child_resource_registry_handler() {
                    ChildResourceRegistryHandler::Shell => {
                        match &rc {
                            Rc::Create(create) => {

                                let chamber = ResourceRegistrationChamber::new(skel.clone());
                                let stub = chamber.create(create).await?;

                                async fn assign(
                                    skel: StarSkel,
                                    stub: ResourceStub,
                                    state: StateSrc,
                                ) -> Result<(), Error> {

                                    let star_kind = StarKind::hosts(&ResourceType::from_str(stub.kind.resource_type.as_str())?);
                                    let key = if skel.info.kind == star_kind {
                                        skel.info.key.clone()
                                    }
                                    else {
                                        let mut star_selector = StarSelector::new();
                                        star_selector.add(StarFieldSelection::Kind(star_kind.clone()));
                                        let wrangle = skel.star_wrangler_api.next(star_selector).await?;
                                        wrangle.key
                                    };
                                    skel.registry_api.assign(stub.address.clone(), key.clone()).await?;

                                    let mut proto = ProtoStarMessage::new();
                                    proto.to(ProtoStarMessageTo::Star(key.clone()));
                                    let assign = ResourceAssign::new(AssignKind::Create, stub.clone(), state);
                                    proto.payload = StarMessagePayload::ResourceHost(
                                        ResourceHostAction::Assign(assign),
                                    );
                                    let reply = skel.messaging_api
                                        .star_exchange(proto, ReplyKind::Empty, "assign resource to host")
                                        .await?;

                                    Ok(())
                                }


                                match assign(skel.clone(), stub.clone(), create.state.clone()).await {
                                    Ok(_) => {
                                        Ok(Payload::Primitive(Primitive::Stub(stub)))
                                    },
                                    Err(fail) => {
                                        eprintln!("FAIL {}",fail.to_string() );
                                        skel.registry_api
                                            .set_status(
                                                to,
                                                Status::Panic,
                                            )
                                            .await;
                                        Err(fail.into())
                                    }
                                }
                            }
                            Rc::Select(select) => {
                                let chamber = ResourceRegistrationChamber::new(skel.clone());
                                Ok(chamber.select(select).await?)
                            },
                            Rc::Update(_) => {
                                unimplemented!()
                            }
                            Rc::Query(query) => {
                                let chamber = ResourceRegistrationChamber::new(skel.clone());
                                chamber.query(&to, query).await
                            },
                            Rc::Get(get) => {
                                let chamber = ResourceRegistrationChamber::new(skel.clone());
                                chamber.get(get).await
                            }
                            Rc::Set(set) => {
                                let chamber = ResourceRegistrationChamber::new(skel.clone());
                                chamber.set(set).await?;
                                Ok(Payload::Empty)
                            }
                        }
                    }
                    ChildResourceRegistryHandler::Core => {
                        resource_core_driver_api.resource_command(to.clone(), rc.clone()).await
                    }
                }
            }

            let result = process(skel, resource_core_driver_api.clone(), rc, delivery.to().expect("expected this to work since we have already established that the item is a Request")).await.into();
            delivery.result(result);
        });
    }


}
pub fn match_kind(template: &KindTemplate) -> Result<Kind, Error> {
    let resource_type: ResourceType = ResourceType::from_str(template.resource_type.as_str())?;
    Ok(match resource_type {
        ResourceType::Root => Kind::Root,
        ResourceType::Space => Kind::Space,
        ResourceType::Base => {
            match &template.kind {
                None => {
                    return Err("kind must be set for Base".into());
                }
                Some(kind) => {
                    let kind = BaseKind::from_str(kind.as_str())?;
                    if template.specific.is_some() {
                        return Err("BaseKind cannot have a Specific".into());
                    }
                    return Ok(Kind::Base(kind));
                }
            }
        },
        ResourceType::User => Kind::User,
        ResourceType::App => Kind::App,
        ResourceType::Mechtron => Kind::Mechtron,
        ResourceType::FileSystem => Kind::FileSystem,
        ResourceType::File => {
            match &template.kind{
                None => {
                    return Err("expected kind for File".into())
                }
                Some(kind) => {
                    let file_kind = FileKind::from_str(kind.as_str())?;
                    return Ok(Kind::File(file_kind));
                }
            }
        }
        ResourceType::Database => {
            unimplemented!("need to write a SpecificPattern matcher...")
        }
        ResourceType::Authenticator => Kind::Authenticator,
        ResourceType::ArtifactBundleSeries => Kind::ArtifactBundleSeries,
        ResourceType::ArtifactBundle => Kind::ArtifactBundle,
        ResourceType::Artifact => {
            match &template.kind {
                None => {
                    return Err("expected kind for Artirtact".into());
                }
                Some(kind) => {
                    let artifact_kind = ArtifactKind::from_str(kind.as_str())?;
                    return Ok(Kind::Artifact(artifact_kind));
                }
            }
        }
        ResourceType::Proxy => Kind::Proxy,
        ResourceType::Credentials => Kind::Credentials,
        ResourceType::Control => Kind::Control,
        ResourceType::UserBase => {
            match &template.kind {
                None => {
                    return Err("kind must be set for UserBase".into());
                }
                Some(kind) => {
                    let kind = UserBaseKind::from_str(kind.as_str())?;
                    Kind::UserBase(kind)
                }
            }
        },
    })
}

pub struct PipelineExecutor {
    pub traversal: Traversal,
    pub skel: StarSkel,
    pub resource_manager_api: ResourceCoreDriverApi,
    pub pipeline: Pipeline,
    pub path_regex: Regex
}

impl  PipelineExecutor {
  pub fn new(delivery: Delivery<Request>, skel: StarSkel, resource_manager_api: ResourceCoreDriverApi, pipeline: Pipeline, path_regex: Regex ) -> Self {
      let traversal = Traversal::new(delivery);
      Self {
          traversal,
          skel,
          resource_manager_api,
          pipeline,
          path_regex
      }
  }
}

fn execute_http_pipeline( selector: Selector<HttpPattern>, delivery: Delivery<Request>, skel: StarSkel, resource_core_driver_api: ResourceCoreDriverApi )
{
println!("executing http pipeline for {}", selector.pattern.to_string() );
    let regex = match Regex::new(selector.pattern.path_regex.as_str() ) {
        Ok(regex) => regex,
        Err(err) => {
            delivery.fail(err.to_string());
            return;
        }
    };
    let exec = PipelineExecutor::new(delivery, skel, resource_core_driver_api, selector.pipeline, regex );
    exec.execute();
}

fn execute_msg_pipeline( selector: Selector<MsgPattern>, delivery: Delivery<Request>, skel: StarSkel, resource_core_driver_api: ResourceCoreDriverApi )
{
    let regex = match Regex::new(selector.pattern.path_regex.as_str() ) {
        Ok(regex) => regex,
        Err(err) => {
            delivery.fail(err.to_string());
            return;
        }
    };
    let exec = PipelineExecutor::new(delivery, skel, resource_core_driver_api, selector.pipeline, regex );
    exec.execute();
}

impl PipelineExecutor {

    pub fn execute( mut self ) {
       tokio::spawn( async move {
           async fn process( exec: &mut PipelineExecutor) -> Result<(),Error> {
               while let Option::Some(segment) = exec.pipeline.consume() {

                   exec.execute_step(&segment.step )?;
                   exec.execute_stop(&segment.stop ).await?;
                   if let PipelineStop::Respond = segment.stop {
                       break;
                   }
               }
               Ok(())
           }
           match process(&mut self ).await {
               Ok(_) => {
                   self.respond();
               }
               Err(error) => {
                   error!("{}",error.to_string());
                   self.fail(error.to_string())
               }
           }
       });
    }

    fn respond(self) {
        self.traversal.respond();
    }

    fn fail(self, error: String) {
        self.traversal.fail(error);
    }


    async fn execute_stop( &mut self, stop: &PipelineStop ) -> Result<(),Error> {
       match stop {
           PipelineStop::Internal => {
               let request = self.traversal.request();
               let response = self.resource_manager_api.request(request).await?.ok_or()?;
               self.traversal.push( Message::Response(response));
           }
           PipelineStop::Call(call) => {
               let uri = self.traversal.uri.clone();
               let captures = self.path_regex.captures( uri.path() ).ok_or("cannot find regex captures" )?;
               let address = call.address.clone().to_address(captures)?;

               let captures = self.path_regex.captures( uri.path() ).ok_or("cannot find regex captures" )?;
               let (action,path) = match &call.kind {
                   CallKind::Msg(msg) => {
                       let mut path = String::new();
                       captures.expand( msg.path.as_str(), & mut path );
                       (Action::Msg(msg.action.clone()),path)

                   }
                   CallKind::Http(http) => {
                       let mut path = String::new();
                       captures.expand( http.path.as_str(), & mut path );
                       (Action::Http(http.method.clone()),path)
                   }
               };
               let mut core :RequestCore= action.into();
               core.body = self.traversal.body.clone();
               core.headers = self.traversal.headers.clone();
               core.uri = Uri::from_str(path.as_str())?;
               let request = Request::new( core, self.traversal.to(), address.clone() );
               let response = self.skel.messaging_api.request(request).await;
               self.traversal.push( Message::Response(response));
           }
           PipelineStop::Respond => {
               // while loop will trigger a response
           }
           PipelineStop::CaptureAddress(address) => {
               let uri = self.traversal.uri.clone();
               let captures = self.path_regex.captures( uri.path() ).ok_or("cannot find regex captures" )?;
               let address = address.clone().to_address(captures)?;
               let action = Action::Rc(Rc::Get(Get{ address:address.clone(), op: GetOp::State}));
               let core = action.into();
               let request = Request::new( core, self.traversal.to(), address.clone() );
               let response = self.skel.messaging_api.request(request).await;
               self.traversal.push( Message::Response(response));
           }
       }
       Ok(())
    }


    fn execute_step( &self,  step: &PipelineStep ) -> Result<(),Error> {
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

    fn execute_block( &self,  block: &Block ) -> Result<(),Error> {
        match block {
            Block::Upload(_) => {
                return Err("upload block can only be used on the command line".into());
            }
            Block::RequestPattern(pattern) => {
                pattern.is_match( &self.traversal.body )?;
            }
            Block::ResponsePattern(pattern) => {
                pattern.is_match( &self.traversal.body )?;
            }
            Block::CreatePayload(payload) => {
                unimplemented!()
            }
        }
        Ok(())
    }
}


pub struct Traversal {
    pub initial_request: Delivery<Request>,
    pub action: Action,
    pub body: Payload,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub status: StatusCode,
}

impl Traversal {
    pub fn new( initial_request: Delivery<Request> ) -> Self {
        Self {
            action: initial_request.core.action.clone(),
            body: initial_request.item.core.body.clone(),
            uri: initial_request.item.core.uri.clone(),
            headers: initial_request.item.core.headers.clone(),
            initial_request,
            status: StatusCode::from_u16(200).unwrap()
        }
    }

    pub fn request_core(&self) -> RequestCore {
        RequestCore {
            headers: self.headers.clone(),
            action: self.action.clone(),
            uri: self.uri.clone(),
            body: self.body.clone()
        }
    }

    pub fn to(&self) -> Address {
        self.initial_request.to.clone()
    }

    pub fn from(&self) -> Address {
        self.initial_request.from.clone()
    }

    pub fn request(&self) -> Request {
        Request::new( self.request_core(), self.from(), self.to() )
    }

    pub fn response_core(&self) -> ResponseCore {
        ResponseCore {
            headers: self.headers.clone(),
            body: self.body.clone(),
            status: self.status.clone()
        }
    }

    pub fn response(&self) -> Response{
        Response::new( self.response_core(), self.to(), self.from(), self.initial_request.id.clone() )
    }

    pub fn push( &mut self, message: Message ) {
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
        self.initial_request.respond( core );
    }

    pub fn fail(self,error:String) {
        self.initial_request.fail(error);
    }

}

pub struct ResourceRegistrationChamber {
    skel: StarSkel,
}

impl ResourceRegistrationChamber {

    pub fn new(skel: StarSkel) -> Self {
        Self { skel }
    }

    pub async fn set(&self, set:&Set) -> Result<(),Error> {
        self.skel.registry_api.set_properties(set.address.clone(), set.properties.clone()).await
    }

    pub async fn get(&self, get:&Get) -> Result<Payload,Error> {
        match &get.op {
            GetOp::State => {
                let mut proto = ProtoStarMessage::new();
                proto.to(ProtoStarMessageTo::Resource(get.address.clone()));
                proto.payload = StarMessagePayload::ResourceHost(ResourceHostAction::GetState(get.address.clone()));
                if let Ok(Reply::Payload(payload)) = self.skel.messaging_api
                    .star_exchange(proto, ReplyKind::Payload, "get state from driver")
                    .await {
                    Ok(payload)
                } else {
                    Err("could not get state".into())
                }
            }
            GetOp::Properties(keys) => {
println!("GET PROPERTIES for {}", get.address.to_string() );
                let properties = self.skel.registry_api.get_properties(get.address.clone(), keys.clone() ).await?;
                let mut map = PayloadMap::new();
                for (index,property) in properties.iter().enumerate() {

println!("\tprop{}", property.0.clone() );
                    map.insert( property.0.clone(), Payload::Primitive(Primitive::Text(property.1.clone())));
                }

                Ok(Payload::Map(map))
            }
        }
    }


    pub async fn create(&self, create:&Create) -> Result<ResourceStub,Error> {

        let child_kind = match_kind(&create.template.kind)?;
        let stub = match &create.template.address.child_segment_template {
            AddressSegmentTemplate::Exact(child_segment) => {

                let address = create.template.address.parent.push(child_segment.clone());
                match &address {
                    Ok(_) => {}
                    Err(err) => {
                        eprintln!("RC CREATE error: {}", err.to_string());
                    }
                }
                let address = address?;

                let properties = child_kind.properties_config().fill_create_defaults(&create.properties )?;
                child_kind.properties_config().check_create(&properties)?;

                let registration = Registration {
                    address: address.clone(),
                    kind: child_kind.clone(),
                    registry: create.registry.clone(),
                    properties,
                    owner: Address::root()
                };
println!("creating {}",address.to_string() );
                let mut result = self.skel.registry_api.register(registration).await;

                // if strategy is ensure then a dupe is GOOD!
                if create.strategy == Strategy::Ensure {
                    if let Err(RegError::Dupe) = result {
                        result = Ok(self.skel.resource_locator_api.locate(address.clone()).await?.stub);
                    }
                }

println!("result {}? {}",address.to_string(), result.is_ok() );
                result?
            }
            AddressSegmentTemplate::Pattern(pattern) => {
                if !pattern.contains("%") {
                    return Err("AddressSegmentTemplate::Pattern must have at least one '%' char for substitution".into());
                }
                loop {
                    let index = self.skel.registry_api.sequence(create.template.address.parent.clone()).await?;
                    let child_segment = pattern.replace( "%", index.to_string().as_str() );
                    let address = create.template.address.parent.push(child_segment.clone())?;
                    let registration = Registration {
                        address: address.clone(),
                        kind: child_kind.clone(),
                        registry: create.registry.clone(),
                        properties: create.properties.clone(),
                        owner: Address::root()
                    };

                    match self.skel.registry_api.register(registration).await {
                        Ok(stub) => {
                            if let Strategy::HostedBy(key) = &create.strategy {
                                let key = StarKey::from_str( key.as_str() )?;
                                self.skel.registry_api.assign(address, key).await?;
                                return Ok(stub);
                            } else {
                                break stub;
                            }
                        },
                        Err(RegError::Dupe) => {
                            // continue loop
                        }
                        Err(RegError::Error(error)) => {
                            return Err(error);
                        }
                    }
                }
            }
        };
        Ok(stub)
    }

    pub async fn select( &self, select: &Select ) -> Result<Payload,Error>{
            let list = Payload::List( self.skel.registry_api.select(select.clone()).await? );
            Ok(list)
    }

    pub async fn query( &self, to: &Address, query: &Query) -> Result<Payload,Error>{
        let result = Payload::Primitive(Primitive::Text(
            self.skel.registry_api
                .query(to.clone(), query.clone())
                .await?
                .to_string(),
        ));
        Ok(result)
    }
}



