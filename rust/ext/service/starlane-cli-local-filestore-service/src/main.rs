use std::{env, fs, io};
use std::fs::{File, ReadDir};
use std::io::{Read, Write};
use std::path::{absolute, Path, PathBuf};
use clap::Parser;
use thiserror::Error;
use once_cell::sync::Lazy;
use starlane::dialect::cli::filestore::Cli;
use starlane::dialect::cli::filestore::Commands;




pub fn data_dir() -> String {
    env::var("DATA_DIR").unwrap_or(".".to_string())
}

pub fn root_dir() -> PathBuf {
    absolute(data_dir()).expect("could not determine absolute path of DATA_dir_root()")
}

pub fn dir_root() -> PathBuf {
    root_dir().join("DIR")
}

pub fn state_root() -> PathBuf {
    root_dir().join("STATE")
}

const TMP : &str = "state.data";

fn main() -> Result<(),Error> {

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            ensure_dir(&dir_root());
            ensure_dir(&state_root());
            Ok(())
        }
        Commands::Write { path } => {
            let pair = StatePair::file(&path);
            let state_file = pair.create()?;
            let mut file = File::create(state_file)?;
            io::copy(&mut io::stdin(), &mut file)?;
            Ok(())
        }
        Commands::Read { path } => {
            let pair = StatePair::file(&path);
            pair.exists().map_err(|_| { Error::String(format!("file {} does not exist", path.display())) })?;

            let mut file = File::open(pair.state_file()?)?;
            io::copy(&mut file, &mut io::stdout())?;
            Ok(())
        }
        Commands::Mkdir { path } => {
            let pair = StatePair::dir(&path);
            pair.create()?;
            Ok(())
        }
        Commands::Delete { path } => {
            // delete is always treated as a file but will delte if it is a Dir or a File
            StatePair::file(&path).delete()
        }
        Commands::List { path } => {
            let path = path.unwrap_or("/".into());
            let pair = StatePair::dir( &path );
            let read = pair.read_dir()?;

            for path in read {

                let path = absolute(path?.path())?;
                let relative = path.to_str().ok_or("could not convert path to string".to_string())?.to_string();
                let pattern = format!("^{}",root_dir().display()).to_string();
                let relative = relative.replace(pattern.as_str(),"");
                let relative = relative.into();


                match StatePair::from_path(&relative) {
                    None => {}
                    Some(StatePair::File(file)) => {
                        io::stdout().write(file.to_str().ok_or("could not convert path to string".to_string())?.as_bytes())?;
                        io::stdout().write("\n".as_bytes())?;
                    }
                    Some(StatePair::Dir(file)) => {
                        io::stdout().write(format!("{}/",file.to_str().ok_or("could not convert path to string".to_string())?).as_bytes())?;
                        io::stdout().write("\n".as_bytes())?;
                    }
                }

            }
            Ok(())
        }
        Commands::Pwd =>  {
            println!("{}", root_dir().to_str().unwrap());
            Ok(())
        }

        Commands::Exists { path } => {
            let pair = StatePair::dir(&path);
            pair.exists()
        }
    }


}

pub enum StatePair {
    Dir(PathBuf),
    File(PathBuf)
}

impl StatePair {

    pub fn from_path( path: &PathBuf) -> Option<Self> {
        let state = state_root().join(path).join(TMP);
        let dir = dir_root().join(path);

        if state.exists() {
            Option::Some(Self::file(path))
        } else if dir.exists() {
            Option::Some(Self::dir(path))
        } else {
            Option::None
        }
    }


    pub fn dir(path: &PathBuf ) -> Self {
        Self::Dir(path.clone())
    }

    pub fn file(path: &PathBuf ) -> Self {
        Self::File(path.clone())
    }

    pub fn raw(&self) -> PathBuf {
        match self {
            StatePair::Dir(path) => path.clone(),
            StatePair::File(path) =>  path.clone()
        }
    }

    pub fn read_dir(&self) -> Result<ReadDir,Error>{
        let dir = self.get_dir();
        let state = self.get_state();
        // first check if this is actually a dir
        if state.exists() {
            return Err(format!("cannot list.  file '{}' is not a directory", self.raw().display()).into());
        }

        Ok(dir.read_dir()?)
    }

    fn get_state(&self) -> PathBuf {
        let path = match self {
            StatePair::Dir(path) => path,
            StatePair::File(path) => path
        };
        state_root().join(path).join(TMP)
    }


    fn get_dir(&self) -> PathBuf {
        let path = match self {
            StatePair::Dir(path) => path,
            StatePair::File(path) => path
        };
        dir_root().join(path)
    }

    pub fn create(&self) -> Result<PathBuf,Error> {
        match self {
            StatePair::Dir(path) => {
                Self::check_for_parent_state(path)?;
                let dir = dir_root().join(path);
                ensure_dir(&dir)?;
                Ok(dir)
            },
            StatePair::File(path) => {
                let parent = path.parent().ok_or("expected parent".to_string())?.clone().into();
                Self::check_for_parent_state(&parent)?;
                ensure_dir(&parent)?;
                let state = state_root().join(path).join(TMP);
                Ok(state)
            }
        }
    }

    /**
      * need to return a manifest of all directories deleted...
      */
    pub fn delete(&self) -> Result<(),Error> {
        fn delete_dir(path: &Path ) -> Result<(),Error>{
            let dir = dir_root().join(path);
            if dir.exists() {
                fs::remove_dir(dir)?;
            }
            let state_dir = dir_root().join(path);
            if state_dir.exists() {
                fs::remove_dir_all(state_dir)?;
            }

            Ok(())
        }

        match self{
            StatePair::Dir(dir) => delete_dir(dir),
            StatePair::File(path) => delete_dir(path)
        }
    }



    pub fn state_file(&self) -> Result<PathBuf,Error> {
        match self {
            StatePair::Dir(path) => Result::Err(format!("cannot create state for a directory {}",path.display()))?,
            StatePair::File(path) => {
                let state = state_root().join(path).join(TMP);
                Ok(state)
            }
        }
    }


    /// make sure all parent directories are not stateful
    fn check_for_parent_state(path: &PathBuf) -> Result<(),String>{
       let mut parent = state_root().join(path);
       loop {
           let state = parent.join(TMP);
           if state.exists() {
               return Result::Err(format!("could not create the directory '{}' because '{}' is a file",path.display(), parent.display()))
           }
           parent = parent.parent().ok_or(format!("expected parent for: {}", path.display()))?.clone().into();
           if *state_root() == parent {
               return Ok(())
           }
       }
    }

    fn exists( &self ) -> Result<(),Error> {

        fn dir_exists( dir: &Path ) -> Result<(),Error>{
            if !dir_root().join(dir).exists() {
                Err(format!("directory '{}' does not exist", dir.display()).into())
            } else {
                Ok(())
            }
        }


        match self {
            StatePair::Dir(dir) => {
                dir_exists( dir )
            }
            StatePair::File(state) => {
                dir_exists( state )?;

                if !state_root().join(state).join(TMP).exists() {
                    return Err(format!("state file '{}' does not exist", state.display()).into())
                }

                Ok(())
            }
        }
    }

    fn ensure(&self) -> Result<(),Error>{
        match self {
            StatePair::Dir(dir) => {
                let dir= dir_root().join(dir);
                ensure_dir(&dir)?;
                Ok(())
            }
            StatePair::File(state) => {
                let dir = dir_root().join(state);
                let state = dir_root().join(state);
                ensure_dir(&dir)?;
                ensure_dir(&state)?;
                Ok(())
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
    #[error("could not access local filesystem")]
    String( String),

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


#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {

    }

}