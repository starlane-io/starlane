use crate::error::Error;
use crate::frame::{Frame, StarMessage, StarMessagePayload, StarPattern, WindAction, SpacePayload, ServerAppPayload, Reply, AppMessage, SpaceMessage, ServerPayload, StarMessageCentral, StarMessageReply};
use crate::star::{ServerManagerBacking, StarCommand, StarSkel, StarKey, StarKind, StarManager, StarManagerCommand, Wind, ServerCommand, CoreRequest};
use crate::message::{ProtoMessage, MessageExpect};
use crate::logger::{Flag, StarFlag, StarLog, StarLogPayload, Log};
use tokio::time::{sleep, Duration};
use crate::core::{StarCoreCommand, StarCoreAppMessage, AppCommandResult, StarCoreAppMessagePayload, StarCoreAppLaunch, StarCoreAppAssign};
use crate::app::{AppCommandKind};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;
use crate::core::server::AppExtError;
use crate::keys::{AppKey, UserKey};

pub struct ServerManagerBackingDefault
{
    pub supervisor: Option<StarKey>
}

impl ServerManagerBackingDefault
{
   pub fn new()-> Self
   {
       ServerManagerBackingDefault{
           supervisor: Option::None
       }
   }
}

impl ServerManagerBacking for ServerManagerBackingDefault
{
    fn set_supervisor(&mut self, supervisor_star: StarKey) {
        self.supervisor = Option::Some(supervisor_star);
    }

    fn get_supervisor(&self) -> Option<&StarKey> {
        self.supervisor.as_ref()
    }
}


pub struct ServerManager
{
    skel: StarSkel,
    backing: Box<dyn ServerManagerBacking>,
}

impl ServerManager
{
    pub fn new(data: StarSkel) -> Self
    {
        ServerManager
        {
            skel: data,
            backing: Box::new(ServerManagerBackingDefault::new())
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

    async fn pledge(&mut self)->Result<(),Error>
    {
        let supervisor = match self.get_supervisor(){
            None => {
                loop
                {
                    let (search, rx) = Wind::new(StarPattern::StarKind(StarKind::Supervisor), WindAction::SearchHits);
                    self.skel.star_tx.send(StarCommand::WindInit(search)).await;
                    if let Ok(hits) = rx.await
                    {
                        break hits.nearest().unwrap().star
                    }
println!("Server: Could not find Supervisor... waiting 5 seconds to try again...");
                    tokio::time::sleep( Duration::from_secs(5) ).await;
                }
            }
            Some(supervisor) => supervisor.clone()
        };

        self.set_supervisor(supervisor.clone());
        self.skel.core_tx.send( StarCoreCommand::SetSupervisor(supervisor.clone() )).await;

        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(supervisor);
        proto.payload = StarMessagePayload::Central(StarMessageCentral::Pledge(self.skel.info.kind.clone()));
        proto.expect = MessageExpect::RetryUntilOk;
        let rx = proto.get_ok_result().await;
        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

        if self.skel.flags.check(Flag::Star(StarFlag::DiagnosePledge))
        {
            self.skel.logger.log( Log::Star( StarLog::new(&self.skel.info, StarLogPayload::PledgeSent )));
            let mut data = self.skel.clone();
            tokio::spawn(async move {
                let payload = rx.await;
                if let Ok(StarMessagePayload::Reply(StarMessageReply::Ok(_))) = payload
                {
                    data.logger.log( Log::Star( StarLog::new( &data.info, StarLogPayload::PledgeOkRecv )))
                }
            });
        }

        Ok(())
    }


}

impl ServerManager
{
    async fn send_proto( &self, proto: ProtoMessage )
    {
        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
    }
}

#[async_trait]
impl StarManager for ServerManager
{
    async fn handle(&mut self, command: StarManagerCommand) {
       match command
       {
           StarManagerCommand::Init => {
               self.pledge().await;
           }
           StarManagerCommand::StarMessage(star_message) => {
               match star_message.payload{
                   StarMessagePayload::Space(space_message) => {
                       match space_message.payload
                       {
                           SpacePayload::Server(server_space_message) => {
                               match server_space_message
                               {
                                   ServerPayload::AppAssign(meta) => {

                                   }
                                   ServerPayload::SequenceResponse(_) => {}
                                   ServerPayload::AppLaunch(launch) => {}
                               }
                           }
                           _ => {}
                       }
                   }
                   _ => {}
               }
           }
           StarManagerCommand::ServerCommand(command) => {
               match command
               {
                   ServerCommand::PledgeToSupervisor => {}
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
