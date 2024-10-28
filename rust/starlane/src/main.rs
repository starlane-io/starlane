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


pub mod foundation;

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
use crate::env::STARLANE_HOME;
use crate::err::HypErr;
use crate::platform::Platform;
use anyhow::{anyhow, Error};
use ascii::AsciiChar::P;
use atty::Stream;
use clap::Parser;
use cliclack::log::{error, remark, success, warning};
use cliclack::{
    clear_screen, confirm, intro, multi_progress, outro, progress_bar, select, spinner,
    ProgressBar,
};
use colored::{Colorize, CustomColor};
use lerp::Lerp;
use nom::{InputIter, InputTake, Slice};
use once_cell::sync::Lazy;
use starlane::space::loc::ToBaseKind;
use starlane_primitive_macros::ToBase;
use starlane_space::space::particle::Progress;
use starlane_space::space::util::log;
use std::cmp::min;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::ops::{Add, Mul};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;
use std::{io, process};
use std::fmt::Display;
use termsize::Size;
use text_to_ascii_art::fonts::get_font;
use text_to_ascii_art::to_art;
use tokio::fs::DirEntry;
use tokio::runtime::Builder;
use tokio::{fs, join, signal};
use tracing::instrument::WithSubscriber;
use tracing::Instrument;
use zip::write::FileOptions;
use crate::foundation::Foundation;
use crate::foundation::StandAloneFoundation;
use crate::registry::postgres::embed::PgEmbedSettings;

fn config_path() -> String {
    format!("{}/config.yaml", STARLANE_HOME.to_string()).to_string()
}

#[cfg(feature = "server")]
async fn config() -> Result<Option<StarlaneConfig>, HypErr> {
    let file = config_path();
    match fs::try_exists(file.clone()).await? {
        true => {
            let config = fs::read_to_string(file.clone()).await?;
            let config = serde_yaml::from_str(config.as_str()).map_err(|err| anyhow!("starlane config found: '{}' yet Starlane encountered an error when attempting to process the config: '{}'", config_path(), err))?;
            Ok(Some(config))
        }
        false => Ok(None),
    }
}

async fn config_save(config: StarlaneConfig) -> Result<(), anyhow::Error> {
    let file = config_path();
    match serde_yaml::to_string(&config) {
        Ok(ser) => {
            let file: PathBuf = file.into();
            match file.parent() {
                Some(dir) => {
                    fs::create_dir_all(dir).await?;
                    fs::write(file, ser).await?;
                    Ok(())
                }
                None => {
                    Err(anyhow!("starlane encountered an error when attempting to save config file: 'invalid parent'"))
                }
            }
        }
        Err(err) => Err(anyhow!(
            "starlane internal error: 'could not deserialize config"
        )),
    }
}

/*
let config = Default::default();

config

 */

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
        Commands::Splash => {
            splash_html();
            Ok(())
        }
        Commands::Install => install(),
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
        Commands::Nuke => {
            nuke();
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
    info("starlane started.")?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {

        intro("RUN STARLANE").unwrap_or_default();

        async fn runner() -> Result<(),anyhow::Error> {
            spinner().start("initializing");
            tokio::time::sleep(Duration::from_millis(100)).await;
            info("initialization complete.")?;

            tokio::time::sleep(Duration::from_millis(100)).await;
            spinner().set_message("loading configuration");

            let config = match config().await {
                Ok(Some(config)) => config,
                Ok(None) => {
                    delay(1000).await;
                    spinner().error("Starlane configuration not found.");
                    delay(100).await;
                    error(format!("Starlane looked for a configuration here: '{}' But none was found.", config_path()))?;
                    delay(100).await;
                    note("wrong config?", format!("if '{}' isn't the config file you wanted, please set environment variable `export STARLANE_HOME=\"/config/parent/dir\"", config_path()))?;
                    delay(100).await;
                    note("install", "please run `starlane install` to configure a new Starlane runner")?;
                    delay(100).await;
                    outro("Good Luck!")?;
                    newlines(3,100).await;
                    process::exit(1);
                }
                Err(err) => {
                    delay(1000).await;
                    spinner().error("invalid configuration");
                    delay(100).await;
                    error(format!("{}", err.to_string()))?;
                    delay(100).await;
                    note("wrong config?", format!("if '{}' isn't the config file you wanted, please set environment variable `export STARLANE_HOME=\"/config/parent/dir\"", config_path()))?;
                    delay(100).await;
                    note("fresh install", "To create a fresh configuration please run: `starlane install`")?;
                    delay(100).await;
                    outro("Good Luck!")?;
                    newlines(3,100).await;
                    process::exit(1);
                }
            };

            delay(1000).await;
            success("starlane configured.")?;
            delay(100).await;
            spinner().set_message("launching registry");
            delay(1000).await;
            let starlane = Starlane::new(config,StandAloneFoundation()).await.map_err(|e|{println!("{}",e.to_string()); e}).unwrap();
            success("registry ready.")?;
            delay(100).await;
            spinner().set_message("starting starlane...");

            let machine_api = starlane.machine();

            delay(100).await;
            success("starlane started.")?;
            delay(100).await;
            spinner().set_message("waiting for ready status...");
            let api = tokio::time::timeout(Duration::from_secs(30), machine_api).await??;
            delay(100).await;
            success("starlane ready.")?;

            delay(100).await;
            spinner().clear();
            newlines(3, 100).await;

            delay(2000).await;
            splash_with_params(1,2, 25).await;
            delay(250).await;
            note("what's next?", "You can connect to this starlane runner with a control terminal: `starlane term`" )?;
            delay(250).await;
            outro("Starlane is running.")?;
            newlines(3, 100).await;

            // this is a dirty hack which is good enough for a 0.3.0 release...
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }

            Ok(())
        };

        match runner().await {
            Ok(_) => {}
            Err(err) =>
                {
                    delay(250).await;
                    error(format!("starlane halted due to an error: {}", err.to_string())).unwrap_or_default();
                    delay(250).await;
                    outro("runner failed").unwrap();
                    newlines(3, 100).await;
                }
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

static COLORS: (u8, u8, u8) = (0x6D, 0xD7, 0xFD);

async fn splash() {
    splash_with_params(6, 6, 100).await;
}

fn info(text: &str) -> io::Result<()> {
    let padding = 10usize;
    let size = term_width();
    let len = size - padding;
    let text = textwrap::wrap(text, len).join("\n");
    cliclack::log::info(text)
}

fn term_width() -> usize {
    match termsize::get() {
        None => 128,
        Some(size) => size.cols as usize
    }
}


pub fn note(prompt: impl Display, message: impl Display) -> io::Result<()> {
    let padding = 10usize;
    let size = term_width();
    let len = size - padding;
    let text = textwrap::wrap(message.to_string().as_str(), len).join("\n");
    cliclack::note(prompt,text)
}


pub fn wrap( text: impl Display) -> impl Display {
    let padding = 10usize;
    let size = term_width();
    let len = size - padding;
    textwrap::wrap(text.to_string().as_str(), len).join("\n")
}



async fn splash_with_params(pre: usize, post: usize, interval: u64) {
        let banners = if term_width() > splash_widest("*STARLANE*") {
            vec!["*STARLANE*"]
        } else {
            vec!["STAR", "LANE"]
        };
        splash_with_params_and_banners(pre, post, interval, banners).await;
}

fn splash_widest(string: &str) -> usize {
    to_art(string.to_string(), "default", 0, 0, 0)
        .unwrap()
        .lines()
        .into_iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap()
}

async fn splash_with_params_and_banners(
    pre: usize,
    post: usize,
    interval: u64,
    banners: Vec<&str>,
) {
    for i in 0..banners.len() {
        let banner = banners.get(i).unwrap();
        match to_art(banner.to_string(), "default", 0, 0, 0) {
            Ok(string) => {
                let begin = (0xFF, 0xFF, 0xFF);
                let end = (0xEE, 0xAA, 0x5A);
                let end = COLORS;

                //let begin = (0x00, 0x00, 0x00);
                // this is bad code however I couldn't find out how to get lines().len() withou
                // giving up ownership (therefor the clone)
                let size = string.clone().lines().count();

                let mut index = 0;
                for line in string.lines() {
                    let progress = if index < (pre - 1) {
                        0.0f32
                    } else if index > pre && index < size - (post - 1) {
                        (index - (pre - 1)) as f32 / (size - (post - 1)) as f32
                    } else {
                        1.0f32
                    };

                    let r = (begin.0 as f32).lerp(end.0 as f32, progress) as u8;
                    let g = (begin.1 as f32).lerp(end.1 as f32, progress) as u8;
                    let b = (begin.2 as f32).lerp(end.2 as f32, progress) as u8;
                    println!("{}", line.truecolor(r, g, b));
                    tokio::time::sleep(Duration::from_millis(interval)).await;

                    index = index + 1;
                }

                //            println!("{}", string.truecolor(0xEE, 0xAA, 0x5A));
            }
            Err(err) => {
                eprintln!("err! {}", err.to_string());
            }
        }
    }
}

#[tokio::main]
async fn splash_html() {
    match to_art("*STARLANE*".to_string(), "default", 0, 0, 0) {
        Ok(string) => {
            let string = format!("\n\n\n\n\n\n{}\n\n\n\n\n\n", string).to_string();

            let begin = (0xFF, 0xFF, 0xFF);
            let end = (0xEE, 0xAA, 0x5A);
            let end = COLORS;

            //let begin = (0x00, 0x00, 0x00);
            // this is bad code however I couldn't find out how to get lines().len() withou
            // giving up ownership (therefor the clone)
            let size = string.clone().lines().count();
            let row_span = 1.0f32 / ((size - 10) as f32);
            let mut index = 0;
            for line in string.lines() {
                let progress = if index < 5 {
                    0.0f32
                } else if index > 6 && index < size - 5 {
                    (index - 5) as f32 / (size - 5) as f32
                } else {
                    1.0f32
                };
                let r = (begin.0 as f32).lerp(end.0 as f32, progress) as u32;
                let g = (begin.1 as f32).lerp(end.1 as f32, progress) as u32;
                let b = (begin.2 as f32).lerp(end.2 as f32, progress) as u32;

                let rgb = (r << 16) + (g << 8) + b;

                let color = format!("{:#0x}", rgb);
                println!("<div style=\"color:{}\">{}</div>", color, line);

                index = index + 1;
            }

            //            println!("{}", string.truecolor(0xEE, 0xAA, 0x5A));
        }
        Err(err) => {
            eprintln!("err! {}", err.to_string());
        }
    }
}

#[derive(ToBase)]
pub enum StartSequence {
    Starting(String),
}

#[tokio::main]
async fn install() -> Result<(), anyhow::Error> {

    intro("pre-install checklist")?;
    let spinner = spinner();
    spinner.start("checking configuration");

    match config().await {
        Ok(Some(_)) => {
            warning(format!("A valid starlane configuration already exists: '{}' this install process will overwrite the existing config", config_path() ))?;
            let should_continue = confirm(format!("Overwrite: '{}'?", config_path())).interact()?;
            if !should_continue {
                outro("Starlane installation aborted by user.")?;
                println!();
                println!();
                println!();
                process::exit(0);
            } else {
                spinner.start("deleting old config");
                fs::remove_file(config_path()).await?;
                info("config deleted.")?;
                spinner.clear();
            }
        }

        Err(err) => {
            warning(format!("An invalid (corrupted) starlane configuration already exists: '{}' the installation process will overwrite this config file.", config_path() )).unwrap_or_default();
            let should_continue = confirm("Proceed with installation?").interact()?;
            if !should_continue {
                outro("Starlane installation aborted by user.")?;
                println!();
                println!();
                println!();
                process::exit(0);
            } else {
                spinner.start("deleting invalid config");
                fs::remove_file(config_path()).await?;
                success("config deleted.")?;
                spinner.clear();
            }
        }
        Ok(None) => {
            // there's no config so proceed with install
            spinner.clear();
        }
    }

    outro("checklist complete.")?;

    print!("{}", "\n".repeat(3));
    delay(1000).await;


    intro("Install Starlane")?;

    success( "Select a foundation for this runner.  A foundation abstracts most infrastructure capabilities for starlane (provisioning servers, networking etc)" )?;
    //note("Foundations", "Select a foundation for this runner.  A foundation abstracts most infrastructure capabilities for starlane (provisioning servers, networking etc)" )?;
    note("Standalone", "If you are using Starlane to develop on your local machine the `Standalone` foundation is recommended and will get you going with minimal hastles")?;

    delay(500).await;
    let selected = select(r#"Choose a Foundation:"#)
        .item(
            "Standalone",
            "Standalone",
            wrap("Standalone is recommended for local development and if you are just getting started"),
        )
/*        .item(
            "Docker",
            "Install Starlane on Docker Desktop",
            "~~~ Kubernetes install isn't actually working in this demo!",
        )

 */
        .interact()?;

    match selected {
        "Standalone" => standalone_foundation().await,
        x => {
            error(format!("Sorry! this is just a proof of concept and the '{}' Foundation is not working just yet!",x))?;
            outro("Installation aborted because of lazy developers")?;
            process::exit(1);
            Ok(())
        }
    }
}

async fn standalone_foundation() -> Result<(), anyhow::Error> {
    let spinner = spinner();
    spinner.start("starting install");
    delay(1000).await;
    async fn inner(spinner: &ProgressBar) -> Result<(), anyhow::Error> {
        spinner.set_message("generating config");
        let config = StarlaneConfig::default();
        delay(100).await;
        spinner.clear();
        info("config generated.")?;
        delay(100).await;
        spinner.set_message("saving config");
        config_save(config.clone()).await?;
        delay(100).await;
        info("config saved.")?;
        delay(100).await;
        spinner.set_message("starting local postgres registry...");
        if let PgRegistryConfig::Embedded(db) = config.registry {
            let foundation = StandAloneFoundation::new();
            foundation.install(&db).await?;
        } else {

        }

        Ok(())
    }

    match inner(&spinner).await {
        Ok(()) => {
            spinner.stop("installation complete");
            Ok(())
        }
        Err(err) => {
            spinner.stop("installation failed");
            error(wrap(err.to_string().as_str())).unwrap_or_default();
            outro("Standalone installation failed").unwrap_or_default();
            process::exit(1);
        }
    }
}



/*
splash().await;

println!(
    "{}",
    "Let Starlane manage your infrastructure with WebAssembly & More!"
        .truecolor(COLORS.0, COLORS.1, crate::COLORS.2)
);
println!();
println!("{}", "Welcome to Starlane!".white());
println!();
println!();


 */





async fn delay(t: u64) {
    tokio::time::sleep(Duration::from_millis(t)).await;
}

async fn newlines( len: usize, delay: u64 )  {
    for i in 0..len {
        println!();
        crate::delay(delay).await;
    }
}



#[tokio::main]
async fn nuke()  {
    if let Ok(Some(config)) = config().await {
        if !config.can_nuke  {
            panic!("in config: '{}' can_nuke flag is set to false.",config_path());
        }
    }

    fs::remove_dir_all(STARLANE_HOME.to_string()).await.unwrap();
}