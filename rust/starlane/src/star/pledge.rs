use rusqlite::{Connection, params_from_iter, params, ToSql};
use tokio::sync::{mpsc, oneshot};
use crate::star::{StarInfo, StarKey, StarKind};
use crate::error::Error;
use std::collections::HashSet;
use crate::message::Fail;
use std::iter::FromIterator;
use std::str::FromStr;
use rusqlite::types::{ValueRef, ToSqlOutput, Value};
use tokio::time::Duration;

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



pub struct StarHandle {
    pub key: StarKey,
    pub kind: StarKind,
    pub hops: Option<usize>
}

pub struct StarSelector {
    fields: HashSet<StarFieldSelection>
}

impl StarSelector {
    pub fn is_empty(&self)->bool {
        self.fields.is_empty()
    }
}

#[derive(Clone,Hash,Eq,PartialEq)]
pub enum StarFieldSelection
{
    Kind(StarKind),
    MinHops
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
    Satisfied(HashSet<StarKind>)
}

pub enum StarHandleResult
{
   Ok,
   StarHandles(Vec<StarHandle>),
   Error(String),
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
                Err(err) => {
                    eprintln!("{}", err);
                    request.tx.send(StarHandleResult::Error(err.to_string()));
                }
            }
        }
        Ok(())
    }

    async fn process(&mut self, command: StarHandleCommand) -> Result<StarHandleResult,Error>
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
                    format!("SELECT DISTINCT star.key,star.kind,star.hops  FROM stars WHERE {}", where_clause )
                }
                else{

                    "SELECT DISTINCT star.key,star.kind,star.hops  FROM stars".to_string()
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
	      hops INTEGER
        )"#;

        let transaction = self.conn.transaction()?;
        transaction.execute(stars, [])?;
        transaction.commit();

        Ok(())
    }
}