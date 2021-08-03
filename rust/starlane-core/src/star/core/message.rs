use std::convert::TryInto;
use tokio::sync::{mpsc, oneshot};

use crate::data::DataSet;
use crate::error::Error;
use crate::frame::{MessagePayload, RegistryAction, Reply, ResourceHostAction, SimpleReply, StarMessage, StarMessagePayload};
use crate::message::Fail;
use crate::message::resource::{
    Delivery, Message, ResourceRequestMessage, ResourceResponseMessage,
};
use crate::resource::{Parent, ParentCore, Resource, ResourceAddress, ResourceArchetype, ResourceKey, ResourceManager, ResourceRecord, ResourceId};
use crate::resource::{ResourceKind, ResourceType};
use crate::star::{StarCommand, StarKind, StarSkel};
use crate::star::core::resource::host::{HostCall, HostComponent};
use crate::star::shell::pledge::ResourceHostSelector;
use crate::util::{AsyncProcessor, AsyncRunner, Call};


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
        AsyncRunner::new(Box::new(Self { skel:skel.clone(), host_tx }), skel.core_messaging_endpoint_tx.clone(), rx);
    }
}

#[async_trait]
impl AsyncProcessor<CoreMessageCall> for MessagingEndpointComponent {
    async fn process(&mut self, call: CoreMessageCall) {
        match call {
            CoreMessageCall::Message(message) => match self.process_resource_message(message).await {
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
        //println!("process_message---> {}", message.payload.to_string() );
        match &star_message.payload {
            StarMessagePayload::MessagePayload(message_payload) => match &message_payload {
                MessagePayload::Request(request) => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.process_resource_request(delivery)
                        .await?;
                }
                _ => {}
            }
            StarMessagePayload::ResourceManager(action) => {
                let delivery = Delivery::new(action.clone(), star_message, self.skel.clone());
                self.process_registry_action(delivery )
                    .await?;
            }
            StarMessagePayload::ResourceHost(action) => {
                let delivery = Delivery::new(action.clone(), star_message, self.skel.clone());
                self.process_resource_host_action(delivery )
                    .await?;
            }

            _ => {}
        }
        Ok(())
    }

    async fn process_resource_request(
        &mut self,
        delivery: Delivery<Message<ResourceRequestMessage>>,
    ) -> Result<(), Error> {
println!("Process resource request....");
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
                let parent = self.get_parent_resource(parent_key).await?;
                tokio::spawn(async move {
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
                });
            }
            ResourceRequestMessage::Select(selector) => {
info!("select resource.");
                let resources = self
                    .skel
                    .registry
                    .as_ref()
                    .unwrap()
                    .select(selector.clone())
                    .await?;
                delivery.reply(Reply::Records(resources))
            }
            ResourceRequestMessage::Unique(resource_type) => {
                let unique_src = self
                    .skel
                    .registry
                    .as_ref()
                    .unwrap()
                    .unique_src(delivery.payload.to.clone().into())
                    .await;
                delivery.reply(Reply::Id( unique_src.next(&resource_type).await? ));
            }
            ResourceRequestMessage::State => {
                let key: ResourceKey = delivery.payload.to.clone().try_into()?;

                let (tx, rx) = oneshot::channel();
                self.host_tx.send(HostCall::Get { key, tx }).await?;
                tokio::spawn(async move {
                    let result = rx.await;
                    if let Ok(Ok(Option::Some(state))) = result {
                        let state = match state.try_into() {
                            Ok(state) => state,
                            Err(_) => {
                                error!("error when try_into from BinSrc to NetworkBinSrc");
                                delivery.fail(Fail::expected( "Ok(Ok(StarCoreResult::State(state)))"));
                                return;
                            }
                        };

                        delivery.reply(Reply::State(state));
                    } else {
                        delivery.fail(Fail::expected( "Ok(Ok(StarCoreResult::State(state)))"));
                    }
                });
            }
        }
        Ok(())
    }

    async fn process_registry_action(
        &mut self,
        delivery: Delivery<RegistryAction>
    ) -> Result<(), Error> {

        let skel = self.skel.clone();

        tokio::spawn( async move {
            if let Option::Some(manager) = skel.registry.clone() {
                match &delivery.payload {
                    RegistryAction::Register(registration) => {
                        let result = manager.register(registration.clone()).await;
                        delivery.result_ok(result);
                    }
                    RegistryAction::Location(location) => {
                        let result = manager.set_location(location.clone()).await;
                        delivery.result_ok(result);
                    }
                    RegistryAction::Find(find) => {
                        let result = manager.get(find.to_owned()).await;

                        match result {
                            Ok(result) => match result {
                                Some(record) => {
                                    delivery.reply(Reply::Record(record))
                                }
                                None => {
                                    delivery.fail(Fail::ResourceNotFound(find.clone()));
                                }
                            },
                            Err(fail) => {
                                delivery.fail(fail);
                            }
                        }
                    }
                    RegistryAction::Status(_report) => {
                        unimplemented!()
                    }
                    RegistryAction::UniqueResourceId { parent, child_type } => {
                        let unique_src = skel.registry.as_ref().unwrap().unique_src(parent.clone()).await;
                        let result: Result<ResourceId, Fail> = unique_src.next(child_type).await;
                        match result {
                            Ok(id) => {
                                delivery.reply(Reply::Id(id));
                            }
                            Err(fail) => {
                                delivery.fail(fail);
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
                let call = HostCall::Assign { assign:assign.clone(), tx };
                self.host_tx.try_send(call).unwrap_or_default();
                delivery.result_rx(rx);
            }
        }
        Ok(())
    }

    async fn get_parent_resource(&mut self, key: ResourceKey) -> Result<Parent, Fail> {
println!("getting parent resource for {}", key.to_string() );
        let resource = self
            .skel
            .registry
            .as_ref()
            .expect("expected registry").get(key.into()).await?.ok_or("expected parent resource")?;


        Ok(Parent {
            core: ParentCore {
                stub: resource.into(),
                selector: ResourceHostSelector::new(self.skel.clone()),
                child_registry: self.skel.registry.as_ref().unwrap().clone(),
                skel: self.skel.clone(),
            },
        })
    }

    pub async fn has_resource(&self, key: &ResourceKey) -> Result<bool, Error> {
        let (tx, mut rx) = oneshot::channel();
        self.host_tx.send(HostCall::Has { key: key.clone(), tx }).await?;
        Ok(rx.await?)
    }
}
