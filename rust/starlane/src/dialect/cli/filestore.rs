use std::path::PathBuf;
use clap::{Parser, Subcommand};
use strum_macros::EnumString;

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand, EnumString, strum_macros::Display)]
pub enum Commands {
    Init,
    Write {path: PathBuf },
    Read {path: PathBuf},
    Mkdir{ path: PathBuf },
    Delete { path: PathBuf },
    List { path: Option<PathBuf> },
    Exists{ path: PathBuf },
    Pwd,
}
