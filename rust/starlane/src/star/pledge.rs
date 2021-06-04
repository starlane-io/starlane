use std::collections::HashSet;
use std::iter::FromIterator;
use std::str::FromStr;

use rusqlite::{Connection, params, params_from_iter, ToSql};
use rusqlite::types::{ToSqlOutput, Value, ValueRef};
use tokio::sync::{mpsc, oneshot};
use tokio::sync::oneshot::error::RecvError;
use tokio::time::Duration;
use tokio::time::error::Elapsed;

use crate::error::Error;
use crate::frame::{Reply, ResourceHostAction, SimpleReply, StarMessagePayload};
use crate::message::{Fail, ProtoMessage};
use crate::resource::{ResourceStub, ResourceAssign, ResourceHost, ResourceType, RemoteResourceHost, ResourceLocationAffinity};
use crate::star::{LocalResourceLocation, StarComm, StarCommand, StarInfo, StarKey, StarKind, StarSkel};
use std::sync::Arc;
use futures::TryFutureExt;

#[derive(Clone)]
pub struct StarHandleBacking{
    tx: mpsc::Sender<StarHandleAction>
}

impl StarHandleBacking {

    pub async fn new()->Self {
        StarHandleBacking {
           tx: StarHandleDb::new().await
        }
    }

    pub async fn add_star_handle(&self, handle: StarHandle ) -> Result<(),Fail>{
       let (action,rx) = StarHandleAction::new(StarHandleCommand::SetStar(handle));
       self.tx.send( action ).await?;
       tokio::time::timeout(Duration::from_secs(5), rx).await??;
       Ok(())
    }

    pub async fn select( &self, selector: StarSelector ) -> Result<Vec<StarHandle>,Fail>{
        let (action,rx) = StarHandleAction::new(StarHandleCommand::Select(selector));
        self.tx.send( action ).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        if let StarHandleResult::StarHandles(handles) = result {
            Ok(handles)
        } else {
            Err(Fail::Unexpected)
        }
    }

    pub async fn next( &self, selector: StarSelector ) -> Result<StarHandle,Fail>{
        let (action,rx) = StarHandleAction::new(StarHandleCommand::Next(selector));
        self.tx.send( action ).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        if let StarHandleResult::StarHandle(handles) = result {
            Ok(handles)
        } else {
            Err(Fail::Unexpected)
        }
    }

    // must have at least one of each StarKind
    pub async fn satisfied( &self, set: HashSet<StarKind> ) -> Result<Satisfaction,Fail> {
        let (action,rx) = StarHandleAction::new(StarHandleCommand::Satisfied(set));
        self.tx.send( action ).await?;
        let result = tokio::time::timeout(Duration::from_secs(5), rx).await??;
        if let StarHandleResult::Satisfaction(satisfaction) = result {
            Ok(satisfaction)
        } else {
            Err(Fail::Unexpected)
        }
    }
}



#[derive(Clone)]
pub struct ResourceHostSelector{
   skel: StarSkel
}

impl ResourceHostSelector{
    pub fn new(skel: StarSkel ) -> Self {
        ResourceHostSelector{
            skel: skel,
        }
    }

    pub async fn select( &self, resource_type: ResourceType ) -> Result<Arc<dyn ResourceHost>,Fail>
    {
        if resource_type.star_host() == self.skel.info.kind{
            let handle = StarHandle{
                key: self.skel.info.key.clone(),
                kind: self.skel.info.kind.clone(),
                hops: None
            };
            let host = RemoteResourceHost{
                comm: self.skel.comm(),
                handle: handle
            };
            Ok(Arc::new(host))
        }
        else{
            let handler = self.skel.star_handler.as_ref().ok_or(format!("non-manager star {} does not have a host star selector", self.skel.info.kind ))?;
            let mut selector = StarSelector::new();
            selector.add(StarFieldSelection::Kind(resource_type.star_host()));
            let handle = handler.next(selector).await?;

            let host = RemoteResourceHost{
                comm: self.skel.comm(),
                handle: handle
            };

            Ok(Arc::new(host))
        }
    }
}

pub struct StarHandle {
    pub key: StarKey,
    pub kind: StarKind,
    pub hops: Option<usize>
}

pub struct StarSelector {
    fields: HashSet<StarFieldSelection>
}

impl ToString for StarSelector{
    fn to_string(&self) -> String {
        let mut rtn = String::new();

        for (index,field) in self.fields.iter().enumerate() {

            if index > 0 {
                rtn.push_str(", ");
            }
            rtn.push_str( field.to_string().as_str() );

        }

        rtn
    }
}

impl StarSelector {
    pub fn new()->Self{
        StarSelector{
            fields: HashSet::new()
        }
    }
    pub fn is_empty(&self)->bool {
        self.fields.is_empty()
    }

    pub fn add( &mut self, field: StarFieldSelection ) {
        self.fields.insert( field );
    }
}

#[derive(Clone,Hash,Eq,PartialEq)]
pub enum StarFieldSelection
{
    Kind(StarKind),
    MinHops
}

impl ToString for StarFieldSelection{

    fn to_string(&self) -> String {
        match self {
            StarFieldSelection::Kind(kind) => format!("Kind:{}",kind),
            StarFieldSelection::MinHops => format!("MinHops")
        }
    }
}

impl ToSql for StarFieldSelection
{
    fn to_sql(&self) -> Result<ToSqlOutput<'_>, rusqlite::Error> {
        match self
        {
            StarFieldSelection::Kind(kind) => {
                Ok(ToSqlOutput::Owned(Value::Text(kind.to_string())))
            }
            StarFieldSelection::MinHops => {
                Ok(ToSqlOutput::Owned(Value::Null))
            }
        }
    }
}

impl StarFieldSelection
{

    pub fn is_param(&self)->bool
    {
        match self {
            StarFieldSelection::Kind(_) => {
                true
            }
            StarFieldSelection::MinHops => {
                false
            }
        }
    }
}

pub struct StarHandleAction
{
    pub command: StarHandleCommand,
    pub tx: oneshot::Sender<StarHandleResult>
}

impl StarHandleAction
{
    pub fn new(command: StarHandleCommand) ->(Self, oneshot::Receiver<StarHandleResult>)
    {
        let (tx,rx) = oneshot::channel();
        (StarHandleAction{ tx: tx, command: command }, rx)
    }
}



pub enum StarHandleCommand {
    Close,
    SetStar(StarHandle),
    Select(StarSelector),
    Next(StarSelector),
    Satisfied(HashSet<StarKind>)
}

pub enum StarHandleResult
{
   Ok,
   StarHandles(Vec<StarHandle>),
   StarHandle(StarHandle),
   Fail(Fail),
   Satisfaction(Satisfaction)
}

#[derive(Eq,PartialEq,Debug)]
pub enum Satisfaction {
    Ok,
    Lacking(HashSet<StarKind>)
}


pub struct StarHandleDb{
    pub conn: Connection,
    pub rx: mpsc::Receiver<StarHandleAction>,
}


impl StarHandleDb {
    pub async fn new() -> mpsc::Sender<StarHandleAction>
    {
        let (tx, rx) = mpsc::channel(8 * 1024);

        tokio::spawn(async move {
            let conn = Connection::open_in_memory();
            if conn.is_ok()
            {
                let mut db = StarHandleDb {
                    conn: conn.unwrap(),
                    rx: rx,
                };
                db.run().await.unwrap()
            }
        });
        tx
    }

    async fn run(&mut self)->Result<(),Error>
    {
        self.setup()?;

        while let Option::Some(request) = self.rx.recv().await {
            if let StarHandleCommand::Close = request.command
            {
                break;
            }
            match self.process(request.command ).await
            {
                Ok(ok) => {
                    request.tx.send(ok);
                }
                Err(fail) => {
                    eprintln!("{}", fail.to_string());
                    request.tx.send(StarHandleResult::Fail(fail));
                }
            }
        }
        Ok(())
    }

    async fn process(&mut self, command: StarHandleCommand) -> Result<StarHandleResult,Fail>
    {
        match command
        {
            StarHandleCommand::Close => {
                // this is handle in the run() method
                Ok(StarHandleResult::Ok)
            }
            StarHandleCommand::SetStar(handle) => {
                let key = handle.key.bin()?;
                let kind = handle.kind.to_string();

                let mut trans = self.conn.transaction()?;
                if handle.hops.is_some() {
                    trans.execute("REPLACE INTO stars (key,kind,hops) VALUES (?1,?2,?3)", params![key,kind,handle.hops])?;
                } else {
                    trans.execute("REPLACE INTO stars (key,kind) VALUES (?1,?2)", params![key,kind])?;
                }
                trans.commit()?;

                Ok(StarHandleResult::Ok)
            }
            StarHandleCommand::Select(selector) => {
                let mut params = vec![];
                let mut where_clause = String::new();
                let mut param_index = 0;

                for (index, field) in Vec::from_iter(selector.fields.clone()).iter().map(|x| x.clone() ).enumerate()
                {
                    if index != 0 {
                        where_clause.push_str(" AND ");
                    }

                    let f = match &field {
                        StarFieldSelection::Kind(kind) => {
                            format!("kind=?{}", index + 1)
                        }
                        StarFieldSelection::MinHops => {
                            format!("hops NOT NULL AND hops=MIN(hops)")
                        }
                    };

                    where_clause.push_str(f.as_str());
                    if field.is_param() {
                        params.push(field);
                        param_index = param_index+1;
                    }
                }


                // in case this search was for EVERYTHING
                let statement = if !selector.is_empty()
                {
                    format!("SELECT DISTINCT key,kind,hops  FROM stars WHERE {}", where_clause )
                }
                else{

                    "SELECT DISTINCT key,kind,hops  FROM stars".to_string()
                };

                println!("STATEMENT {}",statement);

                let mut statement = self.conn.prepare(statement.as_str())?;
                let mut rows= statement.query( params_from_iter(params.iter() ) )?;

                let mut handles = vec![];
                while let Option::Some(row) = rows.next()?
                {
                    let key:Vec<u8> = row.get(0)?;
                    let key = StarKey::from_bin(key)?;

                    let kind:String = row.get(1)?;
                    let kind= StarKind::from_str(kind.as_str())?;

                    let hops = if let ValueRef::Null = row.get_ref(2)? {
                        Option::None
                    }
                    else {
                        let hops : usize = row.get(2)?;
                        Option::Some(hops)
                    };

                    let handle = StarHandle{
                        key: key,
                        kind: kind,
                        hops: hops
                    };

                    handles.push(handle);
                }
                Ok(StarHandleResult::StarHandles(handles))
            }
            StarHandleCommand::Next(selector) => {
                let mut params = vec![];
                let mut where_clause = String::new();
                let mut param_index = 0;

                for (index, field) in Vec::from_iter(selector.fields.clone()).iter().map(|x| x.clone() ).enumerate()
                {
                    if index != 0 {
                        where_clause.push_str(" AND ");
                    }

                    let f = match &field {
                        StarFieldSelection::Kind(kind) => {
println!("kind {}",kind);
                            format!("kind=?{}", index + 1)
                        }
                        StarFieldSelection::MinHops => {
                            format!("hops NOT NULL AND hops=MIN(hops)")
                        }
                    };

                    where_clause.push_str(f.as_str());
                    if field.is_param() {
                        params.push(field);
                        param_index = param_index+1;
                    }
                }


                // in case this search was for EVERYTHING
                let statement = if !selector.is_empty()
                {
                    format!("SELECT DISTINCT key,kind,hops  FROM stars WHERE {} ORDER BY selections", where_clause )
                }
                else{

                    "SELECT DISTINCT key,kind,hops  FROM stars ORDER BY selections".to_string()
                };



                println!("STATEMENT {}",statement);
                let trans = self.conn.transaction()?;

                let handle= trans.query_row( statement.as_str(), params_from_iter(params.iter() ), |row|
                    {
                        let key:Vec<u8> = row.get(0)?;
                        let key = StarKey::from_bin(key)?;

                        let kind:String = row.get(1)?;
                        let kind= StarKind::from_str(kind.as_str()).map_err(|_|rusqlite::Error::InvalidQuery)?;

                        let hops = if let ValueRef::Null = row.get_ref(2)? {
                            Option::None
                        }
                        else {
                            let hops : usize = row.get(2)?;
                            Option::Some(hops)
                        };

                        let handle = StarHandle{
                            key: key,
                            kind: kind,
                            hops: hops
                        };

                        Ok(handle)
                    }
                );

                let handle = match handle {
                    Ok(handle) => handle,
                    Err(err) => {
                        match err{
                            rusqlite::Error::QueryReturnedNoRows => {
                                return Err(Fail::SuitableHostNotAvailable(format!("could not select for: {}", selector.to_string() )));
                            }
                            _ => {
                                return Err(err.to_string().into());
                            }
                       };
                    }
                };

                trans.execute("UPDATE stars SET selections=selections+1 WHERE key=?1", params![handle.key.bin()?])?;

                trans.commit()?;

                Ok(StarHandleResult::StarHandle(handle))
            }

            StarHandleCommand::Satisfied(kinds) => {
                let mut lacking = HashSet::new();
                for kind in kinds {
                    if !self.conn.query_row("SELECT count(*) AS count FROM stars WHERE kind=?1", params![kind.to_string()], |row| {
                       let count:usize = row.get(0)?;
                       return Ok(count > 0);
                    })? {
                        lacking.insert(kind);
                    }
                }
                if lacking.is_empty() {
                    Ok(StarHandleResult::Satisfaction(Satisfaction::Ok))
                } else {
                    Ok(StarHandleResult::Satisfaction(Satisfaction::Lacking(lacking)))
                }

            }
        }

    }

    pub fn setup(&mut self)->Result<(),Error>
    {
        let stars= r#"
       CREATE TABLE IF NOT EXISTS stars(
	      key BLOB PRIMARY KEY,
	      kind TEXT NOT NULL,
	      hops INTEGER,
	      selections INTEGER NOT NULL DEFAULT 0
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(stars, [])?;
        transaction.commit();

        Ok(())
    }
}