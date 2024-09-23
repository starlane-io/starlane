use std::path::PathBuf;
use clap::{Parser, Subcommand};
use strum_macros::EnumString;
use starlane::space::command::Command;

#[derive(Clone,Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Clone, Debug, Subcommand, EnumString, strum_macros::Display)]
pub enum Commands {
    Init,
    Write {path: PathBuf },
    Read {path: PathBuf},
    Mkdir{ path: PathBuf },
    Remove { path: PathBuf },
    List { path: Option<PathBuf> },
    Exists{ path: PathBuf },
    Pwd,
}

impl Cli {
    pub fn new( command: Commands) -> Self {
        Cli {
            command
        }
    }
}


impl Into<Vec<String>> for Cli {
    fn into(self) -> Vec<String> {
        stringify(match self.command {
            Commands::Init => vec!["init"],
            Commands::Write { path } => {
                vec!["write", to_str(path)]
            }
            Commands::Read { path } => {
                vec!["read", to_str(path)]
            }
            Commands::Mkdir { path } => {
                vec!["mkdir", to_str(path)]
            }
            Commands::Remove { path } => {
                vec!["remove", to_str(path)]
            }
            Commands::List { path } => {
                match path {
                    None => vec!["list"],
                    Some(path) => vec!["list", to_str(path)]
                }
            }
            Commands::Exists { path } => {
                vec!["exists", to_str(path)]
            }
            Commands::Pwd => {
                vec!["pwd"]
            }
        })
    }
}

pub fn to_str( path: PathBuf ) -> &'static str {
    path.to_str().unwrap()
}


pub fn stringify(vec: Vec<dyn ToString> ) -> Vec<String> {
    let mut rtn = vec![];
    for v in vec {
       rtn.push(v.to_string());
    }
    rtn
}

impl ToString for Cli{
    fn to_string(&self) -> String {
        self.command.to_string()
    }
}