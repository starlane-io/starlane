use clap::{Parser, Subcommand};
use strum_macros::EnumString;
use std::{env, fs, path::PathBuf};
use std::io;

use std::fs::File;
use std::io::prelude::*;
/// Simple program to greet a person

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand, EnumString, strum_macros::Display)]
enum Commands {
    Write {path: PathBuf},
    Read {path: PathBuf},
    Mkdir{ path: PathBuf },
    Delete { path: PathBuf }
}

fn main() -> Result<(),()> {
    let cli = Cli::parse();


    match cli.command {
        Commands::Write { path } => {
          let mut file = File::create(path).unwrap();
           let mut buf : [u8;1024] = [0; 1024];
           while io::stdin().read(& mut buf).unwrap() > 0 {
               file.write( & buf ).unwrap();
           }
        }
        Commands::Read { path } => {

            let mut file = File::open(path).unwrap();
            let mut buf : [u8;1024] = [0; 1024];
            for size in file.read(& mut buf) {
                if size == 0 {
                    break;
                }
                io::stdout().write( & buf ).unwrap();
            }
        }
        Commands::Mkdir { path } => {
            fs::create_dir(path).unwrap();
        }
        Commands::Delete { path } => {
            fs::remove_file(path).unwrap();
        }
    }

    Ok(())
}
