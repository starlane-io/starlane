use std::sync::Arc;

use tokio::sync::mpsc;

use cosmic_hyperlane::HyperwayInterchange;
use cosmic_universe::log::PointLogger;
use cosmic_universe::particle::traversal::Traversal;
use cosmic_universe::wave::{UltraWave, Wave};

#[derive(Clone)]
pub struct HostSkel {
    pub logger: PointLogger,
    pub fabric: Arc<HyperwayInterchange>,
    pub towards_core: mpsc::Sender<Traversal<UltraWave>>,
}
