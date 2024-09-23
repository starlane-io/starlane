use clap::Parser;
use starlane::dialect::cli::filestore::Cli;
use starlane::dialect::cli::filestore::Commands;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{absolute, PathBuf, StripPrefixError};
use std::{env, fs, io};
use std::env::VarError;
use std::process::{ExitCode, Termination};
use thiserror::Error;


pub const FILE_STORE_ROOT: &'static str = "FILE_STORE_ROOT";

pub fn root_dir() -> Result<PathBuf,Error> {
    Ok(absolute(env::var(FILE_STORE_ROOT)?)?)
}


fn main() -> ExitCode{
    match run() {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}",err.to_string());
            ExitCode::FAILURE
        }
    }
}


fn run() -> Result<(),Error> {
    env::var(FILE_STORE_ROOT).map_err( |_|format!("'{}' environment variable is not set.", FILE_STORE_ROOT).to_string())?;


    let cli = Cli::parse();
    if let Commands::Init = cli.command {
        ensure_dir(&root_dir()?);
        return Ok(());
    }

     if !root_dir()?.exists()  {
          let root = root_dir()?.to_str().unwrap().to_string();
          Err(format!("{} is set but directory {} does not exisst.  Run 'init' command first.",FILE_STORE_ROOT,root).to_string())?;
      }

    match cli.command {
        Commands::Init => {
            Ok(())
        }
        Commands::Write { path } => {
            let file = norm(&path)?;
            let mut file = File::create(file)?;
            io::copy(&mut io::stdin(), &mut file)?;
            Ok(())
        }
        Commands::Read { path } => {
            let file = norm(&path)?;
            let mut file = File::open(file)?;
            io::copy(&mut file, &mut io::stdout())?;
            Ok(())
        }
        Commands::Mkdir { path } => {
            let dir = norm(&path)?;
            fs::create_dir_all(dir)?;
            Ok(())
        }
        Commands::Remove { path } => {
            let file = norm(&path)?;
            if file.is_file() {
                fs::remove_file(file)?;
            }
            else {
                fs::remove_dir(file)?;
            }
            Ok(())           // delete is always treated as a file but will delte if it is a Dir or a File
        }
        Commands::List { path: Option::Some(path)} => {
            let file = norm(&path)?;
            for f in file.read_dir()?.into_iter().map(|r|r.unwrap()) {
                println!("{}",f.path().display());
            }
            Ok(())
        }
        Commands::List { path: Option::None } => {
            let path = "/".into();
            let file = norm(&path)?;
            for f in file.read_dir()?.into_iter().map(|r|r.unwrap()) {
                println!("{}",f.path().display());
            }
            Ok(())
        }
        Commands::Pwd =>  {
            println!("{}", root_dir()?.to_str().unwrap());
            Ok(())
        }

        Commands::Exists { path } => {
            let file = norm(&path)?;
            match file.exists() {
                true => Ok(()),
                false => Err("file does not exist".into())
            }
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