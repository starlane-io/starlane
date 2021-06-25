use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};

use dyn_clone::DynClone;

use crate::error::Error;
use crate::resource::{FileKind, Path};
use crate::star::Star;
use crate::util;
use notify::{raw_watcher, Op, RawEvent, RecursiveMode, Watcher};
use std::convert::TryFrom;
use std::fs::{DirBuilder, File};
use std::future::Future;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvError};
use std::{fs, thread};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::time::Duration;
use walkdir::{DirEntry, WalkDir};

pub enum FileCommand {
    Read {
        path: Path,
        tx: tokio::sync::oneshot::Sender<Result<Arc<Vec<u8>>, Error>>,
    },
    Write {
        path: Path,
        data: Arc<Vec<u8>>,
        tx: tokio::sync::oneshot::Sender<Result<(), Error>>,
    },
    //    WriteStream{ path: Path, stream: Box<dyn AsyncRead>, tx: tokio::sync::oneshot::Sender<Result<(),Error>> },
    MkDir {
        path: Path,
        tx: tokio::sync::oneshot::Sender<Result<(), Error>>,
    },
    Watch {
        tx: tokio::sync::oneshot::Sender<Result<tokio::sync::mpsc::Receiver<FileEvent>, Error>>,
    },
    Walk {
        tx: tokio::sync::oneshot::Sender<Result<tokio::sync::mpsc::Receiver<FileEvent>, Error>>,
    },
    UnZip {
        source: String,
        target: String,
        tx: tokio::sync::oneshot::Sender<Result<(), Error>>,
    },
}

#[derive(Clone)]
pub struct FileAccess {
    path: String,
    tx: tokio::sync::mpsc::Sender<FileCommand>,
}

impl FileAccess {
    pub fn path(&self) -> String {
        self.path.clone()
    }

    pub fn new(path: String) -> Result<Self, Error> {
        let tx = LocalFileAccess::new(path.clone())?;
        let path = fs::canonicalize(&path)?
            .to_str()
            .ok_or("turning path to string")?
            .to_string();
        Ok(FileAccess { path: path, tx: tx })
    }

    pub async fn read(&self, path: &Path) -> Result<Arc<Vec<u8>>, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(FileCommand::Read {
                path: path.clone(),
                tx: tx,
            })
            .await?;
        Ok(util::wait_for_it(rx).await?)
    }

    pub async fn write(&mut self, path: &Path, data: Arc<Vec<u8>>) -> Result<(), Error> {
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(FileCommand::Write {
                path: path.clone(),
                data,
                tx,
            })
            .await?;
        Ok(util::wait_for_it(rx).await?)
    }

    /*
    pub async fn write_stream( &mut self, path: &Path, stream: Box<dyn AsyncReadExt> )->Result<(),Error> {
        let (tx,mut rx) = tokio::sync::oneshot::channel();
        self.tx.send( FileCommand::WriteStream{path:path.clone(),stream,tx}).await?;
        Ok(util::wait_for_it_for(rx, Duration::from_secs(60*15)).await?)
    }

     */

    pub fn with_path(&self, path: String) -> Result<FileAccess, Error> {
        let path = format!("{}/{}", self.path, path);
        Ok(FileAccess::new(path)?)
    }

    pub async fn unzip(&self, source: String, target: String) -> Result<(), Error> {
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(FileCommand::UnZip { source, target, tx })
            .await?;
        Ok(util::wait_for_it_for(rx, Duration::from_secs(60 * 2)).await?)
    }

    pub async fn mkdir(&mut self, path: &Path) -> Result<FileAccess, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(FileCommand::MkDir {
                path: path.clone(),
                tx,
            })
            .await?;
        util::wait_for_it(rx).await?;
        self.with_path(path.to_relative())
    }

    pub async fn watch(&self) -> Result<tokio::sync::mpsc::Receiver<FileEvent>, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(FileCommand::Watch { tx }).await?;
        Ok(util::wait_for_it(rx).await?)
    }

    pub async fn walk(&self) -> Result<tokio::sync::mpsc::Receiver<FileEvent>, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(FileCommand::Walk { tx }).await?;
        Ok(util::wait_for_it(rx).await?)
    }
}

#[derive(Debug, Clone)]
pub enum FileEventKind {
    Discovered,
    Create,
    Update,
    Delete,
}

#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: String,
    pub event_kind: FileEventKind,
    pub file_kind: FileKind,
}

#[derive(Clone)]
pub struct MemoryFileAccess {
    map: HashMap<Path, Arc<Vec<u8>>>,
}

impl MemoryFileAccess {
    pub fn new() -> Self {
        MemoryFileAccess {
            map: HashMap::new(),
        }
    }
}

pub struct LocalFileAccess {
    base_dir: String,
    rx: tokio::sync::mpsc::Receiver<FileCommand>,
}

impl LocalFileAccess {
    pub fn new(base_dir: String) -> Result<tokio::sync::mpsc::Sender<FileCommand>, Error> {
        let mut builder = DirBuilder::new();
        builder.recursive(true);
        builder.create(base_dir.clone())?;

        let (tx, rx) = tokio::sync::mpsc::channel(128);

        tokio::spawn(async move {
            Self {
                base_dir: base_dir,
                rx: rx,
            }
            .run()
            .await;
        });

        Ok(tx)
    }

    async fn run(mut self) {
        tokio::spawn(async move {
            while let Option::Some(command) = self.rx.recv().await {
                match self.process(command).await {
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!("Error in LocalFileAccess: {}", error)
                    }
                }
            }
        });
    }

    async fn process(&mut self, command: FileCommand) -> Result<(), Error> {
        match command {
            FileCommand::Read { path, tx } => {
                tx.send(self.read(&path));
            }
            FileCommand::Write {
                path: path,
                data,
                tx,
            } => {
                tx.send(self.write(&path, data));
            }
            /*            FileCommand::WriteStream { path: path, stream, tx } => {
                tx.send(self.write_sream(&path,stream).await);
            }*/
            FileCommand::MkDir { path, tx } => {
                tx.send(self.mkdir(&path));
            }
            FileCommand::Watch { tx } => {
                tx.send(self.watch());
            }
            FileCommand::Walk { tx } => {
                tx.send(self.walk());
            }
            FileCommand::UnZip { source, target, tx } => {
                tx.send(self.unzip(source, target));
            }
        }
        Ok(())
    }

    pub fn cat_path(&self, path: &str) -> Result<String, Error> {
        if path.len() < 1 {
            return Err("path cannot be empty".into());
        }

        let mut path_str = path.to_string();
        if path_str.starts_with("/") {
            path_str.remove(0);
        }
        let mut path_buf = PathBuf::new();
        path_buf.push(self.base_dir.clone());
        path_buf.push(path_str);
        let path = path_buf.as_path().clone();
        let path = path.to_str().ok_or("path error")?.to_string();

        Ok(path)
    }
}

impl LocalFileAccess {
    pub fn unzip(&mut self, source: String, target: String) -> Result<(), Error> {
        let source = format!("{}/{}", self.base_dir, source);
        let source = File::open(source)?;
        let mut archive = zip::ZipArchive::new(source)?;

        for i in 0..archive.len() {
            let mut zip_file = archive.by_index(i)?;
            if zip_file.is_dir() {
                let path = Path::new(format!("/{}/{}", target, zip_file.name()).as_str())?;
                self.mkdir(&path)?;
            } else {
                let path = format!("{}/{}/{}", self.base_dir, target, zip_file.name());
                let mut file = fs::File::create(path)?;
                std::io::copy(&mut zip_file, &mut file)?;
            }
        }

        Ok(())
    }

    pub fn read(&self, path: &Path) -> Result<Arc<Vec<u8>>, Error> {
        let path = self.cat_path(path.to_relative().as_str())?;

        let mut buf = vec![];
        let mut file = File::open(&path)?;
        file.read_to_end(&mut buf)?;
        Ok(Arc::new(buf))
    }

    pub fn write(&mut self, path: &Path, data: Arc<Vec<u8>>) -> Result<(), Error> {
        if let Option::Some(parent) = path.parent() {
            self.mkdir(&parent)?;
        }

        let path = self.cat_path(path.to_relative().as_str())?;
        let mut file = File::create(&path)?;
        file.write(data.as_slice()).unwrap();
        Ok(())
    }

    /*
    pub async fn write_stream(&mut self, path: &Path, mut stream: Box<dyn AsyncReadExt>) -> Result<(), Error> {

        if let Option::Some(parent) = path.parent(){
            self.mkdir(&parent)?;
        }

        let path = self.cat_path(path.to_relative().as_str() )?;
        let mut file = tokio::fs::File::create(&path).await?;

        tokio::io::copy(&mut stream,&mut file ).await?;

        Ok(())
    }
     */

    fn mkdir(&mut self, path: &Path) -> Result<(), Error> {
        let path = self.cat_path(path.to_relative().as_str())?;
        let mut builder = DirBuilder::new();
        builder.recursive(true);
        builder.create(path.clone())?;
        Ok(())
    }

    fn walk(&mut self) -> Result<tokio::sync::mpsc::Receiver<FileEvent>, Error> {
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(128);
        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let base_dir = self.base_dir.clone();

        thread::spawn(move || {
            for entry in WalkDir::new(base_dir.clone()) {
                match entry {
                    Ok(dir_entry) => match dir_entry.path().to_str() {
                        Some(path) => {
                            let event = FileEvent {
                                path: path.to_string(),
                                event_kind: FileEventKind::Discovered,
                                file_kind: FileKind::Directory,
                            };
                            let event_tx = event_tx.clone();
                            tokio_runtime.block_on(async move {
                                event_tx.send(event).await;
                            })
                        }
                        None => {
                            return;
                        }
                    },
                    Err(error) => {
                        eprintln!("Error when walking filesystem: {}", error);
                    }
                }
            }
        });

        Ok(event_rx)
    }

    fn watch(&mut self) -> Result<tokio::sync::mpsc::Receiver<FileEvent>, Error> {
        let (tx, rx) = mpsc::channel();
        let (event_tx, event_rx) = tokio::sync::mpsc::channel(128);

        let tokio_runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let base_dir = self.base_dir.clone();

        thread::spawn(move || {
            let mut watcher = raw_watcher(tx).unwrap();
            watcher
                .watch(base_dir.clone(), RecursiveMode::Recursive)
                .unwrap();

            loop {
                match rx.recv() {
                    Ok(RawEvent {
                        path: Some(path),
                        op: Ok(op),
                        cookie,
                    }) => {
                        let event_kind = match op {
                            Op::CREATE => FileEventKind::Create,
                            CREATE_WRITE => FileEventKind::Create,
                            Op::REMOVE => FileEventKind::Delete,
                            Op::WRITE => FileEventKind::Update,
                            x => {
                                continue;
                            }
                        };

                        let file_kind = match path.is_dir() {
                            true => FileKind::Directory,
                            false => FileKind::File,
                        };

                        let event = FileEvent {
                            path: path.to_str().unwrap().to_string(),
                            event_kind: event_kind,
                            file_kind: file_kind,
                        };

                        let event_tx = event_tx.clone();
                        tokio_runtime.block_on(async move {
                            event_tx.send(event).await;
                        })
                    }
                    Ok(event) => {
                        eprintln!("file_access broken event: {:?}", event);
                    }
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
