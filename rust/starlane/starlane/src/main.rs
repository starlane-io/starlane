#![allow(warnings)]
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;
pub mod err;
pub mod properties;

#[cfg(feature="space")]
pub mod space;

#[cfg(feature = "hyperspace")]
pub mod hyper;
pub mod registry;
#[cfg(feature = "server")]
pub mod server;

use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;


use crate::err::StarErr;
use self::hyper::space::lib::Cosmos;
use crate::server::Starlane;
use cosmic_space::loc::ToBaseKind;
use std::io::{Read, Seek, Write};
use std::path::Path;
use std::fs::File;
use tokio::fs::DirEntry;
use zip::write::FileOptions;
use cosmic_space::err::SpaceErr;

#[cfg(feature = "server")]
fn server() -> Result<(), StarErr> {
    ctrlc::set_handler(move || {
        std::process::exit(1);
    });

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let starlane = Starlane::new().await.unwrap();
        let machine_api = starlane.machine();
        tokio::time::timeout(Duration::from_secs(30), machine_api.wait_ready())
            .await
            .unwrap();
        println!("> STARLANE Ready!");
        // this is a dirty hack which is good enough for a 0.3.0 release...
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
        let cl = machine_api.clone();
        machine_api.await_termination().await.unwrap();
        cl.terminate();
    });
    Ok(())
}

lazy_static! {
    pub static ref STARLANE_CONTROL_PORT: u16 = std::env::var("STARLANE_PORT")
        .unwrap_or("4343".to_string())
        .parse::<u16>()
        .unwrap_or(4343);
    pub static ref STARLANE_DATA_DIR: String =
        std::env::var("STARLANE_DATA_DIR").unwrap_or("./data/".to_string());
    pub static ref STARLANE_CACHE_DIR: String =
        std::env::var("STARLANE_CACHE_DIR").unwrap_or("cache".to_string());
    pub static ref STARLANE_TOKEN: String =
        std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string());
    pub static ref STARLANE_REGISTRY_URL: String =
        std::env::var("STARLANE_REGISTRY_URL").unwrap_or("localhost".to_string());
    pub static ref STARLANE_REGISTRY_USER: String =
        std::env::var("STARLANE_REGISTRY_USER").unwrap_or("postgres".to_string());
    pub static ref STARLANE_REGISTRY_PASSWORD: String =
        std::env::var("STARLANE_REGISTRY_PASSWORD").unwrap_or("password".to_string());
    pub static ref STARLANE_REGISTRY_DATABASE: String =
        std::env::var("STARLANE_REGISTRY_DATABASE").unwrap_or("postgres".to_string());
}

/*
#[no_mangle]
pub extern "C" fn cosmic_uuid() -> loc::Uuid {
    loc::Uuid::from(uuid::Uuid::new_v4()).unwrap()
}

#[no_mangle]
pub extern "C" fn cosmic_timestamp() -> Timestamp {
    Timestamp { millis: Utc::now().timestamp_millis() }
}

 */
#[tokio::main]
async fn main() -> Result<(),()> {
    Ok(())
}

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}

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
                .default_value(format!("{}/.starlane/localhost/certs", home_dir).as_str()),
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