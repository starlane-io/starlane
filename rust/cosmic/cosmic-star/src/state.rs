use cosmic_api::State;
use crate::portal::PortalInlet;
use dashmap::DashSet;
use cosmic_api::version::v0_0_1::id::id::{Point, Uuid};
use cosmic_api::version::v0_0_1::particle::particle::Details;
use std::sync::Arc;
use mesh_portal::version::latest::payload::Substance;
use serde::{Deserialize, Serialize};
use mesh_portal::version::latest::id::Port;
use cosmic_api::version::v0_0_1::id::id;
use cosmic_api::version::v0_0_1::wave::WaveId;
use crate::field::FieldState;

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

#[derive(Clone)]
pub struct TunnelState {}

impl TunnelState {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Clone)]
pub struct PortalInletState {}

impl PortalInletState {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Clone)]
pub struct PortalOutletState {}

impl PortalOutletState {
    pub fn new() -> Self {
        Self {}
    }
}

#[derive(Clone)]
pub struct PortalShellState {}

impl PortalShellState {
    pub fn new() -> Self {
        Self {}
    }
}
