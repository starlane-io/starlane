use cosmic_universe::id::Traversal;
use cosmic_universe::log::PointLogger;
use cosmic_universe::wave::{UltraWave, Wave};
use cosmic_hyperlane::HyperwayInterchange;
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct HostSkel {
    pub logger: PointLogger,
    pub fabric: Arc<HyperwayInterchange>,
    pub towards_core: mpsc::Sender<Traversal<UltraWave>>,
}
