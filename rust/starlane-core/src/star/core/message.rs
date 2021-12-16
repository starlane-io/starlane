use std::collections::HashMap;
use std::convert::TryInto;

use tokio::sync::oneshot::error::RecvError;
use tokio::sync::{mpsc, oneshot};

use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::{
    ResourceHostAction, ResourceRegistryRequest, SimpleReply, StarMessage, StarMessagePayload,
};
use crate::mesh::serde::entity::request::{Http, Msg, Rc, ReqEntity};
use crate::mesh::serde::fail;
use crate::mesh::serde::http::HttpRequest;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::pattern::TksPattern;
use crate::mesh::serde::payload::Payload;
use crate::mesh::serde::payload::Primitive;
use crate::mesh::serde::resource::ResourceStub;
use crate::mesh::serde::resource::command::create::{AddressSegmentTemplate, KindTemplate};
use crate::mesh::serde::resource::command::RcCommand;
use crate::mesh::Message;
use crate::mesh::Request;
use crate::mesh::Response;
use crate::message::delivery::Delivery;
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind};
use crate::resource::{ResourceRecord, ResourceAssign, AssignKind};
use crate::resource::{Kind, ResourceType};
use crate::star::core::resource::registry::{Parent, ParentCore, Registration};
use crate::star::core::resource::shell::{HostCall, HostComponent};
use crate::star::shell::wrangler::{ResourceHostSelector, StarSelector, StarFieldSelection};
use crate::star::{StarCommand, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use mesh_portal_serde::version::latest::fail::BadRequest;
use std::future::Future;
use crate::mesh::serde::resource::Status;
use crate::mesh::serde::resource::command::common::StateSrc;

pub enum CoreMessageCall {
    Message(StarMessage),
}

impl Call for CoreMessageCall {}

pub struct MessagingEndpointComponent {
    skel: StarSkel,
    host_tx: mpsc::Sender<HostCall>,
}

impl MessagingEndpointComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<CoreMessageCall>) {
        let host_tx = HostComponent::new(skel.clone());
        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                host_tx,
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
            StarMessagePayload::Request(message_payload) => match &message_payload {
                Message::Request(request) => match &request.entity {
                    ReqEntity::Rc(rc) => {
                        let delivery = Delivery::new(rc.clone(), star_message, self.skel.clone());
                        self.process_resource_command(delivery).await?;
                    }
                    _ => {
                        self.handle_by_host(delivery).await?;
                    }
                },
                Message::Response(response) => {
                    // we don't handle responses here
                }
            },
            StarMessagePayload::ResourceRegistry(action) => {
                let delivery = Delivery::new(action.clone(), star_message, self.skel.clone());
                self.process_registry_action(delivery).await?;
            }
            StarMessagePayload::ResourceHost(action) => {
                let delivery = Delivery::new(action.clone(), star_message, self.skel.clone());
                self.process_resource_host_action(delivery).await?;
            }

            /*            StarMessagePayload::Select(selector) => {
                           let delivery = Delivery::new(selector.clone(), star_message.clone(), self.skel.clone());
                           let results = self.skel.registry.as_ref().unwrap().select(selector.clone()).await;
                           match results {
                               Ok(records) => {
                                   delivery.reply(Reply::Records(records))
                               }
                               Err(error) => {
                                   delivery.fail(Fail::Error("could not select records".to_string()))
                               }
                           }
                       }
            */
            _ => {}
        }
        Ok(())
    }

    async fn process_resource_command(&mut self, delivery: Delivery<Rc>) -> Result<(), Error> {
        let skel = self.skel.clone();
        tokio::spawn(async move {
            async fn process(skel: StarSkel, rc: &Rc, to: Address) -> Result<(), Fail> {
                match &rc.command {
                    RcCommand::Create(create) => {
                        let address = match &create.template.address.child_segment_template {
                            AddressSegmentTemplate::Exact(child_segment) => {
                                create.template.address.parent.push(child_segment.clone())?
                            }
                        };

                        let kind = match_kind(&create.template.kind)?;

                        let registration = Registration {
                            address: address.clone(),
                            kind: kind.clone(),
                            registry: create.registry.clone(),
                            properties: create.properties.clone(),
                        };

                        let stub = skel.registry_api.register(registration).await?;

                        async fn assign( skel: StarSkel, stub: ResourceStub, state: StateSrc ) -> Result<(),Fail> {
                            let star_kind = StarKind::hosts(&stub.kind.resource_type());
                            let mut star_selector = StarSelector::new();
                            star_selector.add(StarFieldSelection::Kind(star_kind.clone()));
                            let wrangle= skel.star_wrangler_api.next(star_selector).await?;
                            let mut proto = ProtoStarMessage::new();
                            proto.to(ProtoStarMessageTo::Star(wrangle.key.clone()));
                            let assign = ResourceAssign::new(AssignKind::Create, stub, state );
                            proto.payload( StarMessagePayload::ResourceHost(ResourceHostAction::Assign(assign)) );
                            skel.messaging_api.star_exchange(proto,ReplyKind::Empty, "assign resource to host").await?;
                            Ok(())
                        }

                        match assign( skel, stub, create.state.clone() ).await {
                            Ok(_) => {
                                Ok(())
                            }
                            Err(fail) => {
                                skel.registry_api.set_status(to, Status::Panic( "could not assign resource to host".to_string())).await;
                                Err(fail)
                            }
                        }
                    }
                    RcCommand::Select(select) => {
                        Ok(Payload::List(skel.registry_api.select(select.clone(), to).await?))
                    }
                    RcCommand::Update(_) => {
                        unimplemented!()
                    }
                    RcCommand::Query(query) => Ok(Payload::Primitive(Primitive::Text(
                        skel.registry_api
                            .query(to, query.clone())
                            .await?
                            .to_string(),
                    ))),
                }
            }

            delivery.result(process(skel, &delivery.item, delivery.to()));
        });
        Ok(())
    }

    async fn process_resource_port_request(
        &mut self,
        delivery: Delivery<Msg>,
    ) -> Result<(), Error> {
        let skel = self.skel.clone();
        let host_tx = self.host_tx.clone();
        tokio::spawn(async move {
            async fn process(
                skel: StarSkel,
                host_tx: mpsc::Sender<HostCall>,
                delivery: Delivery<Msg>,
            ) -> Result<(), Error> {
                host_tx
                    .try_send(HostCall::Port(delivery))
                    .unwrap_or_default();
                Ok(())
            }

            match process(skel, host_tx, delivery).await {
                Ok(_) => {}
                Err(err) => {
                    error!("{}", err.to_string());
                }
            }
        });
        Ok(())
    }

    async fn process_resource_http_request(
        &mut self,
        delivery: Delivery<Http>,
    ) -> Result<(), Error> {
        let skel = self.skel.clone();
        let host_tx = self.host_tx.clone();
        tokio::spawn(async move {
            async fn process(
                skel: StarSkel,
                host_tx: mpsc::Sender<HostCall>,
                delivery: Delivery<Http>,
            ) -> Result<(), Error> {
                host_tx
                    .try_send(HostCall::Http(delivery))
                    .unwrap_or_default();
                Ok(())
            }

            match process(skel, host_tx, delivery).await {
                Ok(_) => {}
                Err(err) => {
                    error!("{}", err.to_string());
                }
            }
        });
        Ok(())
    }

    async fn process_registry_action(
        &mut self,
        delivery: Delivery<ResourceRegistryRequest>,
    ) -> Result<(), Error> {
        let skel = self.skel.clone();

        tokio::spawn(async move {
            async fn process(
                skel: StarSkel,
                delivery: Delivery<ResourceRegistryRequest>,
            ) -> Result<(), Error> {
                if let Option::Some(registry) = skel.registry.clone() {
                    match &delivery.item {
                        ResourceRegistryRequest::Register(registration) => {
                            let result = registry.register(registration.clone()).await;
                            delivery.result_ok(result);
                        }
                        ResourceRegistryRequest::Location(location) => {
                            let result = registry.set_location(location.clone()).await;
                            delivery.result_ok(result);
                        }
                        ResourceRegistryRequest::Find(find) => {
                            let result = registry.locate(find.to_owned()).await;

                            match result {
                                Ok(result) => match result {
                                    Some(record) => delivery.reply(Reply::Record(record)),
                                    None => {
                                        delivery.fail(Fail::ResourceNotFound(find.clone()));
                                    }
                                },
                                Err(fail) => {
                                    delivery.fail(fail.into());
                                }
                            }
                        }
                        ResourceRegistryRequest::Status(_report) => {
                            unimplemented!()
                        }
                    }
                }
                Ok(())
            }

            match process(skel, delivery).await {
                Ok(_) => {}
                Err(error) => {
                    eprintln!(
                        "error when processing registry action: {}",
                        error.to_string()
                    );
                }
            }
        });

        Ok(())
    }

    async fn process_resource_host_action(
        &self,
        delivery: Delivery<ResourceHostAction>,
    ) -> Result<(), Error> {
        match &delivery.item {
            ResourceHostAction::Assign(assign) => {
                let (tx, rx) = oneshot::channel();
                let call = HostCall::Assign {
                    assign: assign.clone(),
                    tx,
                };
                self.host_tx.try_send(call).unwrap_or_default();
                delivery.result_rx(rx);
            }
            ResourceHostAction::Init(key) => {
                let (tx, rx) = oneshot::channel();
                let call = HostCall::Init {
                    key: key.clone(),
                    tx,
                };
                self.host_tx.try_send(call).unwrap_or_default();
                delivery.result_rx(rx);
            }
        }
        Ok(())
    }

    async fn get_parent_resource(skel: StarSkel, address: Address) -> Result<Parent, Error> {
        let resource = skel
            .resource_locator_api
            .locate(address.clone().into())
            .await?;

        Ok(Parent {
            core: ParentCore {
                stub: resource.into(),
                selector: ResourceHostSelector::new(skel.clone()),
                child_registry: skel.registry.as_ref().unwrap().clone(),
                skel: skel.clone(),
            },
        })
    }

    pub async fn has_resource(&self, key: &Address) -> Result<bool, Error> {
        let (tx, mut rx) = oneshot::channel();
        self.host_tx
            .send(HostCall::Has {
                address: key.clone(),
                tx,
            })
            .await?;
        Ok(rx.await?)
    }
}
pub fn match_kind(template: &KindTemplate) -> Result<Kind, Fail> {
    let resource_type: ResourceType = ResourceType::from_str(template.resource_type.as_str())?;
    Ok(match resource_type {
        ResourceType::Root => Kind::Root,
        ResourceType::Space => Kind::Space,
        ResourceType::Base => Kind::Base,
        ResourceType::User => Kind::User,
        ResourceType::App => Kind::App,
        ResourceType::Mechtron => Kind::Mechtron,
        ResourceType::FileSystem => Kind::FileSystem,
        ResourceType::File => Kind::File,
        ResourceType::Database => {
            unimplemented!("need to right a SpecificPattern matcher...")
        }
        ResourceType::Authenticator => Kind::Authenticator,
        ResourceType::ArtifactBundleSeries => Kind::ArtifactBundleSeries,
        ResourceType::ArtifactBundle => Kind::ArtifactBundle,
        ResourceType::Artifact => {
            let artifact_kind = ArtifactKind::from_str(template.kind.ok_or(Err(Fail::Fail(
                fail::Fail::Resource(fail::resource::Fail::BadRequest(BadRequest::Bad(
                    fail::Bad::Kind("ArtifactKind cannot be None".to_string()),
                ))),
            ))))?;
            Kind::Artifact(artifact_kind)
        }
        ResourceType::Proxy => Kind::Proxy,
        ResourceType::Credentials => Kind::Credentials,
    })
}
pub struct WrappedHttpRequest {
    pub resource: Address,
    pub request: HttpRequest,
}
