use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;

use std::hash::Hash;
use std::io::{Read, Seek, Write};
use std::path::Path;
use std::thread;

use tokio::sync::broadcast;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, oneshot};

use tokio::time::Duration;

use walkdir::{DirEntry, WalkDir};
use zip::result::ZipError;
use zip::write::FileOptions;

use crate::error::Error;

lazy_static! {
    pub static ref SHUTDOWN_TX: broadcast::Sender<()> = { broadcast::channel(1).0 };
}

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
    Clear,
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
                    AsyncHashMapCommand::Clear => {
                        map.clear();
                    }
                }
            }
        });

        AsyncHashMap { tx: tx }
    }

    pub fn clear(&self) -> Result<(), Error> {
        self.tx.try_send(AsyncHashMapCommand::Clear)?;
        Ok(())
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
    match tokio::time::timeout(Duration::from_secs(4), rx).await {
        Ok(result) => match result {
            Ok(result) => match result {
                Ok(result) => Ok(result),
                Err(error) => Err(error.into())
            },
            Err(error) => Err(error.into())
        },
        Err(_err) => Err("timeout".into())
    }
}

pub async fn wait_for_it_whatever<R>(rx: oneshot::Receiver<R>) -> Result<R, Error> {
    match tokio::time::timeout(Duration::from_secs(4), rx).await {
        Ok(result) => match result {
            Ok(result) => Ok(result),
            Err(error) => log_err(error),
        },
        Err(_err) => log_err("timeout"),
    }
}

pub async fn wait_for_it_for<R>(
    rx: oneshot::Receiver<Result<R, Error>>,
    duration: Duration,
) -> Result<R, Error> {
    match tokio::time::timeout(duration, rx).await {
        Ok(result) => match result {
            Ok(result) => match result {
                Ok(result) => Ok(result),
                Err(error) => log_err(error),
            },
            Err(error) => log_err(error),
        },
        Err(_err) => log_err("timeout"),
    }
}

#[async_trait]
pub trait AsyncProcessor<C>: Send + Sync + 'static {
    async fn init(&mut self) {

    }
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

pub fn sort<T: Ord + PartialOrd + ToString>(a: T, b: T) -> Result<(T, T), Error> {
    if a == b {
        Err(format!("both items are equal. {}=={}", a.to_string(), b.to_string()).into())
    } else if a.cmp(&b) == Ordering::Greater {
        Ok((a, b))
    } else {
        Ok((b, a))
    }
}

pub fn log_err<E: ToString, OK, O: From<E>>(err: E) -> Result<OK, O> {
    error!("{}", err.to_string());
    Err(err.into())
}

fn zip_dir<T>(
    it: &mut dyn Iterator<Item = DirEntry>,
    prefix: &str,
    writer: T,
    method: zip::CompressionMethod,
) -> zip::result::ZipResult<()>
where
    T: Write + Seek,
{
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(method)
        .unix_permissions(0o755);

    let mut buffer = Vec::new();
    for entry in it {
        let path = entry.path();
        let name = path.strip_prefix(Path::new(prefix)).unwrap();

        // Write file or directory explicitly
        // Some unzip tools unzip files with directory paths correctly, some do not!
        if path.is_file() {
            //            println!("adding file {:?} as {:?} ...", path, name);
            #[allow(deprecated)]
            zip.start_file_from_path(name, options)?;
            let mut f = File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&*buffer)?;
            buffer.clear();
        } else if name.as_os_str().len() != 0 {
            // Only if not root! Avoids path spec / warning
            // and mapname conversion failed error on unzip
            println!("adding dir {:?} as {:?} ...", path, name);
            #[allow(deprecated)]
            zip.add_directory_from_path(name, options)?;
        }
    }
    zip.finish()?;
    Result::Ok(())
}

pub fn zip(
    src_dir: &str,
    dst_file: &File,
    method: zip::CompressionMethod,
) -> zip::result::ZipResult<()> {
    if !Path::new(src_dir).is_dir() {
        return Err(ZipError::FileNotFound);
    }

    /*    let path = Path::new(dst_file);
       let file = File::create(&path).unwrap();

    */

    let walkdir = WalkDir::new(src_dir.to_string());
    let it = walkdir.into_iter();

    zip_dir(&mut it.filter_map(|e| e.ok()), src_dir, dst_file, method)?;

    Ok(())
}

pub fn shutdown() {
    SHUTDOWN_TX.send(());
    thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(100));
        std::process::exit(0);
    });
}


#[derive(Clone)]
pub struct ServiceChamber<S> where S: Clone{
    name: String,
    service: Option<S>
}

impl <S> ServiceChamber<S> where S: Clone{
    pub fn new( name: &str ) -> Self {
        let name = name.to_string();
        Self {
            name,
            service: None
        }
    }

    pub fn set( &mut self, service: S ) {
        self.service = Option::Some(service);
    }

    pub fn get( &self ) -> Result<S,Error> {
        match &self.service {
            None => {
                Err(format!("Service Unavalable: {}", self.name).into())
            }
            Some(service) => {
                Ok(service.clone())
            }
        }
    }
}
