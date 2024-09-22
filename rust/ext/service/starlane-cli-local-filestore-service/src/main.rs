use std::{env, fs, io};
use std::fs::{File, ReadDir};
use std::io::{Read, Write};
use std::path::{absolute, Path, PathBuf};
use clap::Parser;
use thiserror::Error;
use once_cell::unsync::Lazy;
use starlane::dialect::cli::filestore::Cli;
use starlane::dialect::cli::filestore::Commands;


pub static DATA_DIR: Lazy<String> = Lazy::new(||{env::var("DATA_DIR").unwrap_or(".".to_string())});
pub static ROOT_DIR: Lazy<Result<PathBuf,Error>> = Lazy::new(||{absolute(DATA_DIR.into()).into()});
pub static DIR : Lazy<PathBuf> = Lazy::new(||{ROOT_DIR.unwrap().join("DIR")});
pub static STATE : Lazy<PathBuf> = Lazy::new(||{ROOT_DIR.unwrap().join("STATE")});

const STATE_FILE : &str = "state.data";

fn main() -> Result<(),Error> {
    ROOT_DIR?;

    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            ensure_dir(&DIR);
            ensure_dir(&STATE);
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
            if !pair.exists() {
                return Result::Err(format!("file {} does not exist", path.display()).into());
            }

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
                let relative: PathBuf  = path.to_str().ok_or(Err("could not convert path to string".into()))?.to_string().replace( format!("^{}",ROOT_DIR?.to_string()),"").into();
                match StatePair::from_path(&relative) {
                    None => {}
                    Some(StatePair::File(file)) => {
                        io::stdout().write(file.to_str().ok_or(Err("could not convert path to string".into()))?.as_bytes())?;
                        io::stdout().write("\n")?;
                    }
                    Some(StatePair::Dir(file)) => {
                        io::stdout().write(format!("{}/",file.to_str().ok_or(Err("could not convert path to string".into()))?.as_bytes())?)?;
                        io::stdout().write("\n")?;
                    }
                }
            }
        }
        Commands::Pwd =>  {
            println!("{}", ROOT_DIR?.to_str());
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
        let state = STATE.join(path).join(STATE_FILE);
        let dir = DIR.join(path);

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
        STATE.join(path).join(STATE_FILE)
    }


    fn get_dir(&self) -> PathBuf {
        let path = match self {
            StatePair::Dir(path) => path,
            StatePair::File(path) => path
        };
        DIR.join(path)
    }

    pub fn create(&self) -> Result<PathBuf,Error> {
        match self {
            StatePair::Dir(path) => {
                Self::check_for_parent_state(path)?;
                let dir = DIR.join(path);
                ensure_dir(&dir)?;
                Ok(dir)
            },
            StatePair::File(path) => {
                let parent = path.parent().ok_or("expected parent")?.clone().into();
                Self::check_for_parent_state(&parent)?;
                ensure_dir(&parent)?;
                let state = STATE.join(path).join(STATE_FILE);
                Ok(state)
            }
        }
    }

    /**
      * need to return a manifest of all directories deleted...
      */
    pub fn delete(&self) -> Result<(),Error> {
        fn delete_dir(path: &Path ) -> Result<(),Error>{
            let dir = DIR.join(path);
            if dir.exists() {
                fs::remove_dir(dir)?;
            }
            let state_dir = DIR.join(path);
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
            StatePair::Dir(path) => Result::Err(format!("cannot create state for a directory {}",path))?,
            StatePair::File(path) => {
                let state = STATE.join(path).join(STATE_FILE);
                Ok(state)
            }
        }
    }


    /// make sure all parent directories are not stateful
    fn check_for_parent_state(path: &PathBuf) -> Result<(),String>{
       let mut parent = STATE.join(path);
       loop {
           let state = parent.join(STATE_FILE);
           if state.exists() {
               return Result::Err(format!("could not create the directory '{}' because '{}' is a file",path, parent.display()))
           }
           parent = parent.parent().ok_or(format!("expected parent for: {}", path.display()))?.clone().into();
           if *STATE == parent {
               return Ok(())
           }
       }
    }

    fn exists( &self ) -> Result<(),Error> {

        fn dir_exists( dir: &Path ) -> Result<(),Error>{
            if !DIR.join(dir).exists() {
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

                if !STATE.join(state).join(STATE_FILE).exists() {
                    return Err(format!("state file '{}' does not exist", state.display()).into())
                }

                Ok(())
            }
        }
    }

    fn ensure(&self) -> Result<(),Error>{
        match self {
            StatePair::Dir(dir) => {
                let dir= DIR.join(dir);
                ensure_dir(&dir)?;
                Ok(())
            }
            StatePair::File(state) => {
                let dir = DIR.join(state);
                let state = DIR.join(state);
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
pub enum Error {
    #[error("could not access local filesystem")]
    FileSys(#[from] io::Error),
    #[error("could not access parent {0}")]
    BadParent(String),
    #[error("{0}")]
    String(#[from] String)
}


#[cfg(test)]
pub mod test {

    #[test]
    pub fn test() {

    }

}