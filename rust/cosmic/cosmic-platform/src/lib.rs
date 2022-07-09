#![allow(warnings)]

#[macro_use]
extern crate cosmic_macros;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;

use cosmic_api::id::StarSub;
use cosmic_api::{ArtifactApi, PlatformErr, RegistryApi};
use cosmic_api::substance::substance::Token;
use std::sync::Arc;
use cosmic_api::command::request::create::KindTemplate;
use cosmic_api::id::id::Kind;
use cosmic_hyperlane::InterchangeEntryRouter;
use crate::driver::DriversBuilder;

pub mod driver;
pub mod field;
pub mod host;
pub mod machine;
pub mod shell;
pub mod star;
pub mod state;
pub mod traversal;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}

pub trait Platform<E>: Send+Sync where E: PlatformErr {
    fn drivers_builder( &self, kind: &StarSub ) -> DriversBuilder;
    fn token(&self) -> Token;
    fn registry(&self) -> Arc<dyn RegistryApi<E>>;
    fn artifacts(&self) -> Arc<dyn ArtifactApi>;
    fn start_services(&self, entry_router: & mut InterchangeEntryRouter );
    fn default_implementation(template: &KindTemplate) -> Result<Kind, PostErr>;
}
