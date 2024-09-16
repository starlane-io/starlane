#![allow(warnings)]
pub mod err;
pub mod properties;


#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate lazy_static;


#[cfg(feature="server")]
pub mod server;
#[cfg(feature="hyperspace")]
pub mod hyperspace;
mod registry;

use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

use cosmic_hyperlane::HyperGate;


#[cfg(feature = "postgres")]
use crate::registry::postgres::{ err::PostErr, PostgresPlatform };

use cosmic_space::loc::ToBaseKind;
use crate::err::StarErr;
use crate::hyperspace::lib::Cosmos;
use crate::server::Starlane;

fn main() -> Result<(), StarErr> {
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

#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
