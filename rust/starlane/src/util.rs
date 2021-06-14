use tokio::sync::mpsc::Receiver;
use crate::error::Error;
use tokio::sync::{mpsc, oneshot};
use std::collections::HashMap;
use std::hash::Hash;
use tokio::time::Duration;

pub struct Progress<E>
{
    rx: Receiver<E>
}

impl <E> Progress<E>
{
}


enum AsyncHashMapCommand<K,V> where K: Clone+Hash+Eq+PartialEq+Send+Sync+'static, V: Clone+Send+Sync+'static {
    Put {key:K,value:V},
    Get{
        key: K,
        tx: oneshot::Sender<Option<V>>
    },
    Remove{
        key: K,
        tx: oneshot::Sender<Option<V>>
    },
    Contains{
        key: K,
        tx: oneshot::Sender<bool>
    }
}

#[derive(Clone)]
pub struct AsyncHashMap<K,V> where K: Clone+Hash+Eq+PartialEq+Send+Sync+'static, V: Clone+Send+Sync+'static {
    tx: mpsc::Sender<AsyncHashMapCommand<K,V>>
}

impl <K,V> AsyncHashMap<K,V> where K: Clone+Hash+Eq+PartialEq+Send+Sync+'static, V: Clone+Send+Sync+'static {
    pub async fn new() -> Self {
        let (tx,mut rx):(mpsc::Sender<AsyncHashMapCommand<K,V>>,mpsc::Receiver<AsyncHashMapCommand<K,V>>) = mpsc::channel(1);

        tokio::spawn( async move {
            let mut map = HashMap::new();
            while let Option::Some(command) = rx.recv().await{
                match command{
                    AsyncHashMapCommand::Put { key, value } => {
                        map.insert(key,value);
                    }
                    AsyncHashMapCommand::Get { key, tx } => {
                        let opt = map.get(&key).cloned();
                        tx.send(opt).unwrap_or_default();
                    }
                    AsyncHashMapCommand::Remove{ key, tx} => {
                        let opt = map.remove(&key).clone();
                        tx.send(opt).unwrap_or_default();
                    }
                    AsyncHashMapCommand::Contains{key,tx} => {
                        tx.send(map.contains_key(&key) ).unwrap_or_default();
                    }
                }
            }
        });

        AsyncHashMap{
            tx: tx
        }
    }

    pub async fn put( &self, key:K, value:V )->Result<(),Error>{
        self.tx.send( AsyncHashMapCommand::Put { key, value}).await?;
        Ok(())
    }

    pub async fn get( &self, key:K )->Result<Option<V>,Error>{
        let (tx,rx) = oneshot::channel();
        self.tx.send( AsyncHashMapCommand::Get{ key, tx }).await?;
        Ok(rx.await?)
    }

    pub async fn remove( &self, key:K )->Result<Option<V>,Error>{
        let (tx,rx) = oneshot::channel();
        self.tx.send( AsyncHashMapCommand::Remove{ key, tx }).await?;
        Ok(rx.await?)
    }

    pub async fn contains( &self, key:K )->Result<bool,Error>{
        let (tx,rx) = oneshot::channel();
        self.tx.send( AsyncHashMapCommand::Contains{ key, tx }).await?;
        Ok(rx.await?)
    }

}

pub async fn wait_for_it<R>( rx: oneshot::Receiver<Result<R,Error>>) -> Result<R,Error> {
    tokio::time::timeout( Duration::from_secs(15), rx).await??
}

pub async fn wait_for_it_whatever<R>( rx: oneshot::Receiver<R>) -> Result<R,Error> {
    Ok(tokio::time::timeout( Duration::from_secs(15), rx).await??)
}

pub async fn wait_for_it_for<R>( rx: oneshot::Receiver<Result<R,Error>>, duration: Duration ) -> Result<R,Error> {
    tokio::time::timeout( duration, rx).await??
}
