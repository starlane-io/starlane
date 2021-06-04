use crate::error::Error;
use crate::frame::{Frame, StarMessage, StarMessagePayload, StarPattern, WindAction, SpacePayload, ServerAppPayload, Reply, SpaceMessage, ServerPayload, SimpleReply, ChildResourceAction};
use crate::star::{ServerVariantBacking, StarCommand, StarSkel, StarKey, StarKind, StarVariant, StarVariantCommand, Wind, ServerCommand, CoreRequest, Request };
use crate::message::{ProtoMessage, MessageExpect, Fail};
use crate::logger::{Flag, StarFlag, StarLog, StarLogPayload, Log};
use tokio::time::{sleep, Duration};
use crate::core::{StarCoreCommand, StarCoreAppCommand, AppCommandResult, StarCoreAppCommandPayload};
use crate::app::{AppCommandKind};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use crate::keys::{AppKey, UserKey, ResourceKey, GatheringKey};
use crate::resource::ResourceLocationRecord;
use crate::actor::ActorKey;


pub struct ServerVariantBackingDefault
{
    pub supervisor: Option<StarKey>
}

impl ServerVariantBackingDefault
{
   pub fn new()-> Self
   {
       ServerVariantBackingDefault {
           supervisor: Option::None
       }
   }
}

impl ServerVariantBacking for ServerVariantBackingDefault
{
    fn set_supervisor(&mut self, supervisor_star: StarKey) {
        self.supervisor = Option::Some(supervisor_star);
    }

    fn get_supervisor(&self) -> Option<&StarKey> {
        self.supervisor.as_ref()
    }
}


pub struct ServerStarVariant
{
    skel: StarSkel,
    backing: Box<dyn ServerVariantBacking>,
}

impl ServerStarVariant
{
    pub fn new(data: StarSkel) -> Self
    {
        ServerStarVariant
        {
            skel: data,
            backing: Box::new(ServerVariantBackingDefault::new())
        }
    }

    pub fn set_supervisor( &mut self, supervisor_star: StarKey )
    {
        self.backing.set_supervisor(supervisor_star);
    }

    pub fn get_supervisor( &self )->Option<&StarKey>
    {
        self.backing.get_supervisor()
    }



}

impl ServerStarVariant
{
    async fn send_proto( &self, proto: ProtoMessage )
    {
        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
    }
}

#[async_trait]
impl StarVariant for ServerStarVariant
{
    async fn handle(&mut self, command: StarVariantCommand) {
       match command
       {
           StarVariantCommand::Init => {
           }
           StarVariantCommand::StarMessage(star_message) => {
               match &star_message.payload{
                   StarMessagePayload::Space(space_message) => {
                       match &space_message.payload
                       {
                           SpacePayload::Server(server_space_message) => {
                               match server_space_message
                               {
                                   ServerPayload::SequenceResponse(_) => {}
                               }
                           }
                           _ => {}
                       }
                   }
                   StarMessagePayload::ResourceManager(resource) => {
                       unimplemented!()
                       /*
                       match resource
                       {
                           ResourceManagerAction::HasResource(resource) => {
                               let (request,rx) = Request::new(resource.clone() );
                               self.skel.core_tx.send( StarCoreCommand::HasResource(request)).await;
                               let skel = self.skel.clone();
                               let star_message = star_message.clone();
                               let resource = resource .clone();
                               tokio::spawn( async move {
                                   if let Result::Ok(Result::Ok(local)) = rx.await {
                                       let location = ResourceLocation{
                                           key: local.resource,
                                           host: skel.info.star.clone(),
                                           gathering: local.gathering
                                       };
                                       skel.comm().simple_reply( star_message, SimpleReply::Ok(Reply::Location(location))).await;
                                   } else {
                                       skel.comm().simple_reply( star_message, SimpleReply::Fail(Fail::ResourceNotFound(resource.clone()))).await;
                                   }
                               } );
                           }
                           _ => {}
                       }

                        */

                   }
                   _ => {}
               }
           }

           _ => {}
       }
    }

        /*
    async fn handle(&mut self, command: StarManagerCommand) {
        match command {
            StarManagerCommand::StarSkel(_) => {}
            StarManagerCommand::Init => {
                self.pledge().await;
            }
            StarManagerCommand::StarMessage(star_message) => {
                match &star_message.payload
                {
                    StarMessagePayload::Space(space_message) => {
                        match &space_message.payload
                        {
                            SpacePayload::App(app_message) =>
                            {
                                match &app_message.payload
                                {
                                    ServerAppPayload::None => {
                                        // do nothing
                                    }
                                    ServerAppPayload::Assign(info) => {
                                        let (tx,rx) = oneshot::channel();
                                        let payload = StarCoreAppMessagePayload::Assign(StarCoreAppAssign {
                                            assign: info.clone(),
                                            tx: tx
                                        }) ;
                                        let message = StarCoreAppMessage{ app: app_message.app.clone(), payload: payload };
                                        self.skel.core_tx.send( StarCoreCommand::AppMessage(message)).await;
                                        let star_tx = self.skel.star_tx.clone();
                                        tokio::spawn( async move {
                                            let result = rx.await;

                                            match result
                                            {
                                                Ok(result) => {
                                                    match result
                                                    {
                                                        Ok(_) => {
                                                            let proto = star_message.reply( StarMessagePayload::Ok(Reply::Empty) );
                                                            star_tx.send(StarCommand::SendProtoMessage(proto) ).await;
                                                        }
                                                        Err(error) => {
                                                            let proto = star_message.reply( StarMessagePayload::Error("App Host Assign Error.".into()) );
                                                            star_tx.send(StarCommand::SendProtoMessage(proto) ).await;
                                                        }
                                                    }
                                                }
                                                Err(error) => {
                                                    let proto = star_message.reply( StarMessagePayload::Error(error.to_string()) );
                                                    star_tx.send(StarCommand::SendProtoMessage(proto) ).await;
                                                }
                                            }
                                        } );
                                    }
                                    ServerAppPayload::Launch(launch) => {
println!("AppMessagePayload::Create...");
                                       let (tx,rx) = oneshot::channel();
                                       let payload = StarCoreAppMessagePayload::Launch(StarCoreAppLaunch{
                                           launch: launch.clone(),
                                           tx: tx
                                       }) ;
                                       let message = StarCoreAppMessage{ app: app_message.app.clone(), payload: payload };
                                       self.skel.core_tx.send( StarCoreCommand::AppMessage(message)).await;
                                       let star_tx = self.skel.star_tx.clone();
                                       tokio::spawn( async move {
                                           let result = rx.await;

                                           match result
                                           {
                                               Ok(result) => {
                                                   match result
                                                   {
                                                       Ok(_) => {
                                                           let proto = star_message.reply( StarMessagePayload::Ok(Reply::Empty) );
                                                           star_tx.send(StarCommand::SendProtoMessage(proto) ).await;
                                                       }
                                                       Err(error) => {
                                                           let proto = star_message.reply( StarMessagePayload::Error("App Launch Error.".into()) );
                                                           star_tx.send(StarCommand::SendProtoMessage(proto) ).await;
                                                       }
                                                   }
                                               }
                                               Err(error) => {
                                                   let proto = star_message.reply( StarMessagePayload::Error(error.to_string()) );
                                                   star_tx.send(StarCommand::SendProtoMessage(proto) ).await;
                                               }
                                           }
                                       } );
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}

                        }
                    }
                    _ => {}
                }
            }
            StarManagerCommand::CentralCommand(_) => {}
            StarManagerCommand::SupervisorCommand(_) => {}
            StarManagerCommand::ServerCommand(command) => {
                match command
                {
                    ServerCommand::PledgeToSupervisor => {
                        self.pledge().await;
                    }
                }
            }
            StarManagerCommand::CoreRequest(request) => {
                match request
                {
                    CoreRequest::AppSequenceRequest(request) => {
                        if let Option::Some(supervisor) = self.get_supervisor()
                        {
                            let app = request.app.clone();
                            let mut proto = ProtoMessage::new();
                            proto.to = Option::Some(supervisor.clone());
                            proto.payload = StarMessagePayload::Space(SpaceMessage{ sub_space: app.sub_space.clone(), user: request.user.clone(), payload:SpacePayload::Request(RequestMessage::AppSequenceRequest(app))});
                            let ok_result = proto.get_ok_result().await;
                            tokio::spawn( async move {
                                // need to timeout here just in case
                                if let Result::Ok(result) = tokio::time::timeout(Duration::from_secs(30), ok_result).await {
                                match result
                                {
                                    Ok(payload) => {
                                        match payload{
                                            StarMessagePayload::Ok(reply) => {
                                                if let Reply::Seq(seq) = reply
                                                {
                                                    request.tx.send(seq);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    Err(_) => {}
                                }}
                            } );
                            self.skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
                        }
                    }
                }
            }
        }
    }

         */
}
