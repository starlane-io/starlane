use tokio::sync::{broadcast, mpsc};

use crate::frame::{WindUp, WindDown};
use crate::star::{StarKey, StarKind, StarInfo};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::collections::{HashSet, HashMap};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Logger
{
   tx: broadcast::Sender<Log>,
}

impl Logger
{
    pub fn new() -> Self
    {
        let (tx,_) = broadcast::channel(16*1024 );
        Logger {
            tx: tx,
        }
    }

    pub fn rx(&self)->broadcast::Receiver<Log>
    {
        self.tx.subscribe()
    }

    pub fn log( &mut self, log: Log)
    {
        self.tx.send(log);
    }
}

pub struct LogAggregate
{
   logs: Arc<RwLock<Vec<Log>>>
}

impl LogAggregate
{
    pub fn new() -> Self
    {
        LogAggregate {
            logs: Arc::new(RwLock::new(vec![] ))
        }
    }

    pub async fn watch( &self, logger: Logger )
    {
        let logs = self.logs.clone();
        let mut rx = logger.rx();
        tokio::spawn( async move {
            while let Ok(log) = rx.recv().await {
                let lock = logs.write();
                match lock
                {
                    Ok(mut logs) => {
                        logs.push(log);
                    }
                    Err(error) => {
                        println!("LogAggregate: {}",error);
                    }
                }
            }
        } );
    }

    pub fn append(&mut self, log: Log)
    {
        let lock = self.logs.write();
        match lock
        {
            Ok(mut logs) => {
                logs.push(log);
            }
            Err(error) => {
                println!("LogAggregate: {}",error);
            }
        }
    }

    pub fn clear(&mut self)
    {
        let lock = self.logs.write();
        match lock
        {
            Ok(mut logs) => {
                logs.clear();
            }
            Err(error) => {
                println!("LogAggregate: {}",error);
            }
        }
    }


    pub fn count<P>(&self, predicate: P ) -> usize where P: FnMut(&&Log) -> bool
    {
        let lock = self.logs.read();
        match lock
        {
            Ok(logs) => {
                logs.iter().filter(predicate).count()
            }
            Err(error) => {
                println!("LogAggregate: {}",error);
                0
            }
        }
    }


}



#[derive(Clone,Serialize,Deserialize)]
pub struct Flags
{
    map: HashMap<Flag,bool>
}

impl Flags
{
    pub fn new()->Self
    {
        Flags{
            map: HashMap::new()
        }
    }

    pub fn on( &mut self, flag: Flag )
    {
        self.map.insert(flag,true );
    }

    pub fn off( &mut self, flag: Flag )
    {
        self.map.insert(flag,false );
    }

    pub fn check( &self, flag: Flag ) -> bool
    {
        if !self.map.contains_key(&flag)
        {
            return false;
        }

        self.map.get(&flag ).unwrap().clone()
    }
}



#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub enum Flag
{
  Star(StarFlag)
}

#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub enum StarFlag
{
    DiagnoseSequence,
    DiagnosePledge,
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum Log
{
    ProtoStar(ProtoStarLog),
    Star(StarLog)
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct ProtoStarLog
{
    kind: StarKind,
    payload: ProtoStarLogPayload
}

impl ProtoStarLog
{
    pub fn new( kind: StarKind, payload: ProtoStarLogPayload ) -> Self {
        ProtoStarLog{
            kind: kind,
            payload: payload
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum ProtoStarLogPayload
{
    SequenceRequest,
    SequenceReplyRecv
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub struct StarLog
{
    pub star: StarKey,
    pub kind: StarKind,
    pub payload: StarLogPayload
}

impl StarLog
{
    pub fn new(info: &StarInfo, payload: StarLogPayload  ) -> Self {
        StarLog{
            star: info.star.clone(),
            kind: info.kind.clone(),
            payload: payload
        }
    }
}

#[derive(Clone,Serialize,Deserialize,Eq,PartialEq)]
pub enum StarLogPayload
{
   PledgeSent,
   PledgeRecv,
   PledgeOkRecv
}
