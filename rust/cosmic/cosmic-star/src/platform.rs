use std::sync::Arc;
use cosmic_api::id::StarSub;
use cosmic_api::{Artifacts, RegistryApi};
use cosmic_api::substance::substance::Token;
use cosmic_driver::DriverFactory;
use crate::driver::DriversBuilder;

pub trait Platform: Send+Sync {
    fn drivers_builder( &self, kind: &StarSub ) -> DriversBuilder;
    fn token(&self) -> Token;
    fn registry(&self) -> Arc<dyn RegistryApi>;
    fn artifacts(&self) -> Arc<dyn Artifacts>;
}