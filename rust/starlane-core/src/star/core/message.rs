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
use crate::particle::{ArtifactSubKind, ChildResourceRegistryHandler, FileSubKind, Kind, KindBase, ParticleLocation, UserBaseSubKind};
use crate::particle::{AssignKind, ParticleAssign, ParticleRecord};
use crate::star::{StarCommand, StarKey, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use mesh_portal::version::latest::fail::BadRequest;
use std::future::Future;
use std::sync::Arc;
use futures::StreamExt;
use http::{HeaderMap, StatusCode, Uri};
use mesh_portal::error::MsgErr;
use mesh_portal::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal::version::latest::config::bind::{BindConfig, Pipeline, PipelineStep, PipelineStop, Selector, StepKind};
use mesh_portal::version::latest::config::Config;
use mesh_portal::version::latest::entity::request::create::{KindTemplate, PointSegFactory, Strategy};
use mesh_portal::version::latest::entity::request::{Method, Rc, RequestCore};
use mesh_portal::version::latest::entity::request::get::Get;
use mesh_portal::version::latest::fail;
use mesh_portal::version::latest::id::{Meta, Point};
use mesh_portal::version::latest::messaging::{Agent, Message, Request, Response};
use mesh_portal::version::latest::payload::{Payload, PayloadMap,  };
use mesh_portal::version::latest::particle::{Status, Stub};
use mesh_portal::version::latest::entity::request::get::GetOp;
use mesh_portal::version::latest::entity::request::query::Query;
use mesh_portal::version::latest::entity::request::select::Select;
use mesh_portal::version::latest::entity::request::set::Set;
use mesh_portal::version::latest::entity::response::ResponseCore;
use mesh_portal::version::latest::id::Tks;
use mesh_portal::version::latest::selector::{Block, HttpPattern, MsgPattern};
use mesh_portal::version::latest::payload::CallKind;
use mesh_portal::version::latest::entity::request::create::Create;
use mesh_portal::version::latest::security::Access;
use mesh_portal_versions::version::v0_0_1::particle::particle::ParticleDetails;
use regex::Regex;
use serde::de::Unexpected::Str;
use crate::artifact::ArtifactRef;
use crate::bindex::{BindConfigCache, BindEx, BindExRouter, RegistryApi};
use crate::cache::{ArtifactCaches, ArtifactItem, CachedConfig};
use crate::config::config::{ContextualConfig, ParticleConfig};
use crate::registry::{RegError, Registration };
use crate::star::core::resource::driver::{ResourceCoreDriverApi, ResourceCoreDriverComponent};
use crate::star::shell::db::{StarFieldSelection, StarSelector};


lazy_static!{

    pub static ref PIPELINE_OVERRIDES: HashMap<Point,Vec<Selector<HttpPattern>>> = {
        let mut map = HashMap::new();
        let mut sel = vec![];
//        sel.push(final_http_pipeline( "<Get>/hyperspace/users/(?<path.user>.*)::(auth) -|/${path.user}|-> hyperspace:users => &;" ).unwrap());
//        sel.push(final_http_pipeline( "<Post>/hyperspace/users/(?<path.user>.*) -|/${path.user}|-> hyperspace:users => &;" ).unwrap());
        map.insert(Point::from_str("localhost").unwrap(),sel);
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
    bindex: BindEx,
    resource_core_driver_api: ResourceCoreDriverApi
}

impl MessagingEndpointComponent {
    pub async fn start(skel: StarSkel, rx: mpsc::Receiver<CoreMessageCall>) {
        let (resource_core_driver_tx, resource_core_driver_rx) = mpsc::channel(1024);
        let resource_core_driver_api = ResourceCoreDriverApi::new(resource_core_driver_tx.clone());
        {
            let skel = skel.clone();
            tokio::spawn(async move {
                ResourceCoreDriverComponent::new(skel, resource_core_driver_tx, resource_core_driver_rx).await;
            });
        }

        let bind_config_cache = Arc::new(BindConfigCacheProxy::new(skel.clone()));

        let router = EndpointRouter{
            skel: skel.clone(),
            core_driver_api: resource_core_driver_api.clone()
        };

        pub struct MockRegistryApi();
        impl RegistryApi for MockRegistryApi {
            fn access(&self, to: &Agent, on: &Point) -> anyhow::Result<Access> {
                Ok(Access::Super(true))
            }
        }

        let bindex = BindEx {
            bind_config_cache,
            router: Arc::new(router),
            pipeline_executors: Arc::new(Default::default()),
            logger: Default::default(),
            registry: Arc::new(MockRegistryApi())
        };

        let inner = MessagingEndpointComponentInner {
            skel: skel.clone(),
            bindex,
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
        match self.bindex.handle_request(delivery).await {
            Ok(_) => {}
            Err(err) => {
                error!("{}",err.to_string())
            }
        }
    }

    pub async fn process_resource_message(&mut self, star_message: StarMessage) -> Result<(), Error> {
        match &star_message.payload {
            StarMessagePayload::Request(request) => match &request.core.method{
                Method::Cmd(rc) => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.process_particle_command(delivery).await;
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
                    ResourceHostAction::GetState(point) => {
                        match self.resource_core_driver_api.get(point.clone()).await {
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

    async fn process_particle_command(&mut self, delivery: Delivery<Request>)  {
        let skel = self.skel.clone();
        let resource_core_driver_api = self.resource_core_driver_api.clone();
        tokio::spawn(async move {

            let rc = match &delivery.item.core.method{
                Method::Cmd(rc) => {rc}
                _ => { panic!("should not get requests that are not Rc") }
            };


            async fn process(skel: StarSkel, resource_core_driver_api: ResourceCoreDriverApi, rc: &Rc, to: Point) -> Result<Payload, Error> {
                let record = skel.registry_api.locate(&to).await?;
                let kind = Kind::try_from( record.details.stub.kind )?;
                match kind.kind().child_resource_registry_handler() {
                    ChildResourceRegistryHandler::Shell => {
                        match &rc {
                            Rc::Create(create) => {

                                let chamber = skel.registry_api.clone();
                                let details= chamber.create(create).await?;

                                async fn assign(
                                    skel: StarSkel,
                                    details: ParticleDetails,
                                    state: StateSrc,
                                ) -> Result<(), Error> {

                                    let star_kind = StarKind::hosts(&KindBase::from_str(details.stub.kind.kind.as_str())?);
                                    let key = if skel.info.kind == star_kind {
                                        skel.info.key.clone()
                                    }
                                    else {
                                        let mut star_selector = StarSelector::new();
                                        star_selector.add(StarFieldSelection::Kind(star_kind.clone()));
                                        let wrangle = skel.star_db.next_wrangle(star_selector).await?;
                                        wrangle.key
                                    };
                                    skel.registry_api.assign(&details.stub.point, &key).await?;

                                    let mut proto = ProtoStarMessage::new();
                                    proto.to(ProtoStarMessageTo::Star(key.clone()));
                                    let assign = ParticleAssign::new(AssignKind::Create, details.clone(), state);
                                    proto.payload = StarMessagePayload::ResourceHost(
                                        ResourceHostAction::Assign(assign),
                                    );
                                    let reply = skel.messaging_api
                                        .star_exchange(proto, ReplyKind::Empty, "assign particle to host")
                                        .await?;

                                    Ok(())
                                }


                                match assign(skel.clone(), details.clone(), create.state.clone()).await {
                                    Ok(_) => {
                                        Ok(Payload::Stub(details.stub))
                                    },
                                    Err(fail) => {
                                        eprintln!("FAIL {}",fail.to_string() );
                                        skel.registry_api
                                            .set_status(
                                                &to,
                                                &Status::Panic,
                                            )
                                            .await;
                                        Err(fail.into())
                                    }
                                }
                            }

                            Rc::Get(get) => {
                                match get.op {
                                    GetOp::State => {
                                        unimplemented!()
                                    }
                                    GetOp::Properties(_) => {
                                        return Err("messaging end point no longer handles this type of command".into());
                                    }
                                }
                            }
                            _ => {
                                return Err("messaging end point no longer handles this type of command".into());
                            }
                       }
                    }
                    ChildResourceRegistryHandler::Core => {
                        resource_core_driver_api.particle_command(to.clone(), rc.clone()).await
                    }
                }
            }

            let result = process(skel, resource_core_driver_api.clone(), rc, delivery.to().expect("expected this to work since we have already established that the item is a Request")).await.into();
            delivery.result(result);
        });
    }


}


pub struct EndpointRouter {
  pub skel: StarSkel,
  pub core_driver_api: ResourceCoreDriverApi
}

impl BindExRouter for EndpointRouter {
    fn route_to_mesh(&self, message: Message) {
        self.skel.messaging_api.message(message);
    }

    fn route_to_particle_core(&self, message: Message) {
        match message {
            Message::Request(request) => {
                self.core_driver_api.request(request);
            }
            Message::Response(_) => {
                unimplemented!()
            }
        }
    }
}



pub struct BindConfigCacheProxy {
   pub skel: StarSkel
}

impl BindConfigCacheProxy {
    pub fn new( skel :StarSkel ) -> Self {
        Self {
            skel
        }
    }
}

#[async_trait]
impl BindConfigCache for BindConfigCacheProxy {
    async fn get_bind_config(&self, particle: &Point) -> anyhow::Result<ArtifactItem<CachedConfig<BindConfig>>> {
        self.skel.machine.get_proto_artifact_caches_factory().await?.root_caches().get_bind_config(particle).await
    }
}
