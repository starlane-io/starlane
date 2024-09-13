use clap::{Parser, Subcommand};
use strum_macros::EnumString;
use std::{env, fs, path::PathBuf};
use std::io;

use std::fs::{File, FileType};
use std::io::{BufReader, BufWriter, StdinLock};
use std::io::prelude::*;
use std::path::Path;

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
    Delete { path: PathBuf },
    List { path: Option<PathBuf> },
    Test
}

fn main() -> Result<(),()> {
    let cli = Cli::parse();


    match cli.command {
        Commands::Write { path } => {

            println!("path: {}", path.display());
            let mut file = File::create(path ).unwrap();

            // Create a handle to stdin
            let mut input = std::io::stdin();

            // Use copy to transfer data from stdin to the file
            let bytes = std::io::copy(&mut input, &mut file).unwrap();
            println!("written: {}", bytes);

            // Flush the file's buffer to ensure all data is written
            file.flush().unwrap();
            println!("FLUSH COMPLETE");

            std::process::exit(0);

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
        Commands::List { path } => {

            let path = match path {
                None => PathBuf::from("."),
                Some(path) => path
            };

            let paths = fs::read_dir(path).unwrap();

            for path in paths {
                let path = path.unwrap().path();
                match path.is_dir() {
                    true => {
                        // we signal a directory path by appending a slash to the end
                        println!("{}/", path.display())
                    }
                    false => {
                        println!("{}", path.display())
                    }
                }
            }
        }
        Commands::Test =>  {
            println!("testing...");



            let dir = Path::new("./test-dir");

            fs::metadata(dir).and_then( |m| {
              println!("! --> ./test-dir already exists!");
                fs::remove_dir_all(dir).unwrap();
                println!("./test-dir removed");
                Ok(m)
            });
            fs::create_dir(dir).unwrap();
            println!("create_dir: {} ", dir.to_str().unwrap() );


            let file = dir.join("file1.txt");
            let mut file = File::create(file).unwrap();
            file.write_all(b"Hello, world!").unwrap();
            println!("write: file1.txt");

            let file = dir.join("file2.txt");
            let mut file = File::create(file).unwrap();
            file.write_all(b"Blah Blah blah!").unwrap();
            println!("write: file2.txt");
            let paths = fs::read_dir(dir).unwrap();
            println!("ls...");
            paths.into_iter().map( |d| d.unwrap() ).for_each(|e| {println!("- {}",e.file_name().to_os_string().to_str().unwrap())});


            println!("done");


        }
    }

    println!("done from WASM");

    Ok(())
}
