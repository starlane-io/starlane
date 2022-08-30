use cosmic_api::id::id;
use cosmic_api::id::id::{Point, Uuid};
use cosmic_api::particle::particle::Details;
use cosmic_api::wave::WaveId;
use cosmic_api::State;
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
