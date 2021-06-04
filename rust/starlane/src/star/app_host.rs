use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use tokio::sync::mpsc::error::SendError;

use crate::actor::{ActorKey};
use crate::app::{AppMeta, AppLocation, AppStatus, AppReadyStatus, AppPanicReason, AppArchetype, InitData, AppCreateResult};
use crate::error::Error;
use crate::frame::{ActorLookup, AppNotifyCreated, AssignMessage, Frame, Reply, SpaceMessage, SpacePayload, StarMessage, StarMessagePayload, ServerAppPayload, SpaceReply, SimpleReply, SupervisorPayload, ServerPayload, StarPattern, FromReply, ChildResourceAction, WindAction};
use crate::keys::{AppKey, UserKey, ResourceKey};
use crate::logger::{Flag, Log, StarFlag, StarLog, StarLogPayload};
use crate::message::{MessageExpect, ProtoMessage, MessageExpectWait, Fail};
use crate::star::{StarCommand, StarSkel, StarInfo, StarKey, StarVariant, StarVariantCommand, StarKind, ResourceRegistryBacking, ResourceRegistryBackingSqLite, Wind};
use tokio::sync::oneshot::Receiver;
use tokio::sync::oneshot::error::RecvError;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{oneshot, mpsc};
use rusqlite::{Connection,params};
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::resource::{Registry, FieldSelection, ResourceType, ResourceLocationRecord, ResourceRegistryAction};
use std::future::Future;
use crate::star::pledge::{StarHandleBacking, StarHandle};
use tokio::time::Duration;
use std::sync::Arc;


pub struct SetAppServerStatus
{
    pub app: AppKey,
    pub server: StarKey,
    pub status: AppServerStatus
}



pub struct SetAppStatus
{
    pub app: AppKey,
    pub status: AppStatus
}

pub struct SupervisorVariant
{
    skel: StarSkel,
    backing: Box<dyn SupervisorManagerBacking>,
    star_handles: StarHandleBacking
}

impl SupervisorVariant
{
    pub async fn new(data: StarSkel) ->Self
    {
        SupervisorVariant {
            skel: data.clone(),
            backing: Box::new(SupervisorStarVariantBackingSqLite::new().await ),
            star_handles: StarHandleBacking::new().await
        }
    }
}

impl SupervisorVariant
{

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
        message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error(error_message.to_string()))));
        let result = self.skel.star_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await;
        self.unwrap(result);
    }

}

#[async_trait]
impl StarVariant for SupervisorVariant
{
    async fn handle(&mut self, command: StarVariantCommand) {
        match command
        {
            StarVariantCommand::Init => {

                let (search, rx) = Wind::new(StarPattern::StarKind(StarKind::FileStore), WindAction::SearchHits);
                self.skel.star_tx.send(StarCommand::WindInit(search)).await;
                let result = tokio::time::timeout( Duration::from_secs(5), rx).await;
                if let Ok(Ok(hits)) = result
                {
                    for (star,hops)in hits.hits{
                        let handle = StarHandle{
                            key: star,
                            kind: StarKind::FileStore,
                            hops: Option::Some(hops)
                        };
                        self.star_handles.add_star_handle(handle).await;
                    }
                } else {
                  eprintln!("error encountered when attempting to get a handle on FileStore's")
                }
            }
            StarVariantCommand::StarMessage(star_message) =>
            {
                match &star_message.payload
                {
                    StarMessagePayload::ResourceManager(resource_message ) => {
                        unimplemented!()

                    }
                    StarMessagePayload::Space(space_message) => {
                        match &space_message.payload
                        {
                            SpacePayload::Supervisor(supervisor_payload) => {
                                match supervisor_payload
                                {

                                    SupervisorPayload::AppSequenceRequest(app_key) => {
println!("AppSEquenceRequest!");
                                        let index = self.backing.app_sequence_next(app_key).await;
                                        match index
                                        {
                                            Ok(index) => {
                                                let reply = star_message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Seq(index))));
                                                self.skel.star_tx.send(StarCommand::SendProtoMessage(reply)).await;
                                            }
                                            Err(error) => {
                                                let reply = star_message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error("could not generate sequence".to_string()))));
                                                self.skel.star_tx.send(StarCommand::SendProtoMessage(reply)).await;
                                            }
                                        }
                                    }
                                }
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

    /*
    async fn handle(&mut self, command: StarManagerCommand)  {

        match &command
        {

           StarManagerCommand::Init => {
               self.pledge().await;
           }
           StarManagerCommand::StarMessage(message)=>{
              match &message.payload
              {
                  StarMessagePayload::Central(central) => {

                      match central
                      {
                          StarMessageCentral::Pledge(_) => {
                              self.backing.add_server(message.from.clone());
                              self.reply_ok(message.clone()).await;
                              if self.skel.flags.check( Flag::Star(StarFlag::DiagnosePledge )) {

                                  self.skel.logger.log( Log::Star(StarLog::new(&self.skel.info, StarLogPayload::PledgeRecv )));
                              }
                          }
                      }
                  }
                  StarMessagePayload::Space(space_message) =>
                  {
                      match &space_message.payload
                      {
                      /*    SpacePayload::Assign(assign) => {
                              match assign
                              {
                                  AssignMessage::App(launch) => {

                                  }
                              }

                       */
                          SpacePayload::Supervisor(payload) => {
                              match payload
                              {
                                  SupervisorPayload::AppSequenceRequest(_) => {
                                      unimplemented!()
                                  }
                                  SupervisorPayload::ActorRegister(_) => {
                                      unimplemented!();
                                  }
                                  SupervisorPayload::ActorUnRegister(_) => {
                                      unimplemented!();
                                  }
                                  SupervisorPayload::ActorStatus(_) => {
                                      unimplemented!();
                                  }
                              }

                          }
                          _ => {}
                      }

                      }

                  StarMessagePayload::None => {}
                  StarMessagePayload::Reply(_) => {}
              }

              }
            StarManagerCommand::StarSkel(_) => {}
            StarManagerCommand::CoreRequest(_) => {}
            StarManagerCommand::CentralCommand(_) => {}
            StarManagerCommand::SupervisorCommand(_) => {}
            StarManagerCommand::ServerCommand(_) => {}
        }



    }}
     */
}



impl SupervisorVariant
{
    async fn handle_message(&mut self, message: StarMessage) {

    }
}

pub struct SupervisorManagerBackingDefault
{
    data: StarSkel,
    servers: Vec<StarKey>,
    server_select_index: usize,
    applications: HashMap<AppKey, AppArchetype>,
    actor_location: HashMap<ResourceKey, ResourceLocationRecord>,
    app_status: HashMap<AppKey,AppStatus>,
    app_server_status: HashMap<(AppKey,StarKey),AppServerStatus>,
    app_to_servers: HashMap<AppKey,HashSet<StarKey>>,
    app_sequence: HashMap<AppKey,AtomicU64>
}

impl SupervisorManagerBackingDefault
{
    pub fn new(data: StarSkel) ->Self
    {
        SupervisorManagerBackingDefault {
            data: data,
            servers: vec![],
            server_select_index: 0,
            applications: HashMap::new(),
            actor_location: HashMap::new(),
            app_status: HashMap::new(),
            app_server_status: HashMap::new(),
            app_to_servers: HashMap::new(),
            app_sequence: HashMap::new()
        }
    }
}




#[async_trait]
pub trait SupervisorManagerBacking: Send+Sync
{
    async fn add_server( &mut self, info: StarInfo ) -> Result<(),Error>;
    async fn select_servers(&mut self, pattern: StarPattern ) -> Vec<StarKey>;

    async fn set_application(&mut self, app: AppKey, archetype: AppArchetype) -> Result<(),Error>;
    async fn get_application(&mut self, app: &AppKey ) -> Option<AppArchetype>;
    async fn set_app_server_status(&mut self, app: AppKey, server: StarKey, status: AppServerStatus) -> Result<(),Error>;
    async fn get_app_server_status(&mut self, app: &AppKey, server: &StarKey) ->AppServerStatus;
    async fn set_app_status(&mut self, app: AppKey, status: AppStatus )-> Result<(),Error>;
    async fn get_app_status(&mut self, app: &AppKey) -> AppStatus;
    async fn add_app_to_server(&mut self, app: AppKey, server: StarKey)-> Result<(),Error>;
    async fn get_servers_for_app(&mut self, app: &AppKey ) -> Vec<StarKey>;

    async fn app_sequence_next(&mut self, app: &AppKey ) -> Result<u64,Error>;

    async fn set_actor_location(&mut self, location: ResourceLocationRecord) -> Result<(),Error>;
    async fn get_actor_location(&self, actor: &ResourceKey) -> Option<ResourceLocationRecord>;
}


#[derive(Clone,Serialize,Deserialize)]
pub enum AppServerStatus
{
    Unknown,
    Assigning,
    Ready
}


impl fmt::Display for AppServerStatus{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            AppServerStatus::Unknown => "Unknown".to_string(),
            AppServerStatus::Assigning => "Assigning".to_string(),
            AppServerStatus::Ready =>  "Ready".to_string()
        };
        write!(f, "{}",r)
    }
}

pub struct SupervisorStarVariantBackingSqLite
{
    pub supervisor_db: mpsc::Sender<SupervisorDbRequest>,
}

impl SupervisorStarVariantBackingSqLite
{
    pub async fn new()->Self
    {
        SupervisorStarVariantBackingSqLite {
            supervisor_db: SupervisorDb::new().await,
        }
    }

    pub fn handle( &self, result: Result<Result<SupervisorDbResult,Error>,RecvError>)->Result<(),Error>
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
impl SupervisorManagerBacking for SupervisorStarVariantBackingSqLite
{
    async fn add_server(&mut self, info: StarInfo ) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::AddServer(info));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn select_servers(&mut self, pattern: StarPattern) -> Vec<StarKey> {
println!("SELECT SERVERS!");
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::StarSelect(pattern));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::Servers(servers) => {

                                println!("rtn servers: {}", servers.len() );
                                servers
                            }
                            _ => vec![]
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        vec![]
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                vec![]
            }
        }
    }

    async fn set_application(&mut self, app: AppKey, archetype: AppArchetype) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetAppArchetype(app, archetype));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_application(&mut self, app: &AppKey) -> Option<AppArchetype> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetAppArchetype(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppArchetype(archetype) => {
                                Option::Some(archetype)
                            }
                            _ => Option::None
                        }
                    }
                    Err(err) => {
                        Option::None
                    }
                }
            }
            Err(err) => {
                Option::None
            }
        }
    }

    async fn set_app_server_status(&mut self, app: AppKey, server: StarKey, status: AppServerStatus) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetAppServerStatus(app,server,status));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_app_server_status(&mut self, app: &AppKey, server: &StarKey) -> AppServerStatus {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetAppServerStatus(app.clone(),server.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppServerStatus(status) => {
                                status
                            }
                            _ => AppServerStatus::Unknown
                        }
                    }
                    Err(err) => {
                        AppServerStatus::Unknown
                    }
                }
            }
            Err(err) => {
                AppServerStatus::Unknown
            }
        }
    }

    async fn set_app_status(&mut self, app: AppKey, status: AppStatus) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetAppStatus(app,status));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_app_status(&mut self, app: &AppKey) -> AppStatus {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetAppStatus(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppStatus(status) => {
                               status
                            }
                            _ => AppStatus::Unknown
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        AppStatus::Unknown
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                AppStatus::Unknown
            }
        }
    }

    async fn add_app_to_server(&mut self, app: AppKey, server: StarKey) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::AddAppToServer(app, server));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_servers_for_app(&mut self, app: &AppKey) -> Vec<StarKey> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetServersForApp(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::Servers(servers) => {
                                servers
                            }
                            _ => vec![]
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        vec![]
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                vec![]
            }
        }
    }

    async fn app_sequence_next(&mut self, app: &AppKey) -> Result<u64, Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::AppSequenceNext(app.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::AppSequenceNext(seq) => {
                                Ok(seq)
                            }
                            _ => Err("unexpected when trying to get seq".into())
                        }
                    }
                    Err(err) => {
                        eprintln!("{}",err);
                        Err("unexpected when trying to get seq".into())
                    }
                }
            }
            Err(err) => {
                eprintln!("{}",err);
                Err("unexpected when trying to get seq".into())
            }
        }
    }

    async fn set_actor_location(&mut self, location: ResourceLocationRecord) -> Result<(), Error> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::SetResourceLocation(location));
        self.supervisor_db.send( request ).await;
        self.handle(rx.await)
    }

    async fn get_actor_location(&self, actor: &ResourceKey) -> Option<ResourceLocationRecord> {
        let (request,rx) = SupervisorDbRequest::new( SupervisorDbCommand::GetActorLocation(actor.clone()));
        self.supervisor_db.send( request ).await;
        match rx.await
        {
            Ok(result) => {
                match result
                {
                    Ok(ok) => {
                        match ok
                        {
                            SupervisorDbResult::ActorLocation(location ) => {
                                Option::Some(location)
                            }
                            _ => Option::None
                        }
                    }
                    Err(err) => {
                        Option::None
                    }
                }
            }
            Err(err) => {
                Option::None
            }
        }
    }
}

pub struct SupervisorDbRequest
{
    pub command: SupervisorDbCommand,
    pub tx: oneshot::Sender<Result<SupervisorDbResult,Error>>
}

impl SupervisorDbRequest
{
    pub fn new(command: SupervisorDbCommand)->(Self,oneshot::Receiver<Result<SupervisorDbResult,Error>>)
    {
        let (tx,rx) = oneshot::channel();
        (SupervisorDbRequest
         {
             command: command,
             tx: tx
         },
         rx)
    }
}

pub enum SupervisorDbCommand
{
    Close,
    AddServer(StarInfo),
    StarSelect(StarPattern),
    AddAppToServer( AppKey, StarKey),
    SetAppArchetype(AppKey,AppArchetype),
    GetAppArchetype(AppKey),
    SetAppStatus(AppKey,AppStatus),
    SetAppServerStatus(AppKey,StarKey,AppServerStatus),
    GetAppServerStatus(AppKey,StarKey),
    GetAppStatus(AppKey),
    GetServersForApp(AppKey),
    AppSequenceNext(AppKey),
    SetResourceLocation(ResourceLocationRecord),
    GetActorLocation(ResourceKey)
}

pub enum SupervisorDbResult
{
    Ok,
    Error(String),
    Server(Option<StarKey>),
    Servers(Vec<StarKey>),
    Supervisor(Option<StarKey>),
    AppArchetype(AppArchetype),
    AppStatus(AppStatus),
    AppServerStatus(AppServerStatus),
    AppSequenceNext(u64),
    ActorLocation(ResourceLocationRecord)
}

pub struct SupervisorDb {
    conn: Connection,
    rx: mpsc::Receiver<SupervisorDbRequest>
}

impl SupervisorDb {

    pub async fn new() -> mpsc::Sender<SupervisorDbRequest> {
        let (tx,rx) = mpsc::channel(2*1024);
        tokio::spawn( async move {
            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = SupervisorDb
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
                SupervisorDbCommand::Close => {
                    break;
                }
                SupervisorDbCommand::AddServer(info) => {
                    let server = bincode::serialize(&info.star ).unwrap();
                    let result = self.conn.execute("INSERT INTO servers (key,kind) VALUES (?1,?2)", params![server,info.kind.to_string()]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok) );
                        }
                        Err(e) => {
                            request.tx.send(Result::Err(e.into()) );
                        }
                    }
                }
                SupervisorDbCommand::StarSelect(pattern) => {
println!("Star SELECT ");
                    let mut statement = self.conn.prepare("SELECT key,kind FROM servers");
                    if let Result::Ok(mut statement) = statement
                    {
                        println!("got here");
                        let servers = statement.query_map( params![], |row|{
                            let key: Vec<u8> = row.get(0).unwrap();
                            if let Result::Ok(key) = bincode::deserialize::<StarKey>(key.as_slice()){
                                let kind: String = row.get(1).unwrap();
                                let kind =   StarKind::from_str(kind.as_str() ).unwrap();
                                let info = StarInfo::new(key,kind);
                                Ok(info)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        } ).unwrap();

                        let mut rtn = vec![];
                        for server in servers
                        {
                            if let Result::Ok(server) = server
                            {
                                if pattern.is_match(&server)
                                {
println!("adding server...");
                                    rtn.push(server.star );
                                }
                            }
                        }


                        request.tx.send( Result::Ok( SupervisorDbResult::Servers(rtn)));
println!("blah returning... ...");

                    }

                }
                SupervisorDbCommand::SetAppServerStatus(app, server,status ) => {
                    let server= bincode::serialize(&server).unwrap();
                    let app = bincode::serialize(&app).unwrap();

                    let transaction = self.conn.transaction().unwrap();
                    transaction.execute("REPLACE INTO apps (key) VALUES (?1)", [app.clone()]);
                    transaction.execute("REPLACE INTO apps_to_servers (app_key,server_key,status) VALUES (?1,?2,?3)", params![app.clone(), server.clone(),status.to_string()]);
                    let result = transaction.commit();

                    match result
                    {
                        Ok(_) => {
                            println!("Server set for application!");
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }

                        Err(e) => {
                            println!("ERROR setting server app: {}", e);
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }

                SupervisorDbCommand::SetAppStatus(app, status) => {
                    let app= bincode::serialize(&app).unwrap();

                    let result = self.conn.execute("REPLACE INTO apps_status (key,status) VALUES (?1,?2)", params![app.clone(),status.to_string()]);

                    match result
                    {
                        Ok(_) => {
                            println!("app status SET");
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
println!("SET APP STATUS ERROR");
                            request.tx.send(Result::Err(e.into()));
                        }
                    }

                }
                SupervisorDbCommand::GetAppStatus(app) => {
                    let app = bincode::serialize(&app ).unwrap();
                    let result = self.conn.query_row("SELECT status FROM apps_status WHERE key=?1", params![app], |row|
                        {
                            let status:String = row.get(0).unwrap();
                            let status:AppStatus = AppStatus::from_str(status.as_str()).unwrap();
                            Ok(status)
                        });
                    match result
                    {
                        Ok(status) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::AppStatus(status)) );
                        }
                        Err(rusqlite::Error::QueryReturnedNoRows) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::AppStatus(AppStatus::Unknown)) );
                        }
                        Err(e) => {
println!("GET APP STATUS ERROR: {}",e);
                            request.tx.send(Result::Err(e.into()) );
                        }
                    }
                }

                SupervisorDbCommand::AppSequenceNext(app) => {
                    let app = bincode::serialize(&app).unwrap();

                    let result = {
                        let transaction = self.conn.transaction().unwrap();
                        transaction.execute("UPDATE apps SET sequence=sequence+1 WHERE key=?1", [app.clone()]);
                        let result = transaction.query_row("SELECT sequence FROM apps WHERE key=?1", params![app.clone()], |row| {
                            let rtn: u64 = row.get(0).unwrap();
                            Ok(rtn)
                        });
                        let trans_result= transaction.commit();
                        if trans_result.is_err()
                        {
                            Err(trans_result.err().unwrap())
                        }
                        else {
                            result
                        }
                    };
                    match result
                    {
                        Ok(result) => {
                            println!("incremented app sequence!!!");
                            request.tx.send(Result::Ok(SupervisorDbResult::AppSequenceNext(result)));
                        }

                        Err(e) => {
                            println!("ERROR APP SEQUENCE NEXT: {}", e);
                            request.tx.send(Result::Err(e.into()));
                        }
                    }
                }

                SupervisorDbCommand::GetServersForApp(app) => {
                    println!("Get Servers For App");
                    let app = bincode::serialize(&app).unwrap();
                    let mut statement = self.conn.prepare("SELECT server_key FROM apps_to_servers WHERE app_key=?1 AND status='Ready'");
                    if let Result::Ok(mut statement) = statement
                    {
                        println!("got here");
                        let servers = statement.query_map( params![app], |row|{
                            let key: Vec<u8> = row.get(0).unwrap();
                            if let Result::Ok(key) = bincode::deserialize::<StarKey>(key.as_slice()){
                                Ok(key)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        } ).unwrap();

                        let mut rtn = vec![];
                        let servers:Vec<StarKey> = servers.into_iter().map(|r|r.unwrap()).collect();
                        request.tx.send( Result::Ok( SupervisorDbResult::Servers(rtn)));
                        println!("blah returning from GEtServers...... ...");
                    }
                }
                SupervisorDbCommand::AddAppToServer(server, app) => {
                    let app = bincode::serialize(&app).unwrap();
                    let server = bincode::serialize(&server).unwrap();

                    let result = self.conn.execute("REPLACE INTO apps_to_servers (app_key,server_key) VALUES (?1,?2)", params![app,server]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Error("AddApplicationToServer failed".into())));
                        }
                    }
                }
                SupervisorDbCommand::SetAppArchetype(app,archetype) => {
                    let app = bincode::serialize(&app).unwrap();
                    let archetype = bincode::serialize(&archetype).unwrap();

                    let result = self.conn.execute("REPLACE INTO apps (key,archetype) VALUES (?1,?2)", params![app,archetype]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Error("AddApplicationToServer failed".into())));
                        }
                    }
                }
                SupervisorDbCommand::GetAppArchetype(app) => {
                    let app = bincode::serialize(&app).unwrap();
                    let result = self.conn.query_row("SELECT archetype FROM apps WHERE key=?1", params![app], |row|
                        {
                            let archetype: Vec<u8>= row.get(0).unwrap();
                            if let Result::Ok(archetype) = bincode::deserialize::<AppArchetype>(archetype.as_slice()) {
                                Ok(archetype)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        });
                    match result
                    {
                        Ok(archetype) => {
                            request.tx.send(Ok(SupervisorDbResult::AppArchetype(archetype)));
                        }
                        Err(_) => {
                            request.tx.send(Ok(SupervisorDbResult::Error("could not find archetype".to_string())));
                        }
                    }
                }
                SupervisorDbCommand::GetAppServerStatus(app, server) => {
                    let app = bincode::serialize(&app).unwrap();
                    let server = bincode::serialize(&server).unwrap();
                    let result = self.conn.query_row("SELECT status FROM apps_to_servers WHERE app_key=?1 AND server_key=?2", params![app,server], |row|
                        {
                            let status: Vec<u8>= row.get(0).unwrap();
                            if let Result::Ok(status) = bincode::deserialize::<AppServerStatus>(status.as_slice()) {
                                Ok(status)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        });
                    match result
                    {
                        Ok(status) => {
                            request.tx.send(Ok(SupervisorDbResult::AppServerStatus(status)));
                        }
                        Err(_) => {
                            request.tx.send(Ok(SupervisorDbResult::AppServerStatus(AppServerStatus::Unknown)));
                        }
                    }
                }
                SupervisorDbCommand::SetResourceLocation(loc) => {
                    let actor = bincode::serialize(&loc.key).unwrap();
                    let location = bincode::serialize(&loc).unwrap();

                    let result = self.conn.execute("REPLACE INTO actors (key,location) VALUES (?1,?2)", params![actor,location]);
                    match result
                    {
                        Ok(_) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Ok));
                        }
                        Err(e) => {
                            request.tx.send(Result::Ok(SupervisorDbResult::Error("SetActorLocation failed".into())));
                        }
                    }
                }
                SupervisorDbCommand::GetActorLocation(actor) => {
                    let actor= bincode::serialize(&actor).unwrap();
                    let result = self.conn.query_row("SELECT location FROM actors WHERE key=?1", params![actor], |row|
                        {
                            let location : Vec<u8>= row.get(0).unwrap();
                            if let Result::Ok(location) = bincode::deserialize::<ResourceLocationRecord>(location.as_slice()) {
                                Ok(location)
                            }
                            else
                            {
                                Err(rusqlite::Error::ExecuteReturnedResults)
                            }
                        });
                    match result
                    {
                        Ok(location) => {
                            request.tx.send(Ok(SupervisorDbResult::ActorLocation(location)));
                        }
                        Err(_) => {
                            request.tx.send(Ok(SupervisorDbResult::Error("could not find actor location".to_string())));
                        }
                    }
                }

            }

        }

        Ok(())
    }

    pub fn setup(&mut self)
    {
        let servers= r#"
       CREATE TABLE IF NOT EXISTS servers(
	      key BLOB PRIMARY KEY,
	      kind TEXT NOT NULL
        );"#;

       let apps = r#"CREATE TABLE IF NOT EXISTS apps (
         key BLOB PRIMARY KEY,
         archetype BLOB,
         sequence INTEGER DEFAULT 0
        );"#;

        let apps_status = r#"CREATE TABLE IF NOT EXISTS apps_status (
         key BLOB PRIMARY KEY,
         status TEXT NOT NULL
        );"#;


        let actors = r#"CREATE TABLE IF NOT EXISTS actors (
         key BLOB PRIMARY KEY,
         location BLOB NOT NULL
        );"#;

        let apps_to_servers = r#"CREATE TABLE IF NOT EXISTS apps_to_servers
        (
           server_key BLOB,
           app_key BLOB,
           status TEXT NOT NULL DEFAULT 'Unknown',
           PRIMARY KEY (server_key, app_key),
           FOREIGN KEY (server_key) REFERENCES servers (key),
           FOREIGN KEY (app_key) REFERENCES apps (key)
        );"#;

        {
            let transaction = self.conn.transaction().unwrap();
            transaction.execute(servers, []).unwrap();
            transaction.execute(apps, []).unwrap();
            transaction.execute(apps_status, []).unwrap();
            transaction.execute(actors, []).unwrap();
            transaction.execute(apps_to_servers, []).unwrap();
            transaction.commit();
        }

    }

}