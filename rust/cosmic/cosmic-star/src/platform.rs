use std::sync::Arc;
use cosmic_api::id::StarSub;
use cosmic_api::{ArtifactApi, RegistryApi};
use cosmic_api::substance::substance::Token;
use cosmic_driver::DriverFactory;
use cosmic_hyperlane::InterchangeEntryRouter;
use crate::driver::DriversBuilder;

pub trait Platform: Send+Sync {
    fn drivers_builder( &self, kind: &StarSub ) -> DriversBuilder;
    fn token(&self) -> Token;
    fn registry<E>(&self) -> Arc<dyn RegistryApi<E>>;
    fn artifacts(&self) -> Arc<dyn ArtifactApi>;
    fn start_services(&self, entry_router: & mut InterchangeEntryRouter );
}