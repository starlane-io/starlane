#![allow(warnings)]
#![feature()]


#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate starlane_macros;shadow!(build);


pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());

#[cfg(test)]
pub mod test;

pub mod install;

pub mod env;
pub mod server;




pub mod cli;


pub mod env;

pub mod server;




pub use starlane_hyperspace::platform::Platform;
use starlane_hyperspace::shutdown::shutdown;
use crate::install::{Console, StarlaneTheme};
use crate::server::Starlane;
use starlane_space::err::PrintErr;
use starlane_space::loc::ToBaseKind;
use starlane_space::log::push_scope;
use starlane_space::parse::SkewerCase;
use starlane_space::particle::Status;
use anyhow::{anyhow, ensure};
use clap::Parser;
use cliclack::log::{error, success};
use cliclack::{intro, outro, spinner};
use colored::Colorize;
use crossterm::execute;
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor, Stylize};
use lerp::Lerp;
use nom::{InputIter, InputTake, Slice};
use once_cell::sync::Lazy;
use shadow_rs::shadow;
use starlane_macros::{create_mark, ToBase};
use std::any::Any;
use std::fmt::Display;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::ops::{Add, Index, Mul};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use std::{io, process};
use tokio::fs::DirEntry;
use tokio::runtime::Builder;
use tracing::instrument::WithSubscriber;
use tracing::Instrument;
use zip::write::{FileOptionExtension, FileOptions};
use crate::env::{context_dir, ensure_global_settings, save_global_settings, STARLANE_HOME};
/*
let config = Default::default();

config

 */

pub fn init() {

    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider()
            .install_default()
            .expect("crypto provider could not be installed");
    }
}


pub fn main() -> Result<(), anyhow::Error> {
    ctrlc::set_handler(move || shutdown(1)).unwrap();

    init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Splash => {
            let console = Console::new();
            console.splash2();
            Ok(())
        }
        Commands::Install { edit, nuke } => {
            if nuke {
                crate::nuke(false);
            }
            install::install(edit)
        }
        Commands::Run => {
            let runtime = Builder::new_multi_thread().enable_all().build()?;
            runtime.block_on(async move { push_scope(run, create_mark!()).await });
            Ok(())
        }
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
        Commands::Scorch => {
            scorch();
            Ok(())
        }
        Commands::Nuke { all } => {
            nuke(all);
            Ok(())
        }
        Commands::Context(args) => {
            match args.command {
                ContextCmd::Create { context_name } => {
                    let context_name =
                        SkewerCase::from_str(context_name.as_str()).map_err(|e| {
                            e.print();
                            anyhow!("illegal context name")
                        })?;
                    set_context(context_name.as_str())?;
                    if config_exists(context_name.to_string()) {
                        Err(anyhow!("context '{}' already exists", context_name))?;
                    }

                    println!(
                        "Context '{}' created.  Next you may want to run '{}'",
                        context_name.truecolor(COOL.0, COOL.1, COOL.2),
                        "starlane install"
                            .to_string()
                            .truecolor(COOL.0, COOL.1, COOL.2)
                    );
                }
                ContextCmd::Switch { context_name } => {
                    let context_name =
                        SkewerCase::from_str(context_name.as_str()).map_err(|e| {
                            e.print();
                            anyhow!("illegal context name")
                        })?;
                    set_context(context_name.as_str());
                }
                ContextCmd::Default => {
                    set_context("default").unwrap_or_default();
                }
                ContextCmd::Which => {
                    println!("{}", context());
                }
                ContextCmd::List => {
                    let context = context();
                    let dir = std::fs::read_dir(STARLANE_HOME.to_string())?;
                    for dir in dir.into_iter() {
                        let dir = dir?;
                        if dir.metadata()?.is_dir() {
                            let dir = dir
                                .path()
                                .iter()
                                .last()
                                .expect("expected a last directory")
                                .to_str()
                                .unwrap_or_default()
                                .to_string();
                            if context == dir {
                                println!("{}{}", "*", dir.truecolor(0xff, 0xff, 0xff));
                            } else {
                                println!(" {}", dir.truecolor(COOL.0, COOL.1, COOL.2));
                            }
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

#[cfg(not(feature = "server"))]
fn run() -> Result<(), anyhow::Error> {
    println!("'' feature is not enabled in this main installation");
    Err(anyhow!(
        "'machine' feature is not enabled in this main installation"
    ))
}

async fn run() -> Result<(), anyhow::Error> {
    let console = Console::new();
    console.info("main started.")?;

    console.intro("RUN STARLANE").unwrap_or_default();

    async fn runner(console: &Console) -> Result<(), anyhow::Error> {
        console.spinner().start("initializing");
        console.info("initialization complete.")?;

        console.long_delay();
        let mut spinner = console.spinner();
        spinner.start("loading configuration");

        let config = match env::config() {
            Ok(Some(config)) => config,
            Ok(None) => {
                console.long_delay();
                spinner.error("Starlane configuration not found.");

                error(format!(
                    "Starlane looked for a configuration here: '{}' But none was found.",
                    env::config_path()
                ))?;

                console.remark(format!("if '{}' isn't the config file you wanted, please set environment variable `export STARLANE_HOME=\"/config/parent/dir\"", env::config_path()))?;

                console.note(
                    "install",
                    "please run `main install` to configure a new Starlane runner",
                )?;

                outro("Good Luck!")?;
                console.newlines(3);
                shutdown(1);
                panic!();
            }
            Err(err) => {
                spinner.error("invalid configuration");
                console.error(format!("{}", err.to_string()))?;
                console.note("wrong config?", format!("if '{}' isn't the config file you wanted, please set environment variable `export STARLANE_HOME=\"/config/parent/dir\"", env::config_path()))?;
                console.note(
                    "fresh install",
                    "To create a fresh configuration please run: `main install`",
                )?;
                outro("Good Luck!")?;
                console.newlines(3);
                shutdown(1);
                panic!();
            }
        };

        console.long_delay();
        console.success("main configured.")?;
        spinner.next(
            "configuration loaded.",
            "launching registry [this may take a while]",
        );

        console.long_delay();
        let starlane = Starlane::new(config, StandAloneFoundation())
            .await
            .map_err(|e| {
                println!("{}", e.to_string());
                e
            })
            .unwrap();


        spinner.next("registry status: [Ready]", "acquiring machine API");

        let machine_api = starlane.machine();

        spinner.next(
            "machine API acquired",
            "waiting for Starlane [Ready] status",
        );

        let api = tokio::time::timeout(Duration::from_secs(30), machine_api).await??;

        spinner.clear();

        console.status("Starlane status:", Status::Ready)?;

        console.newlines(3);

        console.splash_with_params(1, 2, 25);

        console.note(
            "what's next?",
            "You can connect to this main runner with a control terminal: `main term`",
        )?;

        console.outro("Starlane is running.")?;
        console.newlines(3);

        // this is a dirty hack which is good enough for a 0.3.0 release...
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }

        Ok(())
    }
    ;

    match runner(&console).await {
        Ok(_) => {}
        Err(err) => {
            error(format!(
                "main halted due to an error: {}",
                err.to_string()
            ))
                .unwrap_or_default();

            console.outro("runner failed").unwrap();
            console.newlines(3);
        }
    }

    Ok(())
}
/*

fn run() -> Result<(), anyhow::Error> {
    let console = Console::new();
    console.info("main started.")?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    runtime.block_on(async move {

        log_entry_point()
        console.intro("RUN STARLANE").unwrap_or_default();

        async fn runner(console: &Console) -> Result<(),anyhow::Error> {
            console.spinner().start("initializing");
            console.info("initialization complete.")?;

            console.long_delay();
            let mut spinner = console.spinner();
            spinner.start("loading configuration");

            let config = match env::config() {
                Ok(Some(config)) => config,
                Ok(None) => {
                    console.long_delay();
                    spinner.error("Starlane configuration not found.");

                    error(format!("Starlane looked for a configuration here: '{}' But none was found.", env::config_path()))?;

                    console.remark(format!("if '{}' isn't the config file you wanted, please set environment variable `export STARLANE_HOME=\"/config/parent/dir\"", env::config_path()))?;

                    console.note("install", "please run `main install` to configure a new Starlane runner")?;

                    outro("Good Luck!")?;
                    console.newlines(3);
                    shutdown(1);
                    panic!();
                }
                Err(err) => {
                    spinner.error("invalid configuration");
                    console.error(format!("{}", err.to_string()))?;
                    console.note("wrong config?", format!("if '{}' isn't the config file you wanted, please set environment variable `export STARLANE_HOME=\"/config/parent/dir\"", env::config_path()))?;
                    console.note("fresh install", "To create a fresh configuration please run: `main install`")?;
                    outro("Good Luck!")?;
                    console.newlines(3);
                    shutdown(1);
                    panic!();
                }
            };

            console.long_delay();
            console.success("main configured.")?;
            spinner.next("configuration loaded.","launching registry [this may take a while]");

            console.long_delay();
            let main = Starlane::new(config,StandAloneFoundation()).await.map_err(|e|{println!("{}",e.to_string()); e}).unwrap();

            spinner.next("registry status: [Ready]","acquiring machine API");

            let machine_api = main.machine();


            spinner.next("machine API acquired","waiting for Starlane [Ready] status");

            let api = tokio::time::timeout(Duration::from_secs(30), machine_api).await??;

            spinner.clear();

            console.status( "Starlane status:", Status::Ready)?;

            console.newlines(3);

            console.splash_with_params(1, 2, 25);

            console.note("what's next?", "You can connect to this main runner with a control terminal: `main term`" )?;

            console.outro("Starlane is running.")?;
            console.newlines(3);

            // this is a dirty hack which is good enough for a 0.3.0 release...
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
            }

            Ok(())
        };

        match runner(&console).await {
            Ok(_) => {}
            Err(err) =>
                {
                    error(format!("main halted due to an error: {}", err.to_string())).unwrap_or_default();

                    console.outro("runner failed").unwrap();
                    console.newlines(3);
                }
        }
    });

    Ok(())
}

 */

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
    it: impl Iterator<Item=DirEntry>,
    prefix: &str,
    writer: T,
    method: zip::CompressionMethod,
) -> zip::result::ZipResult<T>
where
    T: Write + Seek,
{
    let mut zip = zip::ZipWriter::new(writer);
    let options: FileOptions<'_,FileOptionExtension> = FileOptions::default()
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

static COOL: (u8, u8, u8) = (0x6D, 0xD7, 0xFD);
static UNDERSTATED: (u8, u8, u8) = (0x66, 0x66, 0xFD);

static IMPORTANT: (u8, u8, u8) = (0xFF, 0xFF, 0xFF);
static ERR: (u8, u8, u8) = (0xcc, 0x00, 0x00);

static OK: (u8, u8, u8) = (0x00, 0xcc, 0x00);
#[derive(ToBase)]
pub enum StartSequence {
    Starting(String),
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

#[tokio::main]
async fn scorch() {
    if let Ok(Some(config)) = env::config() {
        if !config.can_scorch {
            panic!(
                "in config: '{}' can_scorch flag is set to false.",
                env::config_path()
            );
        }
    }
}

fn nuke(all: bool) {
    if all {
        let global = ensure_global_settings();
        if global.nuke {
            std::fs::remove_dir_all(STARLANE_HOME.as_str()).unwrap();
            println!("all main contexts deleted");
            // saving the global.conf again
            save_global_settings(global).unwrap();
        }
    }

    if let Ok(Some(config)) = env::config() {
        if !config.can_nuke {
            panic!(
                "in config: '{}' can_nuke flag is set to false.",
                env::config_path()
            );
        } else {
            std::fs::remove_dir_all(context_dir()).unwrap();
        }
    }
}

/*
fn list_contexts() -> Result<Vec<String>,anyhow::Error> {
    let mut rtn = vec![];
    let dir = std::fs::read_dir(STARLANE_HOME.to_string())?;
    for dir in dir.into_iter() {
        let dir = dir?;
        if dir.metadata()?.is_dir() {
            let dir : String= dir.path().into();
            rtn.push(dir);
        }
    }
    Ok(rtn)
}

 */
