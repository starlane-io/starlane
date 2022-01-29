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
use crate::mesh::serde::entity::request::{Http, Msg, Rc, ReqEntity};
use crate::mesh::serde::fail;
use crate::mesh::serde::http::HttpRequest;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::pattern::TksPattern;
use crate::mesh::serde::payload::Payload;
use crate::mesh::serde::payload::Primitive;
use crate::mesh::serde::resource::command::common::StateSrc;
use crate::mesh::serde::resource::command::create::{AddressSegmentTemplate, KindTemplate, Strategy};
use crate::mesh::serde::resource::command::RcCommand;
use crate::mesh::serde::resource::ResourceStub;
use crate::mesh::serde::resource::Status;
use crate::mesh::Request;
use crate::mesh::Response;
use crate::message::delivery::Delivery;
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind};
use crate::resource::{ArtifactKind, Kind, ResourceType,BaseKind, FileKind};
use crate::resource::{AssignKind, ResourceAssign, ResourceRecord};
use crate::star::core::resource::registry::{RegError, Registration};
use crate::star::shell::wrangler::{ StarFieldSelection, StarSelector};
use crate::star::{StarCommand, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use mesh_portal_serde::version::latest::fail::BadRequest;
use std::future::Future;
use mesh_portal_serde::version::v0_0_1::generic::payload::PayloadType::Primitive;
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
                    let delivery = Delivery::new(rc.clone(), star_message, self.skel.clone());
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
                    }
                    ResourceHostAction::Init(_) => {}
                }
            }
            _ => {}
        }
        Ok(())
    }

    async fn process_resource_command(&mut self, delivery: Delivery<Rc>)  {
        let skel = self.skel.clone();
        let resource_manager_api = self.resource_manager_api.clone();
        tokio::spawn(async move {
            async fn process(skel: StarSkel, resource_manager_api: ResourceManagerApi, rc: &Rc, to: Address) -> Result<Payload, Error> {
                match &rc.command {
                    RcCommand::Create(create) => {
                        let kind = match_kind(&create.template.kind)?;
                        let stub = match &create.template.address.child_segment_template {
                            AddressSegmentTemplate::Exact(child_segment) => {
                                let address = create.template.address.parent.push(child_segment.clone())?;
                                let registration = Registration {
                                    address: address.clone(),
                                    kind: kind.clone(),
                                    registry: create.registry.clone(),
                                    properties: create.properties.clone(),
                                };

                                skel.registry_api.register(registration).await?

                            }
                            AddressSegmentTemplate::Pattern(pattern) => {
                                if !pattern.contains("%") {
                                    return Err("AddressSegmentTemplate::Pattern must have at least one '%' char for substitution".into());
                                }
                                loop {
                                    let index = skel.registry_api.sequence(create.template.address.parent.clone()).await?;
                                    let child_segment = pattern.replace( "%", index.as_str() );
                                    let address = create.template.address.parent.push(child_segment.clone())?;
                                    let registration = Registration {
                                        address,
                                        kind: kind.clone(),
                                        registry: create.registry.clone(),
                                        properties: create.properties.clone(),
                                    };

                                    match skel.registry_api.register(registration).await {
                                        Ok(stub) => {
                                            break stub;
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

                        if Strategy::AlreadyHosted = create.strategy.clone() {
                            return Ok(Payload::Primitive(Primitive::Stub(stub)));
                        }


                        async fn assign(
                            skel: StarSkel,
                            stub: ResourceStub,
                            state: StateSrc,
                        ) -> Result<(), Error> {
                            let star_kind = StarKind::hosts(&stub.kind.resource_type());
                            let mut star_selector = StarSelector::new();
                            star_selector.add(StarFieldSelection::Kind(star_kind.clone()));
                            let wrangle = skel.star_wrangler_api.next(star_selector).await?;
                            let mut proto = ProtoStarMessage::new();
                            proto.to(ProtoStarMessageTo::Star(wrangle.key.clone()));
                            let assign = ResourceAssign::new(AssignKind::Create, stub, state);
                            proto.payload = StarMessagePayload::ResourceHost(
                                ResourceHostAction::Assign(assign),
                            );
                            skel.messaging_api
                                .star_exchange(proto, ReplyKind::Empty, "assign resource to host")
                                .await?;
                            Ok(())
                        }

                        match assign(skel.clone(), stub, create.state.clone()).await {
                            Ok(_) => {
                                Ok(Payload::Empty)
                            },
                            Err(fail) => {
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
                        let list = Payload::List( skel.registry_api.select(select.clone(), to).await? );
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
                }
            }
            let result = process(skel,resource_manager_api.clone(), &delivery.item, delivery.to().expect("expected this to work since we have already established that the item is a Request")).await.into();
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
    })
}
pub struct WrappedHttpRequest {
    pub resource: Address,
    pub request: HttpRequest,
}
