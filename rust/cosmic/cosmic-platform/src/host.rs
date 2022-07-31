use cosmic_api::id::Traversal;
use cosmic_api::log::PointLogger;
use cosmic_api::wave::{UltraWave, Wave};
use cosmic_hyperlane::HyperwayInterchange;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct HostSkel {
    pub logger: PointLogger,
    pub fabric: Arc<HyperwayInterchange>,
    pub towards_core: mpsc::Sender<Traversal<UltraWave>>,
}
