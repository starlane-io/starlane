use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use crate::app::{AppInfo, AppKey, ApplicationStatus, AppCreate};
use crate::error::Error;
use crate::frame::{AppAssign, ApplicationSupervisorReport, Frame, Rejection, StarMessage, StarMessagePayload, StarUnwind, StarUnwindPayload, StarWindPayload, TenantMessage, TenantMessagePayload, RequestMessage, AssignMessage};
use crate::id::Id;
use crate::label::Labels;
use crate::star::{CentralCommand, ForwardFrame, StarCommand, StarInfo, StarKey, StarManager, StarManagerCommand, StarNotify};
use crate::user::{AuthToken, AppAccess};
use crate::message::ProtoMessage;

pub struct CentralManager
{
    info: StarInfo,
    backing: Box<dyn CentralManagerBacking>
}

impl CentralManager
{
    pub fn new(info: StarInfo )->CentralManager
    {
        CentralManager
        {
            info: info.clone(),
            backing: Box::new( CentralManagerBackingDefault::new(info) )
        }
    }

    pub async fn reply_ok(&self, mut message: StarMessage )
    {
        message.reply( self.info.sequence.next(), StarMessagePayload::Ok );
        let result = self.info.command_tx.send( StarCommand::Frame(Frame::StarMessage(message))).await;
        match result
        {
            Ok(_) => {}
            Err(error) => {
                eprintln!("could not send starcommand from manager to star: {}",error.into());
            }
        }
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: &str )
    {
        message.reply( self.info.sequence.next(), StarMessagePayload::Error(error_message.to_string()));
        let result = self.info.command_tx.send( StarCommand::Frame(Frame::StarMessage(message))).await;
        match result
        {
            Ok(_) => {}
            Err(error) => {
                eprintln!("could not send star command from manager to star: {}",error.into());
            }
        }
    }


    async fn launch_app( &mut self, tenant: TenantKey, create: AppCreate, notify: Vec<StarNotify> )->Result<(),Error>
    {
            let app_id = self.info.sequence.next();
            let app_key = AppKey::new(tenant_message.tenant.clone(), app_id.index );
            let app = AppInfo::new(app_key, tenant_payload.kind.clone());
            let supervisor = self.backing.select_supervisor();
            if let Option::None = supervisor
            {
                Err("could not find supervisor to host application".into())
            } else {
                let supervisor = supervisor.unwrap();
                let mut message = ProtoMessage::new();
                message.to = Some(supervisor);
                message.payload = StarMessagePayload::Tenant(TenantMessage {
                    tenant: tenant,
                    token: self.backing.get_superuser_token()?,
                    payload: TenantMessagePayload::Assign(AssignMessage::App(AppAssign{
                        app: app,
                        data: create.data,
                        notify: notify
                    }))
                });
            }
        Ok(())
    }


    #[async_trait]
impl StarManager for CentralManager
{
    async fn handle(&mut self, command: StarManagerCommand) {

        if let StarManagerCommand::Init = command
        {
        }
        else if let StarManagerCommand::Frame(Frame::StarMessage(message)) = command
        {
            let mut message = message;
            match &message.payload
            {
                unexpected => { eprintln!("CentralManager: unexpected message: {} ", unexpected) }

                StarMessagePayload::Pledge=> {
                    self.backing.add_supervisor(message.from.clone());
                    self.reply_ok(message).await;
                }
                StarMessagePayload::Tenant(tenant_message) => {
                    match &tenant_message.payload
                    {
                        TenantMessagePayload::Request(tenant_payload) => {
                            match tenant_payload {
                                RequestMessage::AppCreate(create) =>{
                                    self.launch_app()
                                }
                                RequestMessage::AppSupervisor(_) => {}
                                RequestMessage::AppLookup(_) => {}
                                RequestMessage::AppMessage(_) => {}
                                RequestMessage::AppLabel(_) => {}
                            }
                        }
                        _ => {}
                    }

                    }
                }
                StarMessagePayload::ApplicationNotifyReady(notify) => {
                    self.backing.set_application_state(notify.location.app.clone(), ApplicationStatus::Ready);
                    Ok(())
                    // do nothing
                }
                StarMessagePayload::ApplicationSupervisorRequest(request) => {
                    if let Option::Some(supervisor) = self.backing.select_supervisor()
                    {
                        message.reply(self.info.sequence.next(), StarMessagePayload::ApplicationSupervisorReport(ApplicationSupervisorReport { app: request.app, supervisor: supervisor.clone() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    } else {
                        message.reply(self.info.sequence.next(), StarMessagePayload::Reject(Rejection { message: format!("cannot find app_id: {}", request.app).to_string() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    }
                }
                StarMessagePayload::ApplicationLookup(request) => {
                    let app_id = self.backing.get_application_for_name(&request.name);
                    if let Some(app) = app_id
                    {
                        if let Option::Some(supervisor) = self.backing.get_supervisor_for_application(&app.key) {
                            message.reply(self.info.sequence.next(), StarMessagePayload::ApplicationSupervisorReport(ApplicationSupervisorReport { app: app.key.clone(), supervisor: supervisor.clone() }));
                            self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                            Ok(())
                        } else {
                            self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                            Ok(())
                        }
                    } else {
                        message.reply(self.info.sequence.next(), StarMessagePayload::Reject(Rejection { message: format!("could not find app_id for lookup name: {}", request.name).to_string() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    }
                    // return this if both conditions fail
                }
                whatever => {
                    Err(format!("unimplemented Central {}",whatever).into())
                }
            }
        }
        else if let StarManagerCommand::Frame(Frame::StarWind(wind)) = &command {
            match wind.payload
            {
                StarWindPayload::RequestSequence => {
                    let payload = StarUnwindPayload::AssignSequence(self.backing.sequence_next().index);
                    let inner = StarUnwind {
                        stars: wind.stars.clone(),
                        payload: payload
                    };

                    self.info.command_tx.send( StarCommand::ForwardFrame(ForwardFrame{ to: inner.stars.last().cloned().unwrap(), frame: Frame::StarUnwind(inner)})).await;

                    Ok(())
                }
            }
        }
        else {
            Err(format!("{} cannot handle command {}",self.info.kind,command).into() )
        }
    }

}

trait CentralManagerBacking: Send+Sync
{
    fn sequence_next(&mut self)->Id;
    fn add_supervisor(&mut self, star: StarKey );
    fn remove_supervisor(&mut self, star: StarKey );
    fn set_supervisor_for_application(&mut self, app: AppKey, supervisor_star: StarKey );
    fn get_supervisor_for_application(&self, app: &AppKey) -> Option<&StarKey>;
    fn select_supervisor(&mut self )->Option<StarKey>;

    fn get_superuser_token(&mut self) -> Result<AuthToken,Error>;
}


pub struct CentralManagerBackingDefault
{
    info: StarInfo,
    supervisors: Vec<StarKey>,
    application_to_supervisor: HashMap<AppKey,StarKey>,
    application_name_to_app_id : HashMap<String,AppInfo>,
    application_state: HashMap<AppKey, ApplicationStatus>,
    supervisor_index: usize
}

impl CentralManagerBackingDefault
{
    pub fn new( info: StarInfo ) -> Self
    {
        CentralManagerBackingDefault {
            info: info,
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
}
pub trait AppCentral
{
    async fn create( &self, info: AppInfo, data: Arc<Vec<u8>> ) -> Result<Labels,Error>;
}