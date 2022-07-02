use std::sync::Arc;
use tokio::sync::mpsc;
use cosmic_hyperlane::HyperwayInterchange;
use cosmic_api::version::v0_0_1::id::Traversal;
use cosmic_api::version::v0_0_1::log::PointLogger;
use cosmic_api::version::v0_0_1::wave::{UltraWave, Wave};

#[derive(Clone)]
pub struct HostSkel
{
    pub logger: PointLogger,
    pub fabric : Arc<HyperwayInterchange>,
    pub towards_core: mpsc::Sender<Traversal<UltraWave>>
}