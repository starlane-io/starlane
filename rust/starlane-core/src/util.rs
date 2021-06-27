use std::cmp::Ordering;
use std::collections::HashMap;
use std::future::Future;
use std::hash::Hash;

use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::Duration;

use crate::error::Error;

pub struct Progress<E> {
    rx: Receiver<E>,
}

impl<E> Progress<E> {}

enum AsyncHashMapCommand<K, V>
where
    K: Clone + Hash + Eq + PartialEq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    Put {
        key: K,
        value: V,
    },
    Get {
        key: K,
        tx: oneshot::Sender<Option<V>>,
    },
    Remove {
        key: K,
        tx: oneshot::Sender<Option<V>>,
    },
    Contains {
        key: K,
        tx: oneshot::Sender<bool>,
    },
    GetMap(oneshot::Sender<HashMap<K, V>>),
    SetMap(HashMap<K, V>),
}

#[derive(Clone)]
pub struct AsyncHashMap<K, V>
where
    K: Clone + Hash + Eq + PartialEq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    tx: mpsc::Sender<AsyncHashMapCommand<K, V>>,
}

impl<K, V> AsyncHashMap<K, V>
where
    K: Clone + Hash + Eq + PartialEq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        let (tx, mut rx): (
            mpsc::Sender<AsyncHashMapCommand<K, V>>,
            mpsc::Receiver<AsyncHashMapCommand<K, V>>,
        ) = mpsc::channel(1);

        tokio::spawn(async move {
            let mut map = HashMap::new();
            while let Option::Some(command) = rx.recv().await {
                match command {
                    AsyncHashMapCommand::Put { key, value } => {
                        map.insert(key, value);
                    }
                    AsyncHashMapCommand::Get { key, tx } => {
                        let opt = map.get(&key).cloned();
                        tx.send(opt).unwrap_or_default();
                    }
                    AsyncHashMapCommand::Remove { key, tx } => {
                        let opt = map.remove(&key).clone();
                        tx.send(opt).unwrap_or_default();
                    }
                    AsyncHashMapCommand::Contains { key, tx } => {
                        tx.send(map.contains_key(&key)).unwrap_or_default();
                    }
                    AsyncHashMapCommand::GetMap(tx) => {
                        tx.send(map.clone());
                    }
                    AsyncHashMapCommand::SetMap(new_map) => map = new_map,
                }
            }
        });

        AsyncHashMap { tx: tx }
    }

    pub async fn put(&self, key: K, value: V) -> Result<(), Error> {
        self.tx
            .send(AsyncHashMapCommand::Put { key, value })
            .await?;
        Ok(())
    }

    pub async fn get(&self, key: K) -> Result<Option<V>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(AsyncHashMapCommand::Get { key, tx }).await?;
        Ok(rx.await?)
    }

    pub async fn remove(&self, key: K) -> Result<Option<V>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(AsyncHashMapCommand::Remove { key, tx })
            .await?;
        Ok(rx.await?)
    }

    pub async fn contains(&self, key: K) -> Result<bool, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(AsyncHashMapCommand::Contains { key, tx })
            .await?;
        Ok(rx.await?)
    }

    pub async fn into_map(self) -> Result<HashMap<K, V>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(AsyncHashMapCommand::GetMap(tx)).await?;
        Ok(rx.await?)
    }

    pub fn set_map(&self, map: HashMap<K, V>) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(AsyncHashMapCommand::SetMap(map)).await;
        });
    }
}

impl<K, V> From<HashMap<K, V>> for AsyncHashMap<K, V>
where
    K: Clone + Hash + Eq + PartialEq + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn from(map: HashMap<K, V>) -> Self {
        let async_map = AsyncHashMap::new();
        async_map.set_map(map);
        async_map
    }
}

pub async fn wait_for_it<R>(rx: oneshot::Receiver<Result<R, Error>>) -> Result<R, Error> {
    tokio::time::timeout(Duration::from_secs(15), rx).await??
}

pub async fn wait_for_it_whatever<R>(rx: oneshot::Receiver<R>) -> Result<R, Error> {
    Ok(tokio::time::timeout(Duration::from_secs(26), rx).await??)
}

pub async fn wait_for_it_for<R>(
    rx: oneshot::Receiver<Result<R, Error>>,
    duration: Duration,
) -> Result<R, Error> {
    tokio::time::timeout(duration, rx).await??
}

#[async_trait]
pub trait AsyncProcessor<C>: Send + Sync + 'static {
    async fn process(&mut self, call: C);
}

pub trait Call: Sync + Send + 'static {}

pub struct AsyncRunner<C: Call> {
    tx: mpsc::Sender<C>,
    rx: mpsc::Receiver<C>,
    processor: Box<dyn AsyncProcessor<C>>,
}

impl<C: Call> AsyncRunner<C> {
    pub fn new(processor: Box<dyn AsyncProcessor<C>>, tx: mpsc::Sender<C>, rx: mpsc::Receiver<C>) {
        tokio::spawn(async move {
            AsyncRunner {
                tx: tx,
                rx: rx,
                processor: processor,
            }
            .run()
            .await;
        });
    }

    async fn run(mut self) {
        while let Option::Some(call) = self.rx.recv().await {
            self.processor.process(call).await;
        }
        println!("ASync Runner terminated");
    }
}

pub fn sort<T:Ord+PartialOrd+ToString>(a: T, b: T) -> Result<(T, T), Error> {
    if a == b {
        Err(format!(
            "both items are equal. {}=={}",
            a.to_string(),
            b.to_string()
        )
            .into())
    } else if a.cmp(&b) == Ordering::Greater {
        Ok((a, b))
    } else {
        Ok((b, a))
    }
}
