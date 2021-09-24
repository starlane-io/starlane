use std::convert::TryInto;

use tokio::sync::{mpsc, oneshot};

use starlane_resources::{Resource, ResourceArchetype};
use starlane_resources::message::{Message, ResourceRequestMessage, ResourceResponseMessage, ResourcePortMessage};
use starlane_resources::message::Fail;

use starlane_resources::data::DataSet;
use crate::error::Error;
use crate::frame::{
    MessagePayload, RegistryAction, Reply, ResourceHostAction, SimpleReply, StarMessage,
    StarMessagePayload,
};
use crate::message::resource::Delivery;
use crate::resource::{Parent, ParentCore, ResourceAddress, ResourceId, ResourceKey, ResourceManager, ResourceRecord};
use crate::resource::{ResourceKind, ResourceType};
use crate::star::{StarCommand, StarKind, StarSkel};
use crate::star::core::resource::host::{HostCall, HostComponent};
use crate::star::shell::pledge::ResourceHostSelector;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::oneshot::error::RecvError;
use starlane_resources::property::{ResourcePropertyAssignment, ResourcePropertyValueSelector, ResourceValue, ResourceValues};
use std::collections::HashMap;

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
            StarMessagePayload::MessagePayload(message_payload) => match &message_payload {
                MessagePayload::Request(request) => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.process_resource_request(delivery).await?;
                }
                MessagePayload::PortRequest(request) => {
info!("Received PORT request!");
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.process_resource_port_request(delivery).await?;
                }
                _ => {}
            },
            StarMessagePayload::ResourceManager(action) => {
                let delivery = Delivery::new(action.clone(), star_message, self.skel.clone());
                self.process_registry_action(delivery).await?;
            }
            StarMessagePayload::ResourceHost(action) => {
                let delivery = Delivery::new(action.clone(), star_message, self.skel.clone());
                self.process_resource_host_action(delivery).await?;
            }

            _ => {}
        }
        Ok(())
    }

    async fn process_resource_request(
        &mut self,
        delivery: Delivery<Message<ResourceRequestMessage>>,
    ) -> Result<(), Error> {
        let skel = self.skel.clone();
        let host_tx = self.host_tx.clone();
        tokio::spawn(async move {
            async fn process(
                skel: StarSkel,
                host_tx: mpsc::Sender<HostCall>,
                delivery: Delivery<Message<ResourceRequestMessage>>,
            ) -> Result<(), Error> {
                match delivery.payload.payload.clone() {
                    ResourceRequestMessage::Create(create) => {
                        let parent_key = match create
                            .parent
                            .clone()
                            .key_or("expected parent to be a ResourceKey")
                        {
                            Ok(key) => key,
                            Err(error) => {
                                return Err(error.to_string().into());
                            }
                        };
                        let parent = MessagingEndpointComponent::get_parent_resource(
                            skel.clone(),
                            parent_key,
                        )
                        .await?;
                        let record = parent.create(create.clone()).await.await;

                        match record {
                            Ok(record) => match record {
                                Ok(record) => {
                                    delivery.reply(Reply::Record(record));
                                }
                                Err(fail) => {
                                    delivery.fail(fail);
                                }
                            },
                            Err(err) => {
                                eprintln!("Error: {}", err);
                            }
                        }
                    }
                    ResourceRequestMessage::Select(selector) => {
                        let resources = skel
                            .registry
                            .as_ref()
                            .unwrap()
                            .select(selector.clone())
                            .await?;
                        delivery.reply(Reply::Records(resources))
                    }
                    ResourceRequestMessage::Unique(resource_type) => {
                        let resource = skel.resource_locator_api.locate(delivery.payload.to.clone() ).await?;
                        let unique_src = skel
                                .registry
                                .as_ref()
                            .unwrap()
                            .unique_src(resource.stub.archetype.kind.resource_type(), delivery.payload.to.clone().into())
                            .await;
                        delivery.reply(Reply::Id(unique_src.next(&resource_type).await?));
                    }
                    ResourceRequestMessage::SelectValues(selector) => {
                        let stub = skel.resource_locator_api.locate(delivery.payload.to.clone()).await?.stub;

println!("{:?}",stub);

                        match selector {
                            ResourcePropertyValueSelector::Config => {
                                let value = ResourceValue::Config(stub.archetype.config.clone());
                                let mut values = HashMap::new();
                                values.insert( selector, value );
                                let mut values = ResourceValues::new(stub, values);
                                delivery.reply(Reply::ResourceValues(values));
                                return Ok(())
                            }
                            _ => {
                                // handle at the host level below
                            }
                        }


                        let key: ResourceKey = skel.resource_locator_api.as_key(delivery.payload.to.clone()).await?;
                        let (tx, rx) = oneshot::channel();


                        host_tx.send(HostCall::Select { key, selector, tx }).await?;
                        let result = rx.await;
                        if let Ok(Ok(Option::Some(values))) = result {
                            let values = values.with(stub);
                            delivery.reply(Reply::ResourceValues(values));
                        } else {
                            delivery.fail(Fail::expected("Ok(Ok(ResourceValues(values)))"));
                        }
                    }
                    ResourceRequestMessage::UpdateState(state) => {
                        let key: ResourceKey = skel.resource_locator_api.as_key(delivery.payload.to.clone()).await?;
                        let (tx, rx) = oneshot::channel();
                        host_tx.send(HostCall::UpdateState{ key, state, tx }).await?;
                        let result = rx.await;

                        match result {
                            Ok(Ok(())) => {
                                delivery.reply(Reply::Empty);
                            }
                            Ok(Err(error)) => {
                                eprintln!("result error: {}", error.to_string() );
                                delivery.fail(Fail::expected("Ok(Ok(()))"));
                            }
                            Err(error) => {
                                eprintln!("Recv Error");
                                delivery.fail(Fail::expected("Ok(Ok(()))"));
                            }
                        }

                    }
                    ResourceRequestMessage::Set(property) => {

                        let assignment = ResourcePropertyAssignment {
                            resource: delivery.payload.to.clone(),
                            property
                        };
println!("assigning : {} on star: {}", assignment.resource.to_string(), skel.info.kind.to_string() );
                        skel
                            .registry
                            .as_ref()
                            .unwrap()
                            .update(assignment)
                            .await?;
                        delivery.reply(Reply::Empty)
                    }
                }
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

    async fn process_resource_port_request(
        &mut self,
        delivery: Delivery<Message<ResourcePortMessage>>,
    ) -> Result<(), Error> {
        let skel = self.skel.clone();
        let host_tx = self.host_tx.clone();
        tokio::spawn(async move {
            async fn process(
                skel: StarSkel,
                host_tx: mpsc::Sender<HostCall>,
                delivery: Delivery<Message<ResourcePortMessage>>,
            ) -> Result<(), Error> {
                host_tx.try_send( HostCall::Deliver(delivery)).unwrap_or_default();
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
        delivery: Delivery<RegistryAction>,
    ) -> Result<(), Error> {
        let skel = self.skel.clone();

        tokio::spawn(async move {
            if let Option::Some(registry) = skel.registry.clone() {
                match &delivery.payload {
                    RegistryAction::Register(registration) => {
                        let result = registry.register(registration.clone()).await;
                        delivery.result_ok(result);
                    }
                    RegistryAction::Location(location) => {
                        let result = registry.set_location(location.clone()).await;
                        delivery.result_ok(result);
                    }
                    RegistryAction::Find(find) => {

                        let result = registry.get(find.to_owned()).await;

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
                    RegistryAction::Status(_report) => {
                        unimplemented!()
                    }
                    RegistryAction::UniqueResourceId { parent, child_type } => {
                        match skel.resource_locator_api.locate(parent.clone() ).await {
                            Ok(parent) => {
                                let unique_src = skel
                                    .registry
                                    .as_ref()
                                    .unwrap()
                                    .unique_src(parent.stub.archetype.kind.resource_type(), parent.stub.key.into() )
                                    .await;
                                let result: Result<ResourceId, Error> = unique_src.next(child_type).await;
                                match result {
                                    Ok(id) => {
                                        delivery.reply(Reply::Id(id));
                                    }
                                    Err(fail) => {
                                        delivery.fail(fail.into());
                                    }
                                }
                            }
                            Err(fail) => {
                                delivery.fail(fail.into());
                            }
                        }

                    }
                }
            }
        });

        Ok(())
    }

    async fn process_resource_host_action(
        &self,
        delivery: Delivery<ResourceHostAction>,
    ) -> Result<(), Error> {
        match &delivery.payload {
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
                let call = HostCall::Init{
                    key: key.clone(),
                    tx
                };
                self.host_tx.try_send(call).unwrap_or_default();
                delivery.result_rx(rx);
            }
        }
        Ok(())
    }

    async fn get_parent_resource(skel: StarSkel, key: ResourceKey) -> Result<Parent, Error> {
        let resource = skel.resource_locator_api.locate(key.clone().into()).await?;

        Ok(Parent {
            core: ParentCore {
                stub: resource.into(),
                selector: ResourceHostSelector::new(skel.clone()),
                child_registry: skel.registry.as_ref().unwrap().clone(),
                skel: skel.clone(),
            },
        })
    }

    pub async fn has_resource(&self, key: &ResourceKey) -> Result<bool, Error> {
        let (tx, mut rx) = oneshot::channel();
        self.host_tx
            .send(HostCall::Has {
                key: key.clone(),
                tx,
            })
            .await?;
        Ok(rx.await?)
    }
}
