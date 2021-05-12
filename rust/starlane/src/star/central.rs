use std::collections::HashMap;
use std::sync::Arc;

use futures::FutureExt;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::Receiver;

use crate::app::{AppCreateController, AppMeta, ApplicationStatus, AppLocation, AppArchetype};
use crate::error::Error;
use crate::frame::{AssignMessage, Frame, SpaceReply, SequenceMessage, SpaceMessage, SpacePayload, StarMessage, StarMessagePayload, Reply, CentralPayload, StarMessageCentral, ServerPayload, SimpleReply, SupervisorPayload};
use crate::id::Id;
use crate::keys::{AppId, AppKey, SubSpaceKey, UserKey, SpaceKey, UserId};
use crate::label::{Labels, LabelDb, LabelRequest};
use crate::logger::{Flag, Log, Logger, StarFlag, StarLog, StarLogPayload};
use crate::message::{MessageExpect, MessageExpectWait, MessageResult, MessageUpdate, ProtoMessage};
use crate::star::{CentralCommand, ForwardFrame, StarCommand, StarSkel, StarInfo, StarKey, StarKind, StarVariant, StarVariantCommand, StarNotify, PublicKeySource};
use crate::star::StarCommand::SpaceCommand;
use crate::permissions::{AppAccess, AuthToken, User, UserKind};
use crate::crypt::{PublicKey, CryptKeyId};
use crate::frame::Reply::App;
use crate::frame::CentralPayload::AppCreate;
use rusqlite::Connection;
use bincode::ErrorKind;

pub struct CentralStarVariant
{
    data: StarSkel,
    backing: Box<dyn CentralStarVariantBacking>,
    pub status: CentralStatus,
    public_key_source: PublicKeySource
}

impl CentralStarVariant
{
    pub async fn new(data: StarSkel) -> CentralStarVariant
    {
        CentralStarVariant
        {
            data: data.clone(),
            backing: Box::new(CentralStarVariantBackingSqlLite::new().await ),
            status: CentralStatus::Launching,
            public_key_source: PublicKeySource::new()
        }
    }

    async fn init(&mut self)
    {
        /*
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

         */
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
        let mut proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
        let result = self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: String )
    {
        let mut proto = message.reply(StarMessagePayload::Reply(SimpleReply::Error(error_message)));
        let result = self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

}


#[async_trait]
impl StarVariant for CentralStarVariant
{
    async fn handle(&mut self, command: StarVariantCommand)
    {
        match &command
        {
            StarVariantCommand::Init => {}
            StarVariantCommand::StarMessage(star_message) => {
               match &star_message.payload
               {
                   StarMessagePayload::Central(central_message) => {
                       match central_message
                       {
                           StarMessageCentral::Pledge(kind) => {
                               if kind.is_supervisor()
                               {
                                   self.backing.add_supervisor(star_message.from.clone()).await;
                                   self.reply_ok(star_message.clone()).await;
                                   if self.data.flags.check(Flag::Star(StarFlag::DiagnosePledge)) {
                                       self.data.logger.log(Log::Star(StarLog::new(&self.data.info, StarLogPayload::PledgeRecv)));
                                   }
                               }
                               else
                               {
                                   self.reply_error(star_message.clone(),format!("expected Supervisor kind got {}",kind)).await;
                               }
                           }
                       }
                   }
                   StarMessagePayload::Space(space_message) => {
                       match &space_message.payload {
                           SpacePayload::Central(central_payload) => {
                               match central_payload
                               {
                                   CentralPayload::AppCreate(archetype) => {
                                       if let Option::Some(supervisor) = self.backing.select_supervisor().await
                                       {
                                           let mut proto = ProtoMessage::new();
                                           let app = AppKey::new(space_message.sub_space.clone());
                                           proto.payload = StarMessagePayload::Space(space_message.with_payload(SpacePayload::Supervisor(SupervisorPayload::AppCreate(archetype.clone()))));
                                           proto.to(supervisor.clone());
                                           let reply = proto.get_ok_result().await;
                                           self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                           match reply.await
                                           {
                                               Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty))) => {
                                                   self.backing.set_supervisor_for_application(app.clone(),supervisor.clone()).await;
                                                   let proto = star_message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::App(app))));
                                                   self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                               }
                                               Err(error) => {
                                                   let proto = star_message.reply(StarMessagePayload::Reply(SimpleReply::Error(format!("central: receiving error: {}.", error.to_string()).into())));
                                                   self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                               }
                                               _ => {
                                                   let proto = star_message.reply(StarMessagePayload::Reply(SimpleReply::Error("central: unexpected response".into())));
                                                   self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                               }
                                           }
                                       } else {
                                           let proto = star_message.reply(StarMessagePayload::Reply( SimpleReply:: Error("central: no supervisors selected.".into())));
                                           self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                       }
                                   }
                                   CentralPayload::AppSupervisorLocationRequest(_) => {}
                               }
                           }
                           _ => {}
                       }
                   }
                   _ => {}
               }
            }
            StarVariantCommand::CentralCommand(_) => {}
            _ => {}
        }
    }

    /*async fn handle(&mut self, command: StarManagerCommand) {
        if let StarManagerCommand::Init = command
        {

        }
        if let StarManagerCommand::StarMessage(message) = command
        {
            let mut message = message;
            match &message.payload
            {
                StarMessagePayload::Space(space_message) => {
                    match &space_message.payload
                    {
                        SpacePayload::Central(central_payload) => {
                            match central_payload {
                                CentralPayload::AppCreate(archetype) => {
                                    if let Option::Some(supervisor) = self.backing.select_supervisor()
                                    {
                                        let mut proto = ProtoMessage::new();
                                        let app = AppKey::new(create.sub_space.clone());
                                        let assign = AppMeta::new(app, archetype.kind.clone(), archetype.config.clone(), archetype.owner.clone() );
                                        proto.payload = StarMessagePayload::Space(space_message.with_payload(SpacePayload::Server(ServerPayload::AppAssign(assign))));
                                        proto.to(supervisor);
                                        let reply = proto.get_ok_result().await;
                                        self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                        match reply.await
                                        {
                                            Ok(StarMessagePayload::Ok(Empty)) => {
                                                let proto = message.reply(StarMessagePayload::Ok(App(app.clone())));
                                                self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                            }
                                            Err(error) => {
                                                let proto = message.reply(StarMessagePayload::Error(format!("central: receiving error: {}.", error).into()));
                                                self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                            }
                                            _ => {
                                                let proto = message.reply(StarMessagePayload::Error(format!("central: unexpected response").into()));
                                                self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                            }
                                        }
                                    } else {
                                        let proto = message.reply(StarMessagePayload::Error("central: no supervisors selected.".into()));
                                        self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                    }
                                }
                                CentralPayload::AppSupervisorLocationRequest(_) => {}
                            }
                        }
                        _ => {}
                    }
                }
                StarMessagePayload::Central(central) => {
                    match central {
                        StarMessageCentral::Pledge(supervisor) => {
                            self.backing.add_supervisor(message.from.clone());
                            self.reply_ok(message).await;
                            if self.data.flags.check(Flag::Star(StarFlag::DiagnosePledge)) {
                                self.data.logger.log(Log::Star(StarLog::new(&self.data.info, StarLogPayload::PledgeRecv)));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }*/

}
/*
StarMessagePayload::Pledge(StarKind::Supervisor) => {


}
}

 */

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

#[async_trait]
trait CentralStarVariantBacking: Send+Sync
{
    async fn add_supervisor(&mut self, star: StarKey )->Result<(),Error>;
    async fn remove_supervisor(&mut self, star: StarKey )->Result<(),Error>;
    async fn set_supervisor_for_application(&mut self, app: AppKey, supervisor_star: StarKey )->Result<(),Error>;
    async fn get_supervisor_for_application(&self, app: &AppKey) -> Option<StarKey>;
    async fn has_supervisor(&self)->bool;
    async fn select_supervisor(&mut self )->Option<StarKey>;
}

/*
pub struct CentralStarVariantBackingDefault
{
    data: StarSkel,
    init_status: CentralInitStatus,
    supervisors: Vec<StarKey>,
    application_to_supervisor: HashMap<AppKey,StarKey>,
    application_name_to_app_id : HashMap<String, AppMeta>,
    application_state: HashMap<AppKey, ApplicationStatus>,
    supervisor_index: usize
}

impl CentralStarVariantBackingDefault
{
    pub fn new(data: StarSkel) -> Self
    {
        CentralStarVariantBackingDefault {
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

#[async_trait]
impl CentralStarVariantBacking for CentralStarVariantBackingDefault
{

    async fn add_supervisor(&mut self, star: StarKey) {
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

 */


struct CentralStarVariantBackingSqlLite
{
    label_db: mpsc::Sender<LabelRequest>,
    central_db: mpsc::Sender<CentralDbRequest>
}

impl CentralStarVariantBackingSqlLite
{
    pub async fn new()->Self
    {
        CentralStarVariantBackingSqlLite{
            label_db: LabelDb::new().await,
            central_db: CentralDb::new().await
        }
    }

    pub fn handle( &self, result: Result<Result<CentralDbResult,Error>,RecvError>)->Result<(),Error>
    {
        match result
        {
            Ok(ok) => {
                match ok{
                    Ok(_) => {
                        Ok(())
                    }
                    Err(error) => {
                        Err(error)
                    }
                }
            }
            Err(error) => {
                Err(error.into())
            }
        }
    }
}

#[async_trait]
impl CentralStarVariantBacking for CentralStarVariantBackingSqlLite
{
    async fn add_supervisor(&mut self, star: StarKey) -> Result<(), Error> {
        let (request,rx) = CentralDbRequest::new( CentralDbCommand::AddSupervisor(star));
        self.central_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn remove_supervisor(&mut self, star: StarKey) -> Result<(), Error> {
        let (request,rx) = CentralDbRequest::new( CentralDbCommand::RemoveSupervisor(star));
        self.central_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn set_supervisor_for_application(&mut self, app: AppKey, supervisor_star: StarKey) -> Result<(), Error> {
        let (request,rx) = CentralDbRequest::new( CentralDbCommand::SetSupervisorForApplication((supervisor_star,app)));
        self.central_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_supervisor_for_application(&self, app: &AppKey) -> Option<StarKey> {
        let (request,rx) = CentralDbRequest::new( CentralDbCommand::GetSupervisorForApplication(app.clone()));
        self.central_db.send( request ).await;
        match rx.await
        {
            Ok(ok) => {
                match ok
                {
                    Ok(ok) => {
                        match ok
                        {
                            CentralDbResult::Supervisor(supervisor) => {supervisor}
                            _ => Option::None
                        }
                    }
                    Err(_) => {
                        Option::None
                    }
                }
            }
            Err(error) => {
                Option::None
            }
        }
    }

    async fn has_supervisor(&self) -> bool {
        let (request,rx) = CentralDbRequest::new( CentralDbCommand::HasSupervisor);
        self.central_db.send( request ).await;
        match rx.await
        {
            Ok(ok) => {
                match ok
                {
                    Ok(result) => {
                        match result
                        {
                            CentralDbResult::HasSupervisor(rtn) => {rtn}
                            _ => false
                        }
                    }
                    Err(err) => {
                        false
                    }
                }
            }
            Err(error) => {false}
        }
    }

    async fn select_supervisor(&mut self) -> Option<StarKey> {

        let (request,rx) = CentralDbRequest::new( CentralDbCommand::SelectSupervisor );
        self.central_db.send( request ).await;
        match rx.await
        {
            Ok(ok) => {
                match ok
                {
                    Ok(result) => {
                        match result
                        {
                            CentralDbResult::Supervisor(rtn) => {rtn}
                            _ => Option::None
                        }
                    }
                    Err(err) => {
                        Option::None
                    }
                }
            }
            Err(error) => {Option::None}
        }
    }
}

pub struct CentralDbRequest
{
    pub command: CentralDbCommand,
    pub tx: oneshot::Sender<Result<CentralDbResult,Error>>
}

impl CentralDbRequest
{
    pub fn new(command: CentralDbCommand)->(Self,oneshot::Receiver<Result<CentralDbResult,Error>>)
    {
        let (tx,rx) = oneshot::channel();
        (CentralDbRequest
        {
            command: command,
            tx: tx
        },
        rx)
    }
}

pub enum CentralDbCommand
{
    Close,
    AddSupervisor(StarKey),
    RemoveSupervisor(StarKey),
    SetSupervisorForApplication((StarKey,AppKey)),
    GetSupervisorForApplication(AppKey),
    HasSupervisor,
    SelectSupervisor,
}

pub enum CentralDbResult
{
    Ok,
    SupervisorForApplication(Option<StarKey>),
    HasSupervisor(bool),
    Supervisor(Option<StarKey>)
}

pub struct CentralDb {
    conn: Connection,
    rx: mpsc::Receiver<CentralDbRequest>
}

impl CentralDb {

    pub async fn new() -> mpsc::Sender<CentralDbRequest> {
        let (tx,rx) = mpsc::channel(2*1024);
        tokio::spawn( async move {
          let conn = Connection::open_in_memory();
          if conn.is_ok()
          {
              let mut db = CentralDb
              {
                  conn: conn.unwrap(),
                  rx: rx
              };

              db.run().await;
          }

        });

        tx
    }

    pub async fn run(&mut self)->Result<(),Error>
    {
        self.setup();

        while let Option::Some(request) = self.rx.recv().await
        {
            match request.command
            {
                CentralDbCommand::Close => {
                    break;
                }
                CentralDbCommand::AddSupervisor(key) => {
                    let blob = bincode::serialize(&key).unwrap();
                    let result = self.conn.execute("INSERT INTO supervisors (key) VALUES (?1)", [blob]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(CentralDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }
                CentralDbCommand::RemoveSupervisor(key) => {
                    let blob = bincode::serialize(&key).unwrap();
                    let result = self.conn.execute("DELETE FROM supervisors WHERE key=?", [blob]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(CentralDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }
                CentralDbCommand::HasSupervisor => {
                    let result = self.conn.query_row("SELECT count(*) FROM supervisors", [], |row| {
                        let count: usize = row.get(0)?;
                        Ok(count)
                    });
                    match result
                    {
                        Ok(count) => {
                            request.tx.send(Result::Ok(CentralDbResult::HasSupervisor(count > 0)));
                        }
                        Err(e) => {
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }
                CentralDbCommand::SelectSupervisor => {
                    let result = self.conn.query_row("SELECT * FROM supervisors", [], |row| {
                        let rtn: Vec<u8> = row.get(0)?;
                        Ok(bincode::deserialize::<StarKey>(rtn.as_slice()))
                    });
                    match result
                    {
                        Ok(result) => {
                            match result
                            {
                                Ok(star) => {
                                    request.tx.send(Result::Ok(CentralDbResult::Supervisor(Option::Some(star))));
                                }
                                Err(error) => {
                                    request.tx.send(Result::Ok(CentralDbResult::Supervisor(Option::None)));
                                }
                            }
                        }
                        Err(err) => {
                            request.tx.send(Result::Ok(CentralDbResult::Supervisor(Option::None)));
                        }
                    }
                }
                CentralDbCommand::GetSupervisorForApplication(app) => {
                    let app = bincode::serialize(&app).unwrap();
                    let result = self.conn.query_row("SELECT supervisors.key FROM supervisors,apps_to_supervisors WHERE apps_to_supervisors.app_key=?1 AND apps_to_supervisors.supervisor_key=supervisors.key", [app], |row| {
                        let rtn: Vec<u8> = row.get(0)?;
                        Ok(bincode::deserialize::<StarKey>(rtn.as_slice()))
                    });
                    match result
                    {
                        Ok(result) => {
                            match result
                            {
                                Ok(star) => {
                                    request.tx.send(Result::Ok(CentralDbResult::Supervisor(Option::Some(star))));
                                }
                                Err(error) => {
                                    println!("(1)error: {}", error);
                                    request.tx.send(Result::Ok(CentralDbResult::Supervisor(Option::None)));
                                }
                            }
                        }
                        Err(err) => {
                            println!("(2)error: {}", err);
                            request.tx.send(Result::Ok(CentralDbResult::Supervisor(Option::None)));
                        }
                    }
                }
                CentralDbCommand::SetSupervisorForApplication((supervisor, app)) => {
                    let supervisor = bincode::serialize(&supervisor).unwrap();
                    let app = bincode::serialize(&app).unwrap();

                    self.conn.execute("BEGIN TRANSACTION", []);
                    self.conn.execute("INSERT INTO apps (key) VALUES (?1)", [app.clone()]);
                    self.conn.execute("INSERT INTO apps_to_supervisors (app_key,supervisor_key) VALUES (?1,?2)", [app.clone(), supervisor]);
                    let result = self.conn.execute("COMMIT TRANSACTION", []);

                    match result
                    {
                        Ok(_) => {
                            println!("Supervisor set for application!");
                            request.tx.send(Result::Ok(CentralDbResult::Ok));
                        }
                        Err(e) => {
                            println!("ERROR setting supervisor app: {}", e);
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }
            }
        }

       Ok(())
    }

    pub fn setup(&self)
    {
        let setup = r#"
       CREATE TABLE supervisors(
	      key BLOB PRIMARY KEY
        );

       CREATE TABLE apps (
         key BLOB PRIMARY KEY
        );

        CREATE TABLE apps_to_supervisors
        {
           supervisor_key BLOB,
           app_key BLOB,
           PRIMARY KEY (supervisor_key, app_key),
           FOREIGN KEY (supervisors_key)
              REFERENCES supervisors (key)
                  ON DELETE CASCADE,
                  ON UPDATE NO ACTION,
           FOREIGN KEY (app_key)
              REFERENCES apps (key)
                  ON DELETE CASCADE,
                  ON UPDATE NO ACTION,
        };
        "#;


        self.conn.execute(setup, []).unwrap();

    }

}