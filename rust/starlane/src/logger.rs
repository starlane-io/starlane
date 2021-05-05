use tokio::sync::{broadcast, mpsc, RwLock};

use crate::frame::{StarSearch, StarSearchResult};
use crate::star::StarKey;
use std::sync::Arc;
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

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

#[derive(Clone,Serialize,Deserialize)]
pub struct Flags
{

}



#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub enum Flag
{
  Star(StarFlag)
}

#[derive(Clone,Hash,Eq,PartialEq,Serialize,Deserialize)]
pub enum StarFlag
{

}

#[derive(Clone,Serialize,Deserialize)]
pub enum Log
{
    Star(StarLog)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct StarLog
{
    star: StarKey
}
