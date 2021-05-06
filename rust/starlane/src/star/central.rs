use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot, broadcast};
use crate::app::{AppInfo, ApplicationStatus, AppCreate, AppLocation};
use crate::id::Id;
use crate::label::Labels;
use crate::star::{CentralCommand, ForwardFrame, StarCommand, StarData, StarKey, StarManager, StarManagerCommand, StarNotify};
use crate::user::{AuthToken, AppAccess};
use crate::message::{ProtoMessage, MessageExpect, MessageUpdate, MessageResult, MessageExpectWait};
use crate::keys::{AppKey, SubSpaceKey};
use tokio::sync::mpsc::error::SendError;
use futures::FutureExt;
use tokio::sync::oneshot::Receiver;
use crate::star::StarCommand::AppLifecycleCommand;
use tokio::sync::oneshot::error::RecvError;
use crate::error::Error;
use crate::frame::{StarMessage, Frame, StarMessagePayload, RequestMessage, SpacePayload, ReportMessage, SpaceMessage, AssignMessage, AppAssign, AppCreateRequest, SequenceMessage};
use crate::logger::Logger;

pub struct CentralManager
{
    info: StarData,
    backing: Box<dyn CentralManagerBacking>,
    manager_tx: mpsc::Sender<StarManagerCommand>,
    pub status: CentralStatus,
}

impl CentralManager
{
    pub fn new(info: StarData, manager_tx: mpsc::Sender<StarManagerCommand>) -> CentralManager
    {

        CentralManager
        {
            info: info.clone(),
            backing: Box::new(CentralManagerBackingDefault::new(info)),
            status: CentralStatus::Launching,
            manager_tx: manager_tx
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
                    self.launch_system_app().await;
                }
            }
            CentralInitStatus::LaunchingSystemApp=> {}
            CentralInitStatus::Ready => {}
        }
    }

    async fn launch_system_app(&mut self)
    {
        let token =  self.backing.get_superuser_token().unwrap();
        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(StarKey::central());
        proto.expect = MessageExpect::RetryUntilOk;
        proto.payload = StarMessagePayload::Space(SpaceMessage {
                                                   sub_space:SubSpaceKey::main(),
                                                   token: token,
                                                   payload: SpacePayload::Request(RequestMessage::AppCreate(AppCreateRequest{
                                                   labels: HashMap::new(),
                                                   kind: "system".to_string(),
                                                   data: Arc::new(vec![]) }))});

        let rx = proto.get_ok_result().await;

        self.info.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;

        let manager_tx = self.manager_tx.clone();
        tokio::spawn( async move {
            rx.await;
            manager_tx.send(StarManagerCommand::Init );
        } );

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
        let mut proto = message.reply(StarMessagePayload::Ok);
        let result = self.info.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: String )
    {
        message.reply(StarMessagePayload::Error(error_message.to_string()));
        let result = self.info.star_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await;
        self.unwrap(result);
    }


    async fn launch_app(&mut self, sub_space: SubSpaceKey, create: AppCreate, expect: MessageExpect) -> Result<oneshot::Receiver<AppLocation>, Error>
    {
        let app_id = self.info.sequence.next();
        let app_key = AppKey::new(sub_space.clone(), app_id.index);
        let app = AppInfo::new(app_key.clone(), create.kind.clone());
        let supervisor = self.backing.select_supervisor();
        if let Option::None = supervisor
        {
//            Err("could not find supervisor to host application".to_string().into());
            unimplemented!()
        }
        let supervisor = supervisor.unwrap();
        let mut proto = ProtoMessage::new();
        proto.to = Some(supervisor.clone());
        proto.payload = StarMessagePayload::Space(SpaceMessage {
            sub_space,
            token: self.backing.get_superuser_token()?,
            payload: SpacePayload::Assign(AssignMessage::App(AppAssign {
                app: app,
                data: create.data,
            }))
        });
        proto.expect = expect;

        let mut command_tx = self.info.star_tx.clone();
        let mut reply_tx = proto.tx.subscribe();
        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            loop {
                if let Result::Ok(update) = reply_tx.recv().await
                {
                    match update
                    {
                        MessageUpdate::Ack(_) => {}
                        MessageUpdate::Result(result) => {
                            let app_loc = AppLocation { app: app_key, supervisor };
                            tx.send(app_loc);
                            break;
                        }
                    }
                }
            }
        });

        self.info.star_tx.send(StarCommand::SendProtoMessage(proto)).await;

        Ok(rx)
    }
}


#[async_trait]
impl StarManager for CentralManager
{
    async fn handle(&mut self, command: StarManagerCommand) {
        if let StarManagerCommand::Init = command
        {

        }
        if let StarManagerCommand::Frame(Frame::StarMessage(message)) = command
        {
            let mut message = message;
            match &message.payload
            {

                StarMessagePayload::Sequence( seq_message)=> {
                   match seq_message
                   {
                       SequenceMessage::Request => {
                           let proto = message.reply(StarMessagePayload::Sequence(SequenceMessage::Response(self.info.sequence.next().index)));
                           self.info.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                       }
                       _ => { eprintln!("CentralManager: unexpected message: Sequence message") }
                   }
                }
                StarMessagePayload::Pledge => {
                    self.backing.add_supervisor(message.from.clone());
                    self.reply_ok(message).await;
                }
                StarMessagePayload::Space(tenant_message) => {
                    match &tenant_message.payload
                    {
                        SpacePayload::Request(tenant_payload) => {
                            match tenant_payload {
                                RequestMessage::AppCreate(app_create_request) => {
                                    let create = AppCreate {
                                        kind: app_create_request.kind.clone(),
                                        data: app_create_request.data.clone(),
                                        labels: app_create_request.labels.clone()
                                    };
                                    match self.launch_app(tenant_message.sub_space.clone(), create, MessageExpect::ReplyErrOrTimeout(MessageExpectWait::Med)).await
                                    {
                                        Ok(rx) => {
                                            match rx.await
                                            {
                                                Ok(app_loc) => {
                                                    match self.backing.get_superuser_token()
                                                    {
                                                        Ok(token) => {
                                                            let proto = message.reply(StarMessagePayload::Space(SpaceMessage {
                                                                sub_space: tenant_message.sub_space.clone(),
                                                                token: token,
                                                                payload: SpacePayload::Report(ReportMessage::AppLocation(app_loc))
                                                            }));
                                                            self.info.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                                        }
                                                        Err(error) => {
                                                            self.reply_error(message, error.to_string());
                                                        }
                                                    }
                                                }
                                                Err(error) => {
                                                    self.reply_error(message, error.to_string());
                                                }
                                            }
                                        }
                                        Err(error) => {
                                            self.reply_error(message, error.to_string());
                                        }
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
    fn sequence_next(&mut self)->Id;
    fn add_supervisor(&mut self, star: StarKey );
    fn remove_supervisor(&mut self, star: StarKey );
    fn set_supervisor_for_application(&mut self, app: AppKey, supervisor_star: StarKey );
    fn get_supervisor_for_application(&self, app: &AppKey) -> Option<&StarKey>;
    fn has_supervisor(&self)->bool;
    fn get_init_status(&self) -> CentralInitStatus;
    fn set_init_status(&self, status: CentralInitStatus );
    fn select_supervisor(&mut self )->Option<StarKey>;

    fn get_superuser_token(&mut self) -> Result<AuthToken,Error>;
}


pub struct CentralManagerBackingDefault
{
    info: StarData,
    init_status: CentralInitStatus,
    supervisors: Vec<StarKey>,
    application_to_supervisor: HashMap<AppKey,StarKey>,
    application_name_to_app_id : HashMap<String,AppInfo>,
    application_state: HashMap<AppKey, ApplicationStatus>,
    supervisor_index: usize
}

impl CentralManagerBackingDefault
{
    pub fn new(info: StarData) -> Self
    {
        CentralManagerBackingDefault {
            info: info,
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
    fn sequence_next(&mut self) -> Id {
        self.info.sequence.next()
    }

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

    fn get_superuser_token(&mut self) -> Result<AuthToken, Error> {
        todo!()
    }
}

#[async_trait]
pub trait AppCentral
{
    async fn create( &self, info: AppInfo, data: Arc<Vec<u8>> ) -> Result<Labels,Error>;
}