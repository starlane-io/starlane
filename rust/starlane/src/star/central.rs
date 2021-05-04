use std::collections::HashMap;
use std::sync::Arc;

use crate::app::{AppInfo, AppKey, ApplicationStatus};
use crate::error::Error;
use crate::frame::{ApplicationAssign, ApplicationSupervisorReport, Frame, Rejection, StarMessage, StarMessagePayload, StarUnwind, StarUnwindPayload, StarWindPayload};
use crate::id::Id;
use crate::label::Labels;
use crate::star::{CentralCommand, ForwardFrame, StarCommand, StarInfo, StarKey, StarManager, StarManagerCommand};

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
}

#[async_trait]
impl StarManager for CentralManager
{
    async fn handle(&mut self, command: StarManagerCommand) -> Result<(), Error> {

        if let StarManagerCommand::Init = command
        {
            Ok(())
        }
        else if let StarManagerCommand::Frame(Frame::StarMessage(message)) = command
        {
            let mut message = message;
            match &message.payload
            {
                StarMessagePayload::SupervisorPledgeToCentral => {
                    self.backing.add_supervisor(message.from.clone());
                    Ok(())
                }
                StarMessagePayload::ApplicationCreateRequest(request) => {
                    let app_key = self.info.sequence.next();
                    let app = AppInfo::new(app_key, request.kind.clone());
                    let supervisor = self.backing.select_supervisor();
                    if let Option::None = supervisor
                    {
                        message.reply(self.info.sequence.next(), StarMessagePayload::Reject(Rejection { message: "no supervisors available to host application.".to_string() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    } else {
                        if let Some(name) = &request.name
                        {
                            self.backing.set_name_to_application(name.clone(), app.clone());
                        }
                        let supervisor = supervisor.unwrap();
                        let message = StarMessage {
                            id: self.info.sequence.next(),
                            from: self.info.star_key.clone(),
                            to: supervisor.clone(),
                            transaction: message.transaction.clone(),
                            payload: StarMessagePayload::ApplicationAssign(ApplicationAssign {
                                app: app,
                                data: request.data.clone(),
                                notify: vec![message.from, self.info.star_key.clone()],
                                supervisor: supervisor.clone()
                            }),
                            retry: 0,
                            max_retries: 16
                        };
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
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
    fn set_name_to_application(&mut self, name: String, app: AppInfo);
    fn set_application_state(&mut self,  app: AppKey, state: ApplicationStatus);
    fn get_application_state(&self,  app: &AppKey ) -> Option<&ApplicationStatus>;
    fn get_application_for_name(&self,  name: &String ) -> Option<&AppInfo>;
    fn select_supervisor(&mut self )->Option<StarKey>;
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

    fn set_name_to_application(&mut self, name: String, app: AppInfo) {
        self.application_name_to_app_id.insert(name, app);
    }

    fn set_application_state(&mut self, app: AppKey, state: ApplicationStatus) {
        self.application_state.insert( app, state );
    }

    fn get_application_state(&self, app: &AppKey )->Option<&ApplicationStatus> {
        self.application_state.get( app)
    }

    fn get_application_for_name(&self, name: &String) -> Option<&AppInfo> {
        self.application_name_to_app_id.get(name)
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