use std::collections::HashMap;
use std::sync::Arc;

use futures::FutureExt;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::Receiver;

use crate::app::{AppCreateController, AppInfo, ApplicationStatus, AppLocation, AppCreateData};
use crate::error::Error;
use crate::frame::{AppAssign, AssignMessage, Frame, ReportMessage, RequestMessage, SequenceMessage, SpaceMessage, SpacePayload, StarMessage, StarMessagePayload, Reply};
use crate::id::Id;
use crate::keys::{AppId, AppKey, SubSpaceKey, UserKey, SpaceKey, UserId};
use crate::label::Labels;
use crate::logger::{Flag, Log, Logger, StarFlag, StarLog, StarLogPayload};
use crate::message::{MessageExpect, MessageExpectWait, MessageResult, MessageUpdate, ProtoMessage};
use crate::star::{CentralCommand, ForwardFrame, StarCommand, StarSkel, StarInfo, StarKey, StarKind, StarManager, StarManagerCommand, StarNotify, PublicKeySource};
use crate::star::StarCommand::SpaceCommand;
use crate::permissions::{AppAccess, AuthToken, User, UserKind};
use crate::crypt::{PublicKey, CryptKeyId};
use crate::frame::Reply::App;

pub struct CentralManager
{
    data: StarSkel,
    backing: Box<dyn CentralManagerBacking>,
    pub status: CentralStatus,
    public_key_source: PublicKeySource
}

impl CentralManager
{
    pub fn new(data: StarSkel) -> CentralManager
    {
        CentralManager
        {
            data: data.clone(),
            backing: Box::new(CentralManagerBackingDefault::new(data)),
            status: CentralStatus::Launching,
            public_key_source: PublicKeySource::new()
        }
    }

    async fn init(&mut self)
    {
        match self.backing.get_init_status()
        {
            CentralInitStatus::None => {
                if self.backing.has_supervisor()
                {
                    self.backing.set_init_status(CentralInitStatus::LaunchingSystemApp);
//                    self.launch_system_app().await;
                }
            }
            CentralInitStatus::LaunchingSystemApp=> {}
            CentralInitStatus::Ready => {}
        }
    }


    pub fn unwrap(&self, result: Result<(), SendError<StarCommand>>)
    {
        match result
        {
            Ok(_) => {}
            Err(error) => {
                eprintln!("could not send starcommand from manager to star: {}", error);
            }
        }
    }

    pub async fn reply_ok(&self, message: StarMessage)
    {
        let mut proto = message.reply(StarMessagePayload::Ok(Reply::Empty));
        let result = self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: String )
    {
        message.reply(StarMessagePayload::Error(error_message.to_string()));
        let result = self.data.star_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await;
        self.unwrap(result);
    }

}


#[async_trait]
impl StarManager for CentralManager
{
    async fn handle(&mut self, command: StarManagerCommand) {
        if let StarManagerCommand::Init = command
        {

        }
        if let StarManagerCommand::StarMessage(message) = command
        {
            let mut message = message;
            match &message.payload
            {

                StarMessagePayload::Pledge(StarKind::Supervisor) => {

                    self.backing.add_supervisor(message.from.clone());
                    self.reply_ok(message).await;
                    if self.data.flags.check( Flag::Star(StarFlag::DiagnosePledge )) {
                        self.data.logger.log( Log::Star(StarLog::new(&self.data.info, StarLogPayload::PledgeRecv )));
                    }
                }
                StarMessagePayload::Space(space_message) => {
                    match &space_message.payload
                    {
                        SpacePayload::Request(space_message_payload) => {
                            match space_message_payload {
                                RequestMessage::AppCreate(create) => {
                                    if let Option::Some(supervisor) = self.backing.select_supervisor()
                                    {
                                        let mut proto = ProtoMessage::new();
                                        let app = AppKey::new( create.sub_space.clone() );
                                        proto.payload = StarMessagePayload::Space( space_message.with_payload(SpacePayload::Assign(AssignMessage::App(AppAssign{app:app.clone(),info:create.clone()}))));
                                        proto.to = Option::Some(supervisor);
                                        let reply = proto.get_ok_result().await;
                                        self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                        match reply.await
                                        {
                                            Ok(StarMessagePayload::Ok(Empty)) => {
                                                let proto = message.reply(StarMessagePayload::Ok(App(app.clone())) );
                                                self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                            }
                                            Err(error) => {
                                                let proto = message.reply(StarMessagePayload::Error(format!("central: receiving error: {}.",error).into()));
                                                self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                            }
                                            _ => {
                                                let proto = message.reply(StarMessagePayload::Error(format!("central: unexpected response").into()));
                                                self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                            }
                                        }
                                    }
                                    else
                                    {
                                        let proto = message.reply(StarMessagePayload::Error("central: no supervisors selected.".into()));
                                        self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                    }
                                }
                                RequestMessage::AppSupervisor(_) => {}
                                RequestMessage::AppLookup(_) => {}
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
                StarMessagePayload::Ok(_)=>{},
                StarMessagePayload::Error(_)=>{},
                unexpected => { eprintln!("CentralManager: unexpected message: {} ", unexpected) }
            }
        }
    }



}


#[derive(Clone)]
pub enum CentralStatus
{
    Launching,
    CreatingSystemApp,
    Ready
}

#[derive(Clone)]
pub enum CentralInitStatus
{
    None,
    LaunchingSystemApp,
    Ready
}

trait CentralManagerBacking: Send+Sync
{
    fn add_supervisor(&mut self, star: StarKey );
    fn remove_supervisor(&mut self, star: StarKey );
    fn set_supervisor_for_application(&mut self, app: AppKey, supervisor_star: StarKey );
    fn get_supervisor_for_application(&self, app: &AppKey) -> Option<&StarKey>;
    fn has_supervisor(&self)->bool;
    fn get_init_status(&self) -> CentralInitStatus;
    fn set_init_status(&self, status: CentralInitStatus );
    fn select_supervisor(&mut self )->Option<StarKey>;

    fn get_public_key_for_star(&self,star:&StarKey) -> Option<PublicKey>;
}


pub struct CentralManagerBackingDefault
{
    data: StarSkel,
    init_status: CentralInitStatus,
    supervisors: Vec<StarKey>,
    application_to_supervisor: HashMap<AppKey,StarKey>,
    application_name_to_app_id : HashMap<String,AppInfo>,
    application_state: HashMap<AppKey, ApplicationStatus>,
    supervisor_index: usize
}

impl CentralManagerBackingDefault
{
    pub fn new(data: StarSkel) -> Self
    {
        CentralManagerBackingDefault {
            data: data,
            init_status: CentralInitStatus::None,
            supervisors: vec![],
            application_to_supervisor: HashMap::new(),
            application_name_to_app_id: HashMap::new(),
            application_state: HashMap::new(),
            supervisor_index: 0
        }
    }
}

impl CentralManagerBacking for CentralManagerBackingDefault
{

    fn add_supervisor(&mut self, star: StarKey) {
        if !self.supervisors.contains(&star)
        {
            self.supervisors.push(star);
        }
    }

    fn remove_supervisor(&mut self, star: StarKey) {
        self.supervisors.retain( |s| *s != star );
    }

    fn set_supervisor_for_application(&mut self, app: AppKey, supervisor_star: StarKey) {
        self.application_to_supervisor.insert( app, supervisor_star );
    }

    fn get_supervisor_for_application(&self, app: &AppKey) -> Option<&StarKey> {
        self.application_to_supervisor.get(app )
    }

    fn has_supervisor(&self) -> bool {
        !self.supervisors.is_empty()
    }

    fn get_init_status(&self) -> CentralInitStatus {
        todo!()
    }

    fn set_init_status(&self, status: CentralInitStatus) {
        todo!()
    }

    fn select_supervisor(&mut self) -> Option<StarKey> {
        if self.supervisors.len() == 0
        {
            return Option::None;
        }
        else {
            self.supervisor_index = &self.supervisor_index + 1;
            return self.supervisors.get(&self.supervisor_index%self.supervisors.len()).cloned();
        }
    }


    fn get_public_key_for_star(&self, star: &StarKey) -> Option<PublicKey> {
        Option::Some( PublicKey{ id: CryptKeyId::default(), data: vec![] })
    }
}

#[async_trait]
pub trait AppCentral
{
    async fn create( &self, info: AppInfo, data: Arc<Vec<u8>> ) -> Result<Labels,Error>;
}