use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use cosmic_api::id::StarSub;
use cosmic_api::{ArtifactApi, RegistryApi};
use cosmic_api::substance::substance::Token;
use cosmic_artifact::Artifacts;
use cosmic_registry_postgres::Registry;
use cosmic_star::driver::DriversBuilder;
use cosmic_star::platform::Platform;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref STARLANE_PORT: usize = std::env::var("STARLANE_PORT").unwrap_or("4343".to_string()).parse::<usize>().unwrap_or(4343);
    pub static ref STARLANE_DATA_DIR: String= std::env::var("STARLANE_DATA_DIR").unwrap_or("data".to_string());
    pub static ref STARLANE_CACHE_DIR: String = std::env::var("STARLANE_CACHE_DIR").unwrap_or("data".to_string());
    pub static ref STARLANE_TOKEN: String = std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string());
}
#[no_mangle]
pub extern "C" fn cosmic_uuid() -> String
{
    Uuid::new_v4().to_string()
}


#[no_mangle]
pub extern "C" fn cosmic_timestamp() -> DateTime<Utc>{
    Utc::now()
}


fn main() {
    println!("Hello, world!");
}

pub struct Starlane {
   registry: Arc<Registry>,
   artifacts: Arc<Artifacts>
}

impl Platform for Starlane {
    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder {
        match kind {
            StarSub::Central => {}
            StarSub::Super => {}
            StarSub::Nexus => {}
            StarSub::Maelstrom => {}
            StarSub::Scribe => {}
            StarSub::Jump => {}
            StarSub::Fold => {}
        }
        DriversBuilder::new()
    }

    fn token(&self) -> Token {
        Token::new(STARLANE_TOKEN.to_string())
    }

    fn registry<E>(&self) -> Arc<dyn RegistryApi<E>> where E: cosmic_api::CosmicErr {
        self.registry.clone()
    }

    fn artifacts(&self) -> Arc<dyn ArtifactApi> {
       self.artifacts.clone()
    }

    fn start_services(&self, entry_router: &mut cosmic_hyperlane::InterchangeEntryRouter) {
        todo!()
    }
}