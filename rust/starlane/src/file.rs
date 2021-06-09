use std::collections::HashMap;
use std::sync::{Arc, mpsc, Mutex};

use dyn_clone::DynClone;

use crate::error::Error;
use crate::resource::{Path, FileKind};
use std::fs::{File, DirBuilder};
use std::io::{Read, Write};
use std::convert::TryFrom;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvError};
use notify::{raw_watcher, Watcher, RecursiveMode, RawEvent, Op};
use std::thread;
use crate::star::Star;
use tokio::runtime::Runtime;
use std::future::Future;
use crate::util;

pub enum FileCommand{
    Read{path: Path, tx:tokio::sync::oneshot::Sender<Result<Arc<Vec<u8>>,Error>>},
    Write{ path: Path, data: Arc<Vec<u8>>, tx: tokio::sync::oneshot::Sender<Result<(),Error>> },
    MkDir{ path: Path,tx: tokio::sync::oneshot::Sender<Result<(),Error>> },
    Watch{tx: tokio::sync::oneshot::Sender<Result<tokio::sync::mpsc::Receiver<FileEvent>,Error>>}
}

#[derive(Clone)]
pub struct FileAccess{
    path: String,
    tx: tokio::sync::mpsc::Sender<FileCommand>
}

impl FileAccess{

    pub fn path(&self)->String{
        self.path.clone()
    }

    pub async fn new( path: String ) -> Result<Self,Error>{
        let tx = LocalFileAccess::new(path.clone()).await?;
        Ok(FileAccess{
            path: path,
            tx: tx
        })
    }

    pub async fn read( &self, path: &Path )->Result<Arc<Vec<u8>>,Error>{
        let (tx,rx) = tokio::sync::oneshot::channel();
        self.tx.send( FileCommand::Read{path: path.clone(),tx:tx}).await?;
        Ok(util::wait_for_it(rx).await?)
    }

    pub async fn write( &mut self, path: &Path, data: Arc<Vec<u8>> )->Result<(),Error> {
        let (tx,mut rx) = tokio::sync::oneshot::channel();
        self.tx.send( FileCommand::Write{path:path.clone(),data,tx}).await?;
        Ok(util::wait_for_it(rx).await?)
    }

    pub async fn with_path(&self, path: String ) -> Result<FileAccess,Error> {
        Ok(FileAccess::new( format!("{}/{}",self.path,path) ).await?)
    }

    pub async fn mkdir( &mut self, path: &Path ) -> Result<FileAccess,Error> {
        let (tx,rx) = tokio::sync::oneshot::channel();
        self.tx.send( FileCommand::MkDir{path:path.clone(),tx}).await?;
        util::wait_for_it(rx).await?;
        self.with_path(path.to_relative() ).await
    }

    pub async fn watch(&self) -> Result<tokio::sync::mpsc::Receiver<FileEvent>,Error> {
        let (tx,rx) = tokio::sync::oneshot::channel();
        self.tx.send( FileCommand::Watch{tx}).await?;
        Ok(util::wait_for_it(rx).await?)
    }
}

#[derive(Debug)]
pub enum FileEventKind{
    Create,
    Update,
    Delete
}

#[derive(Debug)]
pub struct FileEvent{
    pub path: String,
    pub event_kind: FileEventKind,
    pub file_kind: FileKind
}


#[derive(Clone)]
pub struct MemoryFileAccess {
    map: HashMap<Path,Arc<Vec<u8>>>
}

impl MemoryFileAccess {
    pub fn new( ) -> Self{
        MemoryFileAccess{
            map: HashMap::new()
        }
    }
}



pub struct LocalFileAccess{
    base_dir: String,
    rx: tokio::sync::mpsc::Receiver<FileCommand>
}

impl LocalFileAccess {
    pub async fn new( base_dir: String) -> Result<tokio::sync::mpsc::Sender<FileCommand>,Error>{

        let mut builder = DirBuilder::new();
        builder.recursive(true);
        builder.create(base_dir.clone() )?;

        let (tx,rx) = tokio::sync::mpsc::channel(128 );

        Self{
            base_dir: base_dir,
            rx: rx
        }.run().await;

        Ok(tx)
    }

    async fn run(mut self) {
        tokio::spawn( async move {
            while let Option::Some(command) = self.rx.recv().await {
                match self.process(command).await {
                    Ok(_) => {}
                    Err(error) => { eprintln!("Error in LocalFileAccess: {}", error) }
                }
            }
        });
    }

    async fn process( &mut self, command: FileCommand ) -> Result<(),Error>{
        match command {
            FileCommand::Read { path, tx } => {
                tx.send(self.read(&path));
            }
            FileCommand::Write { path: path, data, tx } => {
                tx.send(self.write(&path,data));
            }
            FileCommand::MkDir { path, tx } => {
                tx.send( self.mkdir(&path));
            }
            FileCommand::Watch { tx } => {
                tx.send(self.watch());
            }
        }
        Ok(())
    }

    pub fn cat_path(&self, path: &str) -> Result<String,Error> {
        if path.len() < 1 {
            return Err("path cannot be empty".into());
        }

        let mut path_str = path.to_string();
        if path_str.starts_with("/") {
            path_str.remove(0);
        }
        let mut path_buf = PathBuf::new();
        path_buf.push(self.base_dir.clone() );
        path_buf.push(path_str);
        let path = path_buf.as_path().clone();
        let path = path.to_str().ok_or("path error")?.to_string();

        Ok(path)
    }
}

impl LocalFileAccess {

    pub fn read(&self, path: &Path) -> Result<Arc<Vec<u8>>, Error> {
        let path = self.cat_path(path.to_relative().as_str())?;

        let mut buf = vec![];
        let mut file = File::open(&path)?;
        file.read_to_end(&mut buf)?;
        Ok(Arc::new(buf))
    }

    pub fn write(&mut self, path: &Path, data: Arc<Vec<u8>>) -> Result<(), Error> {
        if let Option::Some(parent) = path.parent(){
            self.mkdir(&parent)?;
        }

        let path = self.cat_path(path.to_relative().as_str() )?;
        let mut file = File::open(&path)?;
        file.write_all(data.as_slice())?;
        Ok(())
    }


    fn mkdir(&mut self, path: &Path) -> Result<(), Error> {
        let path = self.cat_path(path.to_relative().as_str())?;
        let mut builder = DirBuilder::new();
        builder.recursive(true);
        builder.create(path.clone() )?;
        Ok(())
    }

    fn watch(&mut self) -> Result<tokio::sync::mpsc::Receiver<FileEvent>, Error> {
println!("!#R@#$ STARTING FS WATCH LOOP FOR {}", self.base_dir );
        let (tx,rx) = mpsc::channel();
        let (event_tx,event_rx) = tokio::sync::mpsc::channel(128);


        let tokio_runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        let base_dir = self.base_dir.clone();

        thread::spawn( move || {

            let mut watcher = raw_watcher(tx).unwrap();
            watcher.watch( base_dir.clone(), RecursiveMode::Recursive ).unwrap();

            loop {
                match rx.recv() {
                    Ok(RawEvent { path: Some(path), op: Ok(op), cookie }) => {

let CREATE_WRITE = Op::CREATE | Op::WRITE;
println!("###########>>>>>>>>>> watch op: {:?}==CREATE_WRITE ? {}", op, op==CREATE_WRITE );
                        let event_kind = match op {
                            Op::CREATE => FileEventKind::Create,
                            CREATE_WRITE => FileEventKind::Create,
                            Op::REMOVE => FileEventKind::Delete,
                            Op::WRITE => FileEventKind::Update,
                            x => {
                                println!("x {:?}", x);
                                continue; }
                        };


                        let file_kind = match path.is_dir(){
                            true => FileKind::Directory,
                            false => FileKind::File
                        };

                        let event = FileEvent {
                            path: path.to_str().unwrap().to_string(),
                            event_kind: event_kind,
                            file_kind: file_kind
                        };
println!("event: {:?}", event);

                        let event_tx = event_tx.clone();
                        tokio_runtime.block_on(async move {
                            event_tx.send(event).await;
                        })
                    }
                    Ok(event) => { eprintln!("file_access broken event: {:?}", event); }
                    Err(error) => {
eprintln!("WATCH ERROR: {}", error);
                        break;
                    }
                }
            }
        });
        Ok(event_rx)
    }
}