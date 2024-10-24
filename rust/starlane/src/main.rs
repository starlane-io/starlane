#![allow(warnings)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate starlane_macros;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());

pub mod err;
pub mod properties;
pub mod template;

pub mod env;

pub mod platform;

#[cfg(test)]
pub mod test;

//#[cfg(feature="space")]
//pub extern crate starlane_space as starlane;
#[cfg(feature = "space")]
pub mod space {
    pub use starlane_space::space::*;
}

#[cfg(feature = "service")]
pub mod service;

#[cfg(feature = "hyperspace")]
pub mod hyperspace;

#[cfg(feature = "hyperlane")]
pub mod hyperlane;
pub mod registry;

pub mod executor;
pub mod host;

#[cfg(feature = "cli")]
pub mod cli;

pub mod driver;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "server")]
pub use server::*;

use crate::cli::{Cli, Commands};
use crate::platform::Platform;
use anyhow::anyhow;
use clap::Parser;
use once_cell::sync::Lazy;
use starlane::space::loc::ToBaseKind;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::ops::{Add, Mul};
use std::path::{Path, PathBuf};
use std::process;
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;
use atty::Stream;
use colored::{Colorize, CustomColor};
use lerp::Lerp;
use text_to_ascii_art::fonts::get_font;
use text_to_ascii_art::to_art;
use tokio::{fs, signal};
use tokio::fs::DirEntry;
use tokio::runtime::Builder;
use zip::write::FileOptions;
use starlane_space::space::util::log;
use crate::env::STARLANE_HOME;

#[cfg(feature = "server")]
async fn config() -> StarlaneConfig {

    let file = format!("{}/config.yaml", STARLANE_HOME.to_string()).to_string();
    let config = match fs::try_exists(file.clone()).await {
        Ok(true) => {
            match fs::read_to_string(file.clone()).await {
                Ok(config) => {
                    match serde_yaml::from_str(&config).map_err(|e| anyhow!(e)) {
                        Ok(config) => config,
                        Err(err) => {
                            println!("starlane config file '{}' failed to parse: '{}'", file, err.to_string());
                            Default::default()
                        }
                    }
                }
                Err(err) => {
                    println!("starlane config file '{}' error when attempting to read to string: '{}'", file, err.to_string());
                    Default::default()
                }
            }
        }
        Ok(false) => {
           let config = Default::default();
            if let Ok(ser) = serde_yaml::to_string(&config) {
                let file: PathBuf = file.into();
                match file.parent() {
                    None => {}
                    Some(dir) => {
                        fs::create_dir_all(dir).await.unwrap_or_default();
                        fs::write(file,ser).await.unwrap_or_default();
                    }
                }
            }
           config
        }
        Err(err) => {
            println!("starlane encountered problem when attempting to load config file: '{}' with error: '{}'", file, err.to_string());
            Default::default()
        }
    };
    config
}



pub fn init() {
    #[cfg(feature = "cli")]
    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider()
            .install_default()
            .expect("crypto provider could not be installed");
    }
}

#[cfg(feature = "cli")]
pub fn main() -> Result<(), anyhow::Error> {

    ctrlc::set_handler(move || process::exit(1)).unwrap();

    init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Run => run(),
        Commands::Term(args) => {
            let runtime = Builder::new_multi_thread().enable_all().build()?;

            match runtime.block_on(async move { cli::term(args).await }) {
                Ok(_) => Ok(()),
                Err(err) => {
                    println!("err! {}", err.to_string());
                    Err(err.into())
                }
            }
        }
        Commands::Version => {
            println!("{}", VERSION.to_string());
            Ok(())
        }
    }
}

#[cfg(not(feature = "server"))]
fn run() -> Result<(), anyhow::Error> {
    println!("'' feature is not enabled in this starlane installation");
    Err(anyhow!(
        "'machine' feature is not enabled in this starlane installation"
    ))
}

#[cfg(feature = "server")]
fn run() -> Result<(), anyhow::Error> {
    match to_art("*STARLANE*".to_string(), "default", 0, 0, 0) {
        Ok(string) => {
            let begin= (0xFF, 0xFF, 0xFF);
            let end= (0xEE, 0xAA, 0x5A);
            let end= (0x6D, 0xD7, 0xFD);

            //let begin = (0x00, 0x00, 0x00);
            // this is bad code however I couldn't find out how to get lines().len() withou
            // giving up ownership (therefor the clone)
            let size = string.clone().lines().count();
            let mut index = 0;
            for line in  string.lines(){
                let progress =  index as f32 / size as f32;
                //let color = begin.clone().lerp(end.clone(), progress);
                //let color= color.custom();
                let r = (begin.0 as f32).lerp(end.0 as f32, progress) as u8;
                let g = (begin.1 as f32).lerp(end.1 as f32, progress) as u8;
                let b = (begin.2 as f32).lerp(end.2 as f32, progress) as u8;
                println!("{}", line.truecolor(r, g, b));
                index = index + 1;
            }

//            println!("{}", string.truecolor(0xEE, 0xAA, 0x5A));
        }
        Err(err) =>  {
            eprintln!("err! {}", err.to_string());
        }
    }
    let spinner = cliclack::spinner();


    spinner.start("Starlane starting...");


    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let config = config().await;
        let starlane = Starlane::new(config.registry).await.unwrap();
        let machine_api = starlane.machine();

        let api = tokio::time::timeout(Duration::from_secs(30), machine_api)
            .await
            .unwrap()
            .unwrap();
        // this is a dirty hack which is good enough for a 0.3.0 release...
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
    Ok(())
}

/*
#[no_mangle]
pub extern "C" fn starlane_uuid() -> loc::Uuid {
loc::Uuid::from(uuid::Uuid::new_v4()).unwrap()
}

#[no_mangle]
pub extern "C" fn starlane_timestamp() -> Timestamp {
Timestamp { millis: Utc::now().timestamp_millis() }
}

*/
/*
#[cfg(feature = "cli")]
async fn cli() -> Result<(), SpaceErr> {
    let home_dir: String = match dirs::home_dir() {
        None => ".".to_string(),
        Some(dir) => dir.display().to_string(),
    };
    let matches = ClapCommand::new("cosmic-cli")
        .arg(
            Arg::new("host")
                .short('h')
                .long("host")
                .takes_value(true)
                .value_name("host")
                .required(false)
                .default_value("localhost"),
        )
        .arg(
            Arg::new("certs")
                .short('c')
                .long("certs")
                .takes_value(true)
                .value_name("certs")
                .required(false)
                .default_value(format!("{}/.old/localhost/certs", home_dir).as_str()),
        )
        .subcommand(ClapCommand::new("script"))
        .allow_external_subcommands(true)
        .get_matches();

    let host = matches.get_one::<String>("host").unwrap().clone();
    let certs = matches.get_one::<String>("certs").unwrap().clone();
    let session = Session::new(host, certs).await?;

    if matches.subcommand_name().is_some() {
        session.command(matches.subcommand_name().unwrap()).await
    } else {
        loop {
            let line: String = text_io::try_read!("{};").map_err(|e| SpaceErr::new(500, "err"))?;

            let line_str = line.trim();

            if "exit" == line_str {
                return Ok(());
            }
            println!("> {}", line_str);
            session.command(line.as_str()).await?;
        }
        Ok(())
    }
}

 */

pub fn zip_dir<T>(
    it: impl Iterator<Item = DirEntry>,
    prefix: &str,
    writer: T,
    method: zip::CompressionMethod,
) -> zip::result::ZipResult<T>
where
    T: Write + Seek,
{
    let mut zip = zip::ZipWriter::new(writer);
    let options = FileOptions::default()
        .compression_method(method)
        .unix_permissions(0o755);

    let mut buffer = Vec::new();
    for entry in it {
        let path = entry.path();
        let name = path.strip_prefix(Path::new(prefix)).unwrap();

        // Write file or directory explicitly
        // Some unzip tools unzip files with directory paths correctly, some do not!
        if path.is_file() {
            zip.start_file(name.to_str().unwrap(), options)?;
            let mut f = File::open(path)?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&*buffer)?;
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            // Only if not root! Avoids path spec / warning
            // and mapname conversion failed error on unzip
            zip.add_directory(name.to_str().unwrap(), options)?;
        }
    }
    let result = zip.finish()?;
    Result::Ok(result)
}

/*

#[derive(Lerp,Clone)]
struct Color {
   pub r: Nu,
   pub g: Nu,
   pub b: Nu,
}

#[derive(Lerp,Clone)]
pub struct Nu {
    value: u8
}

impl Nu {
    pub fn new(value: u8) -> Nu {
        Nu { value }
    }
}


impl Mul for Nu {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Nu::new((self.value as f32 * rhs.value as f32) as u8)
    }
}

impl Add for Nu {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Nu::new(self.value + rhs.value  )
    }
}

impl Color {
    pub fn new( r: u8, g: u8, b: u8 ) -> Self {
        let r = Nu::new(r);
        let g = Nu::new(g);
        let b = Nu::new(b);
        Self { r, g, b }
    }

    pub fn custom(&self) -> CustomColor {
        CustomColor::new(self.r.value.clone() , self.g.value.0.clone() , self.b.value.0.clone() )
    }
}

 */






