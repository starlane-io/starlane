use std::sync::Arc;
use cosmic_api::Artifacts;
use cosmic_api::quota::Timeouts;

#[derive(Clone)]
pub struct MachineSkel {
    pub artifacts: Arc<dyn Artifacts>,
    pub timeouts: Timeouts
}