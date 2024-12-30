use clap::Parser;
use starlane::executor::dialect::filestore::FileStoreCli;
use starlane::executor::dialect::filestore::FileStoreCommand;
use std::env::VarError;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{absolute, PathBuf, StripPrefixError};
use std::process::{ExitCode, Termination};
use std::{env, fs, io};
use thiserror::Error;


pub const FILE_STORE_ROOT: &'static str = "FILE_STORE_ROOT";

pub fn root_dir() -> Result<PathBuf,Error> {
    Ok(absolute(env::var(FILE_STORE_ROOT)?)?)
}


fn main() -> ExitCode{
    match run() {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            ExitCode::FAILURE
        }
    }
}


fn run() -> Result<(),Error> {


    let cli = FileStoreCli::parse();
    if let FileStoreCommand::Init = cli.command {
        ensure_dir(&root_dir()?);
        return Ok(());
    }


    match cli.command {
        FileStoreCommand::Init => {
            todo!()
        }
        FileStoreCommand::Write { path } => {
            todo!()

        }
        FileStoreCommand::Read { path } => {
            todo!()
        }
        FileStoreCommand::Mkdir { path } => {
            todo!()
        }
        FileStoreCommand::Remove { path } => {
            todo!()
        }
        FileStoreCommand::List { path: Option::Some(path)} => {
            todo!()
        }
        FileStoreCommand::List { path: Option::None } => {
            todo!()
        }
        FileStoreCommand::Pwd =>  {
            todo!()
        }

        FileStoreCommand::Exists { path } => {
            todo!()
        }
    }
}

fn norm(orig: &PathBuf ) -> Result<PathBuf,Error> {
    let path: PathBuf = match orig.starts_with("/") {
        true => orig.strip_prefix("/")?.into(),
        false => orig.clone()
    };
    let root_dir = root_dir()?;
//    let normed= canonicalize(absolute(root_dir.join(path))?)?;
    let normed : PathBuf = root_dir.join(path).into();
    let parent = normed.parent().unwrap().canonicalize()?;

    if let Option::Some(root) = root_dir.parent() {
        if parent == root {
            return Err(Error::String(format!("illegal path '{}' escapes filesystem boundaries", orig.display())));
        }
    }

    if !parent.starts_with(&root_dir){
        return Err(Error::String(format!("illegal path '{}' escapes filesystem boundaries", orig.display())));
    }

    Ok(normed)
}


fn ensure_dir(dir: &PathBuf ) -> Result<(),Error> {
   if dir.exists() && dir.is_dir(){
        Ok(())
    } else {
       fs::create_dir_all(dir)?;
       Ok(())
    }
}


#[derive(Error, Debug)]
pub enum Error{
    #[error("could not access local filesystem")]
    FileSys(#[from] io::Error),
    #[error("{0}")]
    String( String),
    #[error("{0}")]
    Path(#[from] StripPrefixError),
    #[error("{0}")]
    VarError(#[from] VarError),

}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}


impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

/*
pub fn join( path: PathBuf, ext: PathBuf) -> Result<PathBuf, Error>{

    let ext : PathBuf = match ext.starts_with("/") {
        true => ext.strip_prefix("/")?.into(),
        false => ext
    };
    let joined = path.join(ext);

    println!("JOINED {}", joined.display());
    Ok(joined)
}

 */



#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {


    }

}