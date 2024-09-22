use std::{env, fs, io};
use std::fs::{File, ReadDir};
use std::io::{Read, Write};
use std::path::{absolute, Path, PathBuf, StripPrefixError};
use clap::Parser;
use thiserror::Error;
use once_cell::sync::Lazy;
use starlane::dialect::cli::filestore::Cli;
use starlane::dialect::cli::filestore::Commands;




pub fn data_dir() -> String {
    env::var("DATA_DIR").unwrap_or(env::current_dir().unwrap().display().to_string())
}

pub fn root_dir() -> PathBuf {
    absolute(data_dir()).expect("could not determine absolute path of DATA_dir_root()")
}

pub fn dir_root() -> PathBuf {
    let dir = PathBuf::from("DIR");
    join(root_dir(),dir).unwrap()
}


fn main() -> Result<(),Error> {
    env::var("DATA_DIR").map_err( |_|"DATA_DIR environment variable must be set")?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            ensure_dir(&dir_root());
            Ok(())
        }
        Commands::Write { path } => {
            let file = join(dir_root(),path)?;
            let mut file = File::create(file)?;
            io::copy(&mut io::stdin(), &mut file)?;
            Ok(())
        }
        Commands::Read { path } => {
            let file = join(dir_root(),path)?;
            let mut file = File::open(file)?;
            io::copy(&mut file, &mut io::stdout())?;
            Ok(())
        }
        Commands::Mkdir { path } => {
            let dir = join(dir_root(),path)?;
            fs::create_dir_all(dir)?;
            Ok(())
        }
        Commands::Delete { path } => {
            let file = join(dir_root(),path)?;
            if file.is_file() {
                fs::remove_file(file)?;
            }
            else {
                fs::remove_dir(file)?;
            }
            Ok(())           // delete is always treated as a file but will delte if it is a Dir or a File
        }
        Commands::List { path } => {
            let path = path.unwrap_or("/".into());
            let file = join(dir_root(),path)?;
            for f in file.read_dir()?.into_iter().map(|r|r.unwrap()) {
                println!("{}",f.path().display());
            }
            Ok(())
        }
        Commands::Pwd =>  {
            println!("root {}", root_dir().to_str().unwrap());
            println!("data dir {}", data_dir());
            Ok(())
        }

        Commands::Exists { path } => {
            let file = join(dir_root(),path)?;
            match file.exists() {
                true => Ok(()),
                false => Err("file does not exist".into())
            }
        }
    }


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

pub fn join( path: PathBuf, ext: PathBuf) -> Result<PathBuf, Error>{

    let ext : PathBuf = match ext.starts_with("/") {
        true => ext.strip_prefix("/")?.into(),
        false => ext
    };
    let joined = path.join(ext);

    println!("JOINED {}", joined.display());
    Ok(joined)
}



#[cfg(test)]
pub mod test {
    use std::fs;

    #[test]
    pub fn test() {


    }

}