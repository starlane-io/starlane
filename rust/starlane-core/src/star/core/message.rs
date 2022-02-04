use std::collections::HashMap;
use std::convert::TryInto;

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
use crate::resource::{ArtifactKind, Kind, ResourceType, BaseKind, FileKind, ResourceLocation};
use crate::resource::{AssignKind, ResourceAssign, ResourceRecord};
use crate::star::core::resource::registry::{RegError, Registration};
use crate::star::shell::wrangler::{ StarFieldSelection, StarSelector};
use crate::star::{StarCommand, StarKey, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use mesh_portal_serde::version::latest::fail::BadRequest;
use std::future::Future;
use mesh_portal_serde::version::latest::command::common::StateSrc;
use mesh_portal_serde::version::latest::entity::request::create::{AddressSegmentTemplate, KindTemplate, Strategy};
use mesh_portal_serde::version::latest::entity::request::{Rc, RcCommand, ReqEntity};
use mesh_portal_serde::version::latest::http::HttpRequest;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::Request;
use mesh_portal_serde::version::latest::payload::{Payload, Primitive};
use mesh_portal_serde::version::latest::resource::{ResourceStub, Status};
use mesh_portal_versions::version::v0_0_1::id::Tks;
use serde::de::Unexpected::Str;
use crate::star::core::resource::manager::{ResourceManagerApi, ResourceManagerComponent};

pub enum CoreMessageCall {
    Message(StarMessage),
}

impl Call for CoreMessageCall {}

pub struct MessagingEndpointComponent {
    skel: StarSkel,
    resource_manager_api: ResourceManagerApi
}

impl MessagingEndpointComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<CoreMessageCall>) {
        let (resource_manager_tx, resource_manager_rx) = mpsc::channel(1024);
        let resource_manager_api= ResourceManagerApi::new(resource_manager_tx.clone());
        ResourceManagerComponent::new(skel.clone(), resource_manager_tx, resource_manager_rx );

        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                resource_manager_api
            }),
            skel.core_messaging_endpoint_tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<CoreMessageCall> for MessagingEndpointComponent {
    async fn process(&mut self, call: CoreMessageCall) {
        match call {
            CoreMessageCall::Message(message) => match self.process_resource_message(message).await
            {
                Ok(_) => {}
                Err(err) => {
                    error!("{}", err);
                }
            },
        }
    }
}

impl MessagingEndpointComponent {
    async fn process_resource_message(&mut self, star_message: StarMessage) -> Result<(), Error> {
        match &star_message.payload {
            StarMessagePayload::Request(request) => match &request.entity {
                ReqEntity::Rc(rc) => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.process_resource_command(delivery).await;
                }
                _ => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.resource_manager_api.request(delivery).await;
                }
            },

            StarMessagePayload::ResourceHost(action) => {
                match action {
                    ResourceHostAction::Assign(assign) => {
                        self.resource_manager_api.assign(assign.clone()).await;
                        let reply = star_message.ok(Reply::Empty);
                        self.skel.messaging_api.star_notify(reply);
                    }
                    ResourceHostAction::Init(_) => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn process_resource_command(&mut self, delivery: Delivery<Request>)  {
        let skel = self.skel.clone();
        let resource_manager_api = self.resource_manager_api.clone();
        tokio::spawn(async move {
            async fn process(skel: StarSkel, resource_manager_api: ResourceManagerApi, rc: &Rc, to: Address) -> Result<Payload, Error> {
                match &rc.command {
                    RcCommand::Create(create) => {
                        let kind = match_kind(&create.template.kind)?;
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
                                let registration = Registration {
                                    address: address.clone(),
                                    kind: kind.clone(),
                                    registry: create.registry.clone(),
                                    properties: create.properties.clone(),
                                };

                                let mut result = skel.registry_api.register(registration).await;

                                // if strategy is ensure then a dupe is GOOD!
                                if create.strategy == Strategy::Ensure {
                                    if let Err(RegError::Dupe) = result {
                                        result = Ok(skel.resource_locator_api.locate(address).await?.stub);
                                    }
                                }

                                result?
                            }
                            AddressSegmentTemplate::Pattern(pattern) => {
                                if !pattern.contains("%") {
                                    return Err("AddressSegmentTemplate::Pattern must have at least one '%' char for substitution".into());
                                }
                                loop {
                                    let index = skel.registry_api.sequence(create.template.address.parent.clone()).await?;
                                    let child_segment = pattern.replace( "%", index.to_string().as_str() );
                                    let address = create.template.address.parent.push(child_segment.clone())?;
                                    let registration = Registration {
                                        address: address.clone(),
                                        kind: kind.clone(),
                                        registry: create.registry.clone(),
                                        properties: create.properties.clone(),
                                    };

                                    match skel.registry_api.register(registration).await {
                                        Ok(stub) => {
                                            if let Strategy::HostedBy(key) = &create.strategy {
                                                let key = StarKey::from_str( key.as_str() )?;
//                                                let location = ResourceLocation::new(key);
                                                skel.registry_api.assign(address, key).await?;
                                                return Ok(Payload::Primitive(Primitive::Stub(stub)));
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



                        async fn assign(
                            skel: StarSkel,
                            stub: ResourceStub,
                            state: StateSrc,
                        ) -> Result<(), Error> {

                            let star_kind = StarKind::hosts(&ResourceType::from_str(stub.kind.resource_type().as_str())?);
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
                            skel.messaging_api
                                .star_exchange(proto, ReplyKind::Empty, "assign resource to host")
                                .await?;
                            Ok(())
                        }

                        match assign(skel.clone(), stub.clone(), create.state.clone()).await {
                            Ok(_) => {
                                Ok(Payload::Primitive(Primitive::Stub(stub)))
                            },
                            Err(fail) => {
                                eprintln!("{}",fail.to_string() );
                                skel.registry_api
                                    .set_status(
                                        to,
                                        Status::Panic(
                                            "could not assign resource to host".to_string(),
                                        ),
                                    )
                                    .await;
                                Err(fail.into())
                            }
                        }
                    }
                    RcCommand::Select(select) => {
                        let list = Payload::List( skel.registry_api.select(select.clone()).await? );
                        Ok(list)
                    },
                    RcCommand::Update(_) => {
                        unimplemented!()
                    }
                    RcCommand::Query(query) => {
                        let result = Payload::Primitive(Primitive::Text(
                        skel.registry_api
                            .query(to, query.clone())
                            .await?
                            .to_string(),
                         ));
                        Ok(result)
                    },
                    RcCommand::Get => {
                        resource_manager_api.get(  to).await
                    }
                    RcCommand::Set(set) => {
                        let set = set.clone();
                        skel.registry_api.set_properties(set.address, set.properties).await?;
                        Ok(Payload::Empty)
                    }

                }
            }
            let rc = match &delivery.item.entity {
                ReqEntity::Rc(rc) => {rc}
                _ => { panic!("should not get requests that are not Rc") }
            };
            let result = process(skel,resource_manager_api.clone(), rc, delivery.to().expect("expected this to work since we have already established that the item is a Request")).await.into();

            delivery.result(result);
        });
    }

    pub async fn has_resource(&self, key: &Address) -> Result<bool, Error> {
        Ok(self.resource_manager_api.has( key.clone() ).await?)
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
        ResourceType::Control => Kind::Control
    })
}
pub struct WrappedHttpRequest {
    pub resource: Address,
    pub request: HttpRequest,
}
