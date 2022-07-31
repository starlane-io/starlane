use crate::field::FieldState;
use cosmic_api::id::id;
use cosmic_api::id::id::{Point, Uuid};
use cosmic_api::particle::particle::Details;
use cosmic_api::wave::WaveId;
use cosmic_api::State;
use dashmap::DashSet;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
pub struct ShellState {
    pub port: id::Port,
    pub core_requests: Arc<DashSet<WaveId>>,
    pub fabric_requests: Arc<DashSet<WaveId>>,
}

impl ShellState {
    pub fn new(port: id::Port) -> Self {
        Self {
            port,
            core_requests: Arc::new(DashSet::new()),
            fabric_requests: Arc::new(DashSet::new()),
        }
    }
}
