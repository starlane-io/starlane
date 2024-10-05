use crate::err::HypErr;
use crate::executor::cli::os::CliOsExecutor;
use crate::executor::cli::{CliExecutor, CliIn, CliOut};
use crate::executor::Executor;
use clap::{Parser, Subcommand};
use itertools::Itertools;
use path_clean::PathClean;
use starlane::space::loc::ToPoint;
use starlane::space::path::Path;
use starlane::space::point::Point;
use starlane::space::substance::Bin;
use std::io::BufRead;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use strum_macros::EnumString;
/*
impl <E> From<Box<E>> for FileStore
where E: Executor<In=CliIn,Out=CliOut> {
    fn from(exec: Box<E>) -> Self {
        FileStore::Cli(exec)
    }
}
 */

pub const FILE_STORE_ROOT: &'static str = "FILE_STORE_ROOT";

#[derive(Clone)]
pub struct FileStoreApi {
    path: PathBuf,
    filestore: Arc<FileStore>,
}

impl FileStoreApi {
    pub fn new(path: PathBuf, filestore: Arc<FileStore>) -> Self {
        Self { path, filestore }
    }

    pub fn sub_root(&self, sub_root: PathBuf) -> Result<FileStoreApi, HypErr> {
        let root = RootDir::new(self.path.clone());
        let path = root.norm(&sub_root)?;

        Ok(FileStoreApi {
            path,
            filestore: self.filestore.clone(),
        })
    }

    pub async fn init(&self) -> Result<(), HypErr> {
        self.filestore.execute(FileStoreIn::Init).await?;
        Ok(())
    }
}

impl From<Box<CliOsExecutor>> for FileStore {
    fn from(value: Box<CliOsExecutor>) -> Self {
        FileStore::Cli(value)
    }
}

pub enum FileStore {
    Cli(Box<dyn Executor<In = CliIn, Out = CliOut>+Send+Sync>),
}

impl FileStore {
    pub async fn sub_root(&self, sub_root: PathBuf) -> Result<FileStore, HypErr> {
        match self {
            FileStore::Cli(cli) => {
                let conf = cli.conf();
                let value = conf.env(FILE_STORE_ROOT).ok_or(HypErr::String(format!("expected environment variable '{}' to be set", FILE_STORE_ROOT).to_string()))?;
                let path = PathBuf::from(value);
                let root = RootDir::new(path);
                let root = root.norm(&sub_root)?;

                let conf = cli.conf().with_env(FILE_STORE_ROOT, &root.to_str().unwrap());
                Ok(conf.create()?)
            }
        }
    }

    pub async fn execute(&self, mut input: FileStoreIn) -> Result<FileStoreOut, HypErr> {
        match self {
            FileStore::Cli(executor) => {
                let kind: FileStoreInKind = (&input).into();
                let mut input = input.into();
                let mut out = executor.execute(input).await?;
                let rtn = match kind {
                    FileStoreInKind::Init => FileStoreOut::Init,
                    FileStoreInKind::Write { .. } => FileStoreOut::Write,
                    FileStoreInKind::Read { .. } => {
                        out.close_stdin()?;
                        let stdout = out.stdout().await?;
                        FileStoreOut::Read(stdout)
                    }
                    FileStoreInKind::Mkdir { .. } => FileStoreOut::Mkdir,
                    FileStoreInKind::Remove { .. } => FileStoreOut::Remove,
                    FileStoreInKind::List { .. } => {
                        out.close_stdin()?;
                        let stdout = out.stdout().await?;
                        let paths = stdout
                            .lines()
                            .into_iter()
                            .map(|line| line.unwrap().into())
                            .collect::<Vec<PathBuf>>();

                        FileStoreOut::List(paths)
                    }
                    FileStoreInKind::Exists { .. } => FileStoreOut::Exists(true),
                    FileStoreInKind::Pwd => {
                        let stdout = out.stdout().await?;
                        let line: PathBuf = stdout
                            .lines()
                            .into_iter()
                            .map_ok(|line| line.into())
                            .find_or_first(|_| true)
                            .ok_or(HypErr::String("pwd didn't return anything".to_string()))??;
                        FileStoreOut::Pwd(line)
                    }
                };
                out.close_stdin();
                Ok(rtn)
            }
        }
    }

}

pub type FileStoreIn = FileStoreInDef<PathBuf, Bin>;
pub type FileStoreInKind = FileStoreInDef<(), ()>;

impl Into<FileStoreInKind> for &FileStoreIn {
    fn into(self) -> FileStoreInKind {
        match self {
            FileStoreIn::Init => FileStoreInKind::Init,
            FileStoreIn::Write { .. } => FileStoreInKind::Write {
                path: (),
                state: (),
            },
            FileStoreIn::Read { .. } => FileStoreInKind::Read { path: () },
            FileStoreIn::Mkdir { .. } => FileStoreInKind::Mkdir { path: () },
            FileStoreIn::Remove { .. } => FileStoreInKind::Remove { path: () },
            FileStoreIn::List { .. } => FileStoreInKind::List { path: () },
            FileStoreIn::Exists { .. } => FileStoreInKind::Exists { path: () },
            FileStoreIn::Pwd => FileStoreInDef::Pwd,
        }
    }
}

pub enum FileStoreInDef<P, S> {
    Init,
    Write { path: P, state: S },
    Read { path: P },
    Mkdir { path: P },
    Remove { path: P },
    List { path: P },
    Exists { path: P },
    Pwd,
}

impl Into<CliIn> for FileStoreIn {
    fn into(self) -> CliIn {
        match self {
            FileStoreIn::Init => CliIn::args(vec!["init"]),
            FileStoreIn::Write { path, state } => {
                CliIn::str_stdin(vec!["write".to_string(), to_str(&path)], state)
            }
            FileStoreIn::Read { path } => CliIn::str_args(vec!["read".to_string(), to_str(&path)]),
            FileStoreIn::Mkdir { path } => {
                CliIn::str_args(vec!["mkdir".to_string(), to_str(&path)])
            }
            FileStoreIn::Remove { path } => {
                CliIn::str_args(vec!["remove".to_string(), to_str(&path)])
            }
            FileStoreIn::List { path } => CliIn::str_args(vec!["list".to_string(), to_str(&path)]),
            FileStoreIn::Exists { path } => {
                CliIn::str_args(vec!["exists".to_string(), to_str(&path)])
            }
            FileStoreIn::Pwd => CliIn::args(vec!["pwd"]),
        }
    }
}

pub enum FileStoreOut {
    Init,
    Write,
    Read(Bin),
    Mkdir,
    Remove,
    List(Vec<PathBuf>),
    Exists(bool),
    Pwd(PathBuf),
}

#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct FileStoreCli {
    #[command(subcommand)]
    pub command: FileStoreCommand,
}

#[derive(Clone, Debug, Subcommand, EnumString, strum_macros::Display)]
pub enum FileStoreCommand {
    Init,
    Write { path: PathBuf },
    Read { path: PathBuf },
    Mkdir { path: PathBuf },
    Remove { path: PathBuf },
    List { path: PathBuf },
    Exists { path: PathBuf },
    Pwd,
}

impl FileStoreCli {
    pub fn new(command: FileStoreCommand) -> Self {
        FileStoreCli { command }
    }
}

impl Into<Vec<String>> for FileStoreCli {
    fn into(self) -> Vec<String> {
        match &self.command {
            FileStoreCommand::Init => vec!["init".to_string()],
            FileStoreCommand::Write { path } => {
                vec!["write".to_string(), to_str(path)]
            }
            FileStoreCommand::Read { path } => {
                vec!["read".to_string(), to_str(path)]
            }
            FileStoreCommand::Mkdir { path } => {
                vec!["mkdir".to_string(), to_str(path)]
            }
            FileStoreCommand::Remove { path } => {
                vec!["remove".to_string(), to_str(path)]
            }
            FileStoreCommand::List { path } => {
                vec!["list".to_string(), to_str(path)]
            },
            FileStoreCommand::Exists { path } => {
                vec!["exists".to_string(), to_str(path)]
            }
            FileStoreCommand::Pwd => {
                vec!["pwd".to_string()]
            }
        }
    }
}

pub fn to_str(path: &PathBuf) -> String {
    path.to_str().unwrap().to_string()
}

pub fn stringify(vec: Vec<&'static str>) -> Vec<String> {
    let mut rtn = vec![];
    for v in vec {
        rtn.push(v.to_string());
    }
    rtn
}

impl ToString for FileStoreCli {
    fn to_string(&self) -> String {
        self.command.to_string()
    }
}

impl TryFrom<CliOsExecutor> for FileStore {
    type Error = HypErr;

    fn try_from(cli: CliOsExecutor) -> Result<Self, Self::Error> {
        Ok(FileStore::Cli(Box::new(cli)))
    }
}

pub struct RootDir {
    root: PathBuf,
}

impl RootDir {
    pub fn new(root: PathBuf) -> Self {
        let root = root.clean();
        Self { root }
    }
}

impl RootDir {
    pub fn norm(&self, sub_path: &PathBuf) -> Result<PathBuf, HypErr> {
        let sub_path = sub_path.clean();

        let path: PathBuf = match sub_path.starts_with("/") {
            true => sub_path.strip_prefix("/")?.into(),
            false => sub_path.clone(),
        };
        let normed: PathBuf = self.root.join(path).clean();
        let parent = match normed.parent() {
            None => PathBuf::from_str("/")?,
            Some(parent) => parent.clone().into(),
        };

        if !parent.starts_with(&self.root) {
            return Err(HypErr::String(format!(
                "illegal path '{}' escapes filesystem boundaries",
                sub_path.display()
            )));
        }

        Ok(normed)
    }
}
