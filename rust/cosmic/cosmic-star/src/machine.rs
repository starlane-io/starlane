use std::sync::Arc;
use cosmic_api::Artifacts;
use cosmic_api::version::v0_0_1::quota::Timeouts;

#[derive(Clone)]
pub struct MachineSkel {
    pub artifacts: Arc<dyn Artifacts>,
    pub timeouts: Timeouts
}