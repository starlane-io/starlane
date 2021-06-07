use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::iter::FromIterator;
use std::sync::Arc;

use bincode::ErrorKind;
use futures::{FutureExt, TryFutureExt};
use rusqlite::Connection;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot::error::RecvError;
use tokio::sync::oneshot::Receiver;
use tokio::time::Duration;

use crate::app::{AppArchetype, AppCreateController, ApplicationStatus, AppLocation, AppMeta, InitData};
use crate::crypt::{CryptKeyId, PublicKey};
use crate::error::Error;
use crate::frame::{AppLabelRequest, AssignMessage, ChildManagerResourceAction, Frame, FromReply, Reply, SequenceMessage, ServerPayload, SimpleReply, SpaceMessage, SpacePayload, SpaceReply, StarMessage, StarMessagePayload, SupervisorPayload};
use crate::id::Id;
use crate::keys::{AppId, AppKey, ResourceKey, SpaceKey, SubSpaceKey, UserId, UserKey};
use crate::logger::{Flag, Log, Logger, StarFlag, StarLog, StarLogPayload};
use crate::message::{Fail, MessageExpect, MessageExpectWait, MessageResult, MessageUpdate, ProtoMessage};
use crate::message::Fail::ResourceCannotGenerateAddress;
use crate::permissions::{AppAccess, AuthToken, User, UserKind};
use crate::resource::{AddressCreationSrc, AssignResourceStateSrc, ChildResourceManager, FieldSelection, KeyCreationSrc, Labels, Registry, ResourceAddress, ResourceAddressPart, ResourceArchetype, ResourceAssign, ResourceCreate, ResourceInit, ResourceKind, ResourceRecord, ResourceManager, ResourceRegistration, ResourceRegistryCommand, ResourceRegistryResult, ResourceSelector, ResourceType};
use crate::resource::space::SpaceState;
use crate::resource::user::UserState;
use crate::star::{CentralCommand, ForwardFrame, PublicKeySource, ResourceRegistryBacking, ResourceRegistryBackingSqLite, SetSupervisorForApp, StarCommand, StarInfo, StarKey, StarKind, StarNotify, StarSkel, StarVariant, StarVariantCommand};
use crate::star::StarCommand::SpaceCommand;
use crate::starlane::api::StarlaneApi;

pub struct CentralStarVariant
{
    skel: StarSkel,
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
            skel: data.clone(),
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
        let result = self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        self.unwrap(result);
    }

    pub async fn reply_error(&self, mut message: StarMessage, error_message: String )
    {
        let mut proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error(error_message))));
        let result = self.skel.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
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
            StarVariantCommand::Init => {
println!("Central: CALLING ENSURE!");
                self.ensure().await;
            }

            StarVariantCommand::CentralCommand(_) => {}
            _ => {}
        }
    }
}

impl CentralStarVariant{

    async fn ensure(&self){
        self.ensure_hyperspace().await.unwrap();
        self.ensure_user(&ResourceAddress::for_space("hyperspace").unwrap(),"hyperuser@starlane.io").await.unwrap();
        self.ensure_subspace(&ResourceAddress::for_space("hyperspace").unwrap(),"default").await.unwrap();
    }

    async fn ensure_hyperspace(&self)->Result<(),Error>{
println!("ENSURING HYPERSPACE!");
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        starlane_api.create_space("hyperspace", "HyperSpace").await?;

        Ok(())
    }

    async fn ensure_user(&self, space_address: &ResourceAddress, email: &str ) ->Result<(),Error>{
println!("ENSURING USER!");
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        let space_api = starlane_api.get_space(space_address.clone().into() ).await?;
        space_api.create_user(email).await?;
        Ok(())
    }

    async fn ensure_subspace(&self, space_address: &ResourceAddress, sub_space: &str ) ->Result<(),Error>{
        println!("---------- sub space ---------------");
        println!("ENSURING SUB SPACE" );
        let starlane_api = StarlaneApi::new(self.skel.star_tx.clone());
        let space_api = starlane_api.get_space(space_address.clone().into()).await?;
        space_api.create_sub_space(sub_space).await?;
        Ok(())
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


struct CentralStarVariantBackingSqlLite
{
    central_db: mpsc::Sender<CentralDbRequest>
}

impl CentralStarVariantBackingSqlLite
{
    pub async fn new()->Self
    {
        CentralStarVariantBackingSqlLite{
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
        let (request, rx) = CentralDbRequest::new(CentralDbCommand::AddSupervisor(star));
        self.central_db.send(request).await;
        self.handle(rx.await)
    }

    async fn remove_supervisor(&mut self, star: StarKey) -> Result<(), Error> {
        let (request, rx) = CentralDbRequest::new(CentralDbCommand::RemoveSupervisor(star));
        self.central_db.send(request).await;
        self.handle(rx.await)
    }

    async fn set_supervisor_for_application(&mut self, app: AppKey, supervisor_star: StarKey) -> Result<(), Error> {
        let (request, rx) = CentralDbRequest::new(CentralDbCommand::SetSupervisorForApplication((supervisor_star, app)));
        self.central_db.send(request).await;
        self.handle(rx.await)
    }

    async fn get_supervisor_for_application(&self, app: &AppKey) -> Option<StarKey> {
        let (request, rx) = CentralDbRequest::new(CentralDbCommand::GetSupervisorForApplication(app.clone()));
        self.central_db.send(request).await;
        match rx.await
        {
            Ok(ok) => {
                match ok
                {
                    Ok(ok) => {
                        match ok
                        {
                            CentralDbResult::Supervisor(supervisor) => { supervisor }
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
        let (request, rx) = CentralDbRequest::new(CentralDbCommand::HasSupervisor);
        self.central_db.send(request).await;
        match rx.await
        {
            Ok(ok) => {
                match ok
                {
                    Ok(result) => {
                        match result
                        {
                            CentralDbResult::HasSupervisor(rtn) => { rtn }
                            _ => false
                        }
                    }
                    Err(err) => {
                        false
                    }
                }
            }
            Err(error) => { false }
        }
    }

    async fn select_supervisor(&mut self) -> Option<StarKey> {
        let (request, rx) = CentralDbRequest::new(CentralDbCommand::SelectSupervisor);
        self.central_db.send(request).await;
        match rx.await
        {
            Ok(ok) => {
                match ok
                {
                    Ok(result) => {
                        match result
                        {
                            CentralDbResult::Supervisor(rtn) => { rtn }
                            _ => Option::None
                        }
                    }
                    Err(err) => {
                        Option::None
                    }
                }
            }
            Err(error) => { Option::None }
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

                    let transaction = self.conn.transaction().unwrap();
                    transaction.execute("INSERT INTO apps (key) VALUES (?1)", [app.clone()]);
                    transaction.execute("INSERT INTO apps_to_supervisors (app_key,supervisor_key) VALUES (?1,?2)", [app.clone(), supervisor]);
                    let result = transaction.commit();

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

    pub fn setup(&mut self)
    {
        let supervisors= r#"
       CREATE TABLE IF NOT EXISTS supervisors(
	      key BLOB PRIMARY KEY
        );"#;

       let apps = r#"CREATE TABLE IF NOT EXISTS apps (
         key BLOB PRIMARY KEY
        );"#;

        let apps_to_supervisors = r#"CREATE TABLE IF NOT EXISTS apps_to_supervisors
        (
           supervisor_key BLOB,
           app_key BLOB,
           PRIMARY KEY (supervisor_key, app_key),
           FOREIGN KEY (supervisor_key) REFERENCES supervisors (key),
           FOREIGN KEY (app_key) REFERENCES apps (key)
        );
        "#;



        let transaction = self.conn.transaction().unwrap();
        transaction.execute(supervisors, []).unwrap();
        transaction.execute(apps, []).unwrap();
        transaction.execute(apps_to_supervisors, []).unwrap();
        transaction.commit();

    }

}