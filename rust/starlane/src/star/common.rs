use crate::star::{StarSkel, RegistryBacking, StarVariant, StarVariantCommand, Request, StarCommand};
use std::sync::Arc;
use crate::star::pledge::StarHandleBacking;
use crate::frame::{ResourceManagerAction, StarMessage, StarMessagePayload, SimpleReply, Reply, ResourceHostAction};
use crate::core::StarCoreCommand;
use crate::resource::ResourceLocation;

pub struct CommonVariant
{
    pub skel: StarSkel,
    registry: Arc<dyn RegistryBacking>,
    star_handles: Arc<StarHandleBacking>
}

#[async_trait]
impl StarVariant for CommonVariant {
    async fn handle(&mut self, command: StarVariantCommand) {
        match command{
            StarVariantCommand::StarMessage(star_message ) => {
                match star_message.payload{
                    StarMessagePayload::ResourceHost(action ) => {
/*
                       match action {
                           ResourceHostAction::HasResource(resource) => {
                               let (request,rx) = Request::new(resource);
                               self.skel.core_tx.send( StarCoreCommand::HasResource(request)).await;
                               let comm = self.skel.comm();
                               tokio::spawn( async move {
                                   if let Result::Ok(Result::Ok(local)) = rx.await {
                                       let location = ResourceLocation{
                                           key: local.resource,
                                           host: self.skel.info.star.clone(),
                                           gathering: local.gathering
                                       };
                                       let proto = star_message.reply( StarMessagePayload::Reply(SimpleReply::Ok(Reply::Location(location))));
                                       comm.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
                                   }
                               } );
                           }
                           ResourceHostAction::ResourceAssign(resource) => {}
                       }*/
                    }
                    StarMessagePayload::ResourceManager(action) => {
                        unimplemented!()
                        /*
                        match action
                        {
                            ResourceManagerAction::Register(registration) => {
                                let result = self.registry.register(registration.clone()).await;
                                self.skel.comm().reply_result_empty(star_message.clone(), result ).await;
                            }
                            ResourceManagerAction::Location(location) => {
                                let result = self.registry.set_location(location.clone()).await;
                                self.skel.comm().reply_result_empty(star_message.clone(), result ).await;
                            }
                            ResourceManagerAction::Find(find) => {
                                let result = self.registry.find(find.to_owned()).await;
                                self.skel.comm().reply_result(star_message.clone(), result ).await;
                            }

                            ResourceManagerAction::GetKey(address) => {
                                let result = self.registry.get_key(address.clone()).await;
                                self.skel.comm().reply_result(star_message.clone(), result ).await;
                            }
                            ResourceManagerAction::Bind(bind) => {
                                let result = self.registry.bind(bind.clone()).await;
                                self.skel.comm().reply_result_empty(star_message.clone(), result ).await;
                            }
                        }*/
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}