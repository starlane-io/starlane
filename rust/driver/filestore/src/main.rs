use clap::{Parser, Subcommand};
use strum_macros::EnumString;
use std::{env, path::PathBuf};
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
    Read {path: PathBuf}
}

fn main() -> Result<(),()> {
    let cli = Cli::parse();
    let path = match cli.command  {
        Commands::Write{ref path} => path.clone(),
        Commands::Read{ref path} => path.clone()
    };

    println!("{} path -> {}", cli.command.to_string(), path.as_path().to_str().unwrap());


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
    }

    Ok(())
}
