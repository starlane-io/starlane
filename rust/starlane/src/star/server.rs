use crate::error::Error;
use crate::frame::{Frame, StarMessage, StarMessagePayload, StarPattern, WindAction, SpacePayload, AppMessagePayload, Reply, AppMessage};
use crate::star::{ServerManagerBacking, StarCommand, StarSkel, StarKey, StarKind, StarManager, StarManagerCommand, Wind, ServerCommand};
use crate::message::{ProtoMessage, MessageExpect};
use crate::logger::{Flag, StarFlag, StarLog, StarLogPayload, Log};
use tokio::time::{sleep, Duration};
use crate::core::{StarCoreCommand, StarCoreAppMessage, AppCommandResult};
use crate::app::{AppCommandKind};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::RecvError;

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

        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(supervisor);
        proto.payload = StarMessagePayload::Pledge(self.skel.info.kind.clone());
        proto.expect = MessageExpect::RetryUntilOk;
        let rx = proto.get_ok_result().await;
        self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

        if self.skel.flags.check(Flag::Star(StarFlag::DiagnosePledge))
        {
            self.skel.logger.log( Log::Star( StarLog::new(&self.skel.info, StarLogPayload::PledgeSent )));
            let mut data = self.skel.clone();
            tokio::spawn(async move {
                let payload = rx.await;
                if let Ok(StarMessagePayload::Ok(_)) = payload
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
        match command {
            StarManagerCommand::StarData(_) => {}
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
                                let (tx,rx) = oneshot::channel();

//                                self.skel.core_tx.send(StarCoreCommand::AppMessage(StarCoreAppMessage{ message: app_message.clone(), tx: tx })).await;

                                let star_tx = self.skel.star_tx.clone();
                                tokio::spawn( async move {
                                    let result = rx.await;
                                    match result
                                    {
                                        Ok(result) => {
                                            match result
                                            {
                                                AppCommandResult::Ok => {
                                                    let proto = star_message.reply_ok(Reply::Empty );
                                                    star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                                }
                                                AppCommandResult::Error(err) => {
                                                    let proto = star_message.reply_err(err);
                                                    star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                                }
                                                _ => {
                                                    let proto = star_message.reply_err("unexpected result".to_string());
                                                    star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            let proto = star_message.reply_err(err.to_string() );
                                            star_tx.send(StarCommand::SendProtoMessage(proto)).await;
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
        }
    }
}
