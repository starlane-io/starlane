use crate::data::DataSet;
use crate::error::Error;
use crate::frame::{RegistryAction, MessagePayload, Reply, SimpleReply, StarMessage, StarMessagePayload, ResourceHostAction};
use crate::message::resource::{
    Delivery, Message, ResourceRequestMessage, ResourceResponseMessage,
};
use crate::message::Fail;
use crate::resource::{
    Parent, ParentCore, Resource, ResourceAddress, ResourceArchetype, ResourceKey, ResourceManager,
    ResourceRecord,
};
use crate::resource::{ResourceKind, ResourceType};
use crate::star::core::resource::host::{HostCall, HostComponent};
use crate::star::pledge::ResourceHostSelector;
use crate::star::{StarCommand, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use std::convert::TryInto;
use tokio::sync::{mpsc, oneshot};

pub mod message;
pub mod resource;

pub enum CoreCall {
    Message(StarMessage),
}

impl Call for CoreCall{}

pub struct Router {
    skel: StarSkel,
    host_tx: mpsc::Sender<HostCall>,
}

impl Router {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<CoreCall>) {
        let host_tx = HostComponent::new(skel.clone());
        AsyncRunner::new(Box::new(Self { skel:skel.clone(), host_tx }), skel.core_tx.clone(), rx);
    }
}

#[async_trait]
impl AsyncProcessor<CoreCall> for Router {
    async fn process(&mut self, call: CoreCall) {
        match call {
            CoreCall::Message(message) => match self.process_resource_message(message).await {
                Ok(_) => {}
                Err(err) => {
                    error!("{}", err);
                }
            },
        }
    }
}

impl Router {
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
                self.process_registry_message(star_message.clone(), action.clone())
                    .await?
            }
            StarMessagePayload::ResourceHost(action) => {
                self.process_resource_host_action(star_message.clone(), action.clone())
                    .await?
            }

            _ => {}
        };
        Ok(())
    }

    async fn process_resource_request(
        &mut self,
        delivery: Delivery<Message<ResourceRequestMessage>>,
    ) -> Result<(), Error> {
        match delivery.message.payload.clone() {
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
                                delivery
                                    .reply(ResourceResponseMessage::Resource(Option::Some(record)))
                                    .await;
                            }
                            Err(fail) => {
                                eprintln!("Fail: {}", fail.to_string());
                            }
                        },
                        Err(err) => {
                            eprintln!("Error: {}", err);
                        }
                    }
                });
            }
            ResourceRequestMessage::Select(selector) => {
                let resources = self
                    .skel
                    .registry
                    .as_ref()
                    .unwrap()
                    .select(selector.clone())
                    .await?;
                delivery
                    .reply(ResourceResponseMessage::Resources(resources))
                    .await?;
            }
            ResourceRequestMessage::Unique(resource_type) => {
                let unique_src = self
                    .skel
                    .registry
                    .as_ref()
                    .unwrap()
                    .unique_src(delivery.message.to.clone().into())
                    .await;
                delivery
                    .reply(ResourceResponseMessage::Unique(
                        unique_src.next(&resource_type).await?,
                    ))
                    .await?;
            }
            ResourceRequestMessage::State => {
                let key: ResourceKey = delivery.message.to.clone().try_into()?;

                let (tx,rx) = oneshot::channel();
                self.host_tx.send(HostCall::Get{ key, tx }).await?;
                tokio::spawn(async move {
                    let result = rx.await;
                    if let Ok(Ok(Option::Some(state))) = result {
                        let state = match state.try_into() {
                            Ok(state) => state,
                            Err(_) => {
                                error!("error when try_into from BinSrc to NetworkBinSrc");
                                delivery
                                    .reply(ResourceResponseMessage::Fail(Fail::expected(
                                        "Ok(Ok(StarCoreResult::State(state)))",
                                    )))
                                    .await
                                    .unwrap_or_default();
                                return;
                            }
                        };

                        delivery
                            .reply(ResourceResponseMessage::State(state))
                            .await
                            .unwrap_or_default();
                    } else {
                        delivery
                            .reply(ResourceResponseMessage::Fail(Fail::expected(
                                "Ok(Ok(StarCoreResult::State(state)))",
                            )))
                            .await
                            .unwrap_or_default();
                    }
                });
            }
        }
        Ok(())
    }

    async fn process_registry_message(
        &mut self,
        message: StarMessage,
        action: RegistryAction,
    ) -> Result<(), Error> {
        if let Option::Some(manager) = self.skel.registry.clone() {
            match action {
                RegistryAction::Register(registration) => {
                    let result = manager.register(registration.clone()).await;
                    self.skel
                        .comm()
                        .reply_result_empty(message.clone(), result)
                        .await;
                }
                RegistryAction::Location(location) => {
                    let result = manager.set_location(location.clone()).await;
                    self.skel
                        .comm()
                        .reply_result_empty(message.clone(), result)
                        .await;
                }
                RegistryAction::Find(find) => {
                    let result = manager.get(find.to_owned()).await;

                    match result {
                        Ok(result) => match result {
                            Some(record) => {
                                self.skel
                                    .comm()
                                    .reply_result(message.clone(), Ok(Reply::Resource(record)))
                                    .await;
                            }
                            None => {
                                self.skel
                                    .comm()
                                    .reply_result(
                                        message.clone(),
                                        Err(Fail::ResourceNotFound(find)),
                                    )
                                    .await;
                            }
                        },
                        Err(fail) => {
                            self.skel
                                .comm()
                                .reply_result(message.clone(), Err(fail))
                                .await;
                        }
                    }
                }
                RegistryAction::Status(_report) => {
                    unimplemented!()
                }
                RegistryAction::UniqueResourceId { parent, child_type } => {
                    let unique_src = self
                        .skel
                        .registry
                        .as_ref()
                        .unwrap()
                        .unique_src(parent)
                        .await;
                    let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(
                        Reply::Id(unique_src.next(&child_type).await?),
                    )));
                    self.skel
                        .star_tx
                        .send(StarCommand::SendProtoMessage(proto))
                        .await;
                }
            }
        }

        Ok(())
    }

    async fn process_resource_host_action(
        &self,
        message: StarMessage,
        action: ResourceHostAction,
    ) -> Result<(), Error> {
        match action {
/*            ResourceHostAction::IsHosting(resource) => {
                if let Option::Some(resource) = self.get_resource(&resource).await? {
                    let record = resource.into();
                    let record = ResourceRecord::new(record, self.skel.info.key.clone());
                    self.skel
                        .comm()
                        .simple_reply(message, SimpleReply::Ok(Reply::Resource(record)))
                        .await;
                } else {
                    self.skel
                        .comm()
                        .simple_reply(
                            message,
                            SimpleReply::Fail(Fail::ResourceNotFound(resource.into())),
                        )
                        .await;
                }
            }*/
            ResourceHostAction::Assign(assign) => {
                let (tx, rx) = oneshot::channel();
                let key = assign.stub.key.clone();
                let call = HostCall::Assign { assign, tx };
                self.host_tx.send(call).await;
                rx.await??;
                    if let Result::Ok(Option::Some(record)) = self
                        .skel
                        .registry.as_ref()
                        .expect("expected registry")
                        .get(key.into())
                        .await
                    {
                        self.skel
                            .comm()
                            .simple_reply(message, SimpleReply::Ok(Reply::Resource(record)))
                            .await;
                    } else {
                        error!("could not get resource record");
                    }

            }
        }
        Ok(())
    }

    async fn get_parent_resource(&mut self, key: ResourceKey) -> Result<Parent, Fail> {
        let resource = self
            .skel
            .registry
            .as_ref()
            .expect("expected reegistry").get(key.into() ).await?.ok_or("expected parent resource")?;


            Ok(Parent {
                core: ParentCore {
                    stub: resource.into(),
                    selector: ResourceHostSelector::new(self.skel.clone()),
                    child_registry: self.skel.registry.as_ref().unwrap().clone(),
                    skel: self.skel.clone(),
                },
            })
    }

    pub async fn has_resource(&self, key: &ResourceKey) -> Result<bool,Error> {
        let (tx,mut rx) = oneshot::channel();
        self.host_tx.send( HostCall::Has{ key: key.clone(), tx } ).await?;
        Ok(rx.await?)
    }

}
