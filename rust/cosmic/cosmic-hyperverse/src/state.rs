use cosmic_universe::loc::Point;
use cosmic_universe::loc::Uuid;
use cosmic_universe::particle::Details;
use cosmic_universe::wave::WaveId;
use dashmap::{DashMap, DashSet};
use serde::{Deserialize, Serialize};
use std::sync::atomic::AtomicU16;
use std::sync::Arc;

#[derive(Clone)]
pub struct ShellState {
    pub point: Point,
    pub core_requests: Arc<DashSet<WaveId>>,
    pub fabric_requests: Arc<DashMap<WaveId, AtomicU16>>,
}

impl ShellState {
    pub fn new(point: Point) -> Self {
        Self {
            point,
            core_requests: Arc::new(DashSet::new()),
            fabric_requests: Arc::new(DashMap::new()),
        }
    }
}
