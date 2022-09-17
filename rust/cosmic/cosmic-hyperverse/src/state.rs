use cosmic_universe::id::id;
use cosmic_universe::id::id::{Point, Uuid};
use cosmic_universe::particle::particle::Details;
use cosmic_universe::wave::WaveId;
use cosmic_universe::state::State;
use dashmap::{DashMap, DashSet};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::AtomicU16;

#[derive(Clone)]
pub struct ShellState {
    pub point: Point,
    pub core_requests: Arc<DashSet<WaveId>>,
    pub fabric_requests: Arc<DashMap<WaveId,AtomicU16>>,
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
