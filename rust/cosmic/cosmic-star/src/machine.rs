use std::sync::Arc;
use mesh_portal_versions::Artifacts;
use mesh_portal_versions::version::v0_0_1::quota::Timeouts;

#[derive(Clone)]
pub struct MachineSkel {
    pub artifacts: Arc<dyn Artifacts>,
    pub timeouts: Timeouts
}