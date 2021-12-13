use std::convert::TryInto;

use tokio::sync::{mpsc, oneshot};


use crate::error::Error;
use crate::frame::{StarMessagePayload, StarMessage, ResourceRegistryRequest, ResourceHostAction};
use crate::message::delivery::Delivery;
use crate::resource::{Parent, ParentCore, ResourceManager, ResourceRecord, AssignResourceStateSrc };
use crate::resource::{Kind, ResourceType};
use crate::star::{StarCommand, StarKind, StarSkel};
use crate::star::core::resource::host::{HostCall, HostComponent};
use crate::star::shell::pledge::ResourceHostSelector;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::oneshot::error::RecvError;
use std::collections::HashMap;
use crate::mesh::Message;
use crate::mesh::serde::entity::request::{ReqEntity, Rc, Msg, Http};
use crate::mesh::Request;
use crate::mesh::Response;
use crate::resource::selector::ConfigSrc;
use crate::mesh::serde::http::HttpRequest;
use crate::fail::Fail;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::resource::command::RcCommand;
use crate::mesh::serde::pattern::TksPattern;
use crate::mesh::serde::payload::Payload;
use crate::mesh::serde::payload::Primitive;

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
                Message::Request(request ) => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone() );

                    match &request.entity {
                        ReqEntity::Rc(_) => {
                            self.process_resource_command(delivery).await?;
                        }
                        _ => {
                            self.handle_by_host(delivery).await?;
                        }
                    }
                }
                Message::Response(response ) => {
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

    async fn process_resource_command(
        &mut self,
        delivery: Delivery<Request>,
    ) -> Result<(), Error> {
        let skel = self.skel.clone();
        let host_tx = self.host_tx.clone();
        tokio::spawn(async move {
            async fn process(
                skel: StarSkel,
                host_tx: mpsc::Sender<HostCall>,
                delivery: Delivery<Request>,
            ) -> Result<(), Fail> {

                let command = consume_command(delivery.request.entity.as_str() )?;

                match delivery.request.entity.command{
                    RcCommand::Create(create) => {
                        let parent = MessagingEndpointComponent::get_parent_resource(
                            skel.clone(),
                            create.address_template.parent
                        ) .await?;

                        let record = parent.create((*create).clone()).await.await;

                        match record {
                            Ok(record) => match record {
                                Ok(stub) => {
                                    let payload = Payload::Primitive( Primitive::Stub(stub) );
                                    delivery.respond(payload);
                                }
                                Err(fail) => {
                                    delivery.fail(fail.into());
                                }
                            },
                            Err(err) => {
                                eprintln!("Error: {}", err);
                            }
                        }
                    }
                    RcCommand::Select(select) => {
                        match skel
                            .registry
                            .as_ref()
                            .unwrap()
                            .select((*select).clone())
                            .await {
                            Ok(rtn) => {
                                delivery.respond(rtn);
                            }
                            Err(fail) => {
                                delivery.respond(fail.into());
                            }
                        }
                    }
                    RcCommand::Update(_) => {
                        unimplemented!()
                    }
                }

                /*
                match delivery.entity.payload.clone() {
                    ResourceRequestMessage::Create(create) => {

                    }
                    ResourceRequestMessage::Select(selector) => {

                    }
                    ResourceRequestMessage::Unique(resource_type) => {

                    }
                    ResourceRequestMessage::SelectValues(selector) => {

                        let resource = skel
                            .registry
                            .as_ref()
                            .unwrap()
                            .get(delivery.entity.to.clone() )
                            .await?.ok_or("expected resource: ")?;

                        let key: Address = skel.resource_locator_api.as_key(delivery.entity.to.clone()).await?;
                        let (tx, rx) = oneshot::channel();


                        host_tx.send(HostCall::Select { key, selector, tx }).await?;
                        let result = rx.await;
                        if let Ok(Ok(Option::Some(values))) = result {
                            let values = values.with(resource.stub);
                            delivery.reply(Reply::ResourceValues(values));
                        } else {
                            delivery.fail(Fail::expected("Ok(Ok(ResourceValues(values)))"));
                        }
                    }
                    ResourceRequestMessage::UpdateState(state) => {
                        let key: Address = skel.resource_locator_api.as_key(delivery.entity.to.clone()).await?;
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

                }

                 */
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
                host_tx.try_send( HostCall::Port(delivery)).unwrap_or_default();
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
                host_tx.try_send( HostCall::Http(delivery)).unwrap_or_default();
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
            async fn process( skel: StarSkel, delivery: Delivery<ResourceRegistryRequest> ) -> Result<(),Error> {
                if let Option::Some(registry) = skel.registry.clone() {
                    match &delivery.request {
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
                    eprintln!("error when processing registry action: {}", error.to_string() );
                }
            }
        });


        Ok(())
    }

    async fn process_resource_host_action(
        &self,
        delivery: Delivery<ResourceHostAction>,
    ) -> Result<(), Error> {
        match &delivery.request {
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

    async fn get_parent_resource(skel: StarSkel, address: Address) -> Result<Parent, Error> {
        let resource = skel.resource_locator_api.locate(address.clone().into()).await?;

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

pub struct WrappedHttpRequest {
    pub resource: Address,
    pub request: HttpRequest,
}
