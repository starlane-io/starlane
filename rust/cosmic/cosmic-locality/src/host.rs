use std::sync::Arc;
use tokio::sync::mpsc;
use cosmic_hyperlane::HyperwayInterchange;
use mesh_portal_versions::version::v0_0_1::id::Traversal;
use mesh_portal_versions::version::v0_0_1::log::PointLogger;
use mesh_portal_versions::version::v0_0_1::wave::Wave;

#[derive(Clone)]
pub struct HostSkel
{
    pub logger: PointLogger,
    pub fabric : Arc<HyperwayInterchange>,
    pub towards_core: mpsc::Sender<Traversal<Wave>>
}