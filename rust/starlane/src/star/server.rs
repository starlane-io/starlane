use crate::error::Error;
use crate::frame::{Frame, StarMessage, StarMessagePayload, StarPattern, WindAction};
use crate::star::{ServerManagerBacking, StarCommand, StarData, StarKey, StarKind, StarManager, StarManagerCommand, Wind, ServerCommand};
use crate::message::{ProtoMessage, MessageExpect};
use crate::logger::{Flag, StarFlag, StarLog, StarLogPayload, Log};
use tokio::time::{sleep, Duration};

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
    data: StarData,
    backing: Box<dyn ServerManagerBacking>,
}

impl ServerManager
{
    pub fn new(data: StarData) -> Self
    {
        ServerManager
        {
            data: data,
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
println!("Server: Pledging");
        let supervisor = match self.get_supervisor(){
            None => {
                loop
                {
                    let (search, rx) = Wind::new(StarPattern::StarKind(StarKind::Supervisor), WindAction::SearchHits);
                    self.data.star_tx.send(StarCommand::WindInit(search)).await;
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
        proto.payload = StarMessagePayload::Pledge(self.data.info.kind.clone());
        proto.expect = MessageExpect::RetryUntilOk;
        let rx = proto.get_ok_result().await;
        self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

        if self.data.flags.check(Flag::Star(StarFlag::DiagnosePledge))
        {
println!("Server: PledgeSent");
            self.data.logger.log( Log::Star( StarLog::new( &self.data.info, StarLogPayload::PledgeSent )));
            let mut data = self.data.clone();
            tokio::spawn(async move {
                let payload = rx.await;
                if let Ok(StarMessagePayload::Ok) = payload
                {

println!("Server: PledgeOkRecv");
                    data.logger.log( Log::Star( StarLog::new( &data.info, StarLogPayload::PledgeOkRecv )))
                }
            });
        }

        Ok(())
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
            StarManagerCommand::StarMessage(_) => {}
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
