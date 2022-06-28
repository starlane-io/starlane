use crate::field::FieldState;
use crate::portal::PortalInlet;
use dashmap::DashSet;
use mesh_portal_versions::version::v0_0_1::id::id::{Point, Uuid};
use mesh_portal_versions::version::v0_0_1::particle::particle::Details;
use std::sync::Arc;
use mesh_portal::version::latest::payload::Substance;
use serde::{Serialize,Deserialize};
use mesh_portal::version::latest::id::Port;

/// includes states for layers [ Field, Shell & Driver ]
#[derive(Clone)]
pub struct ParticleStates {
    pub field: FieldState,
    pub shell: ShellState,
    pub driver: DriverState,
    pub portal_inlet: Option<PortalInletState>,
    pub tunnel: Option<TunnelState>,
}

impl ParticleStates {
    pub fn new() -> Self {
        Self {
            field: FieldState::new(),
            shell: ShellState::new(),
            driver: DriverState::None,
            portal_inlet: None,
            tunnel: None,
        }
    }

    pub fn portal_inlet(&mut self, locality: &Point) -> PortalInletState {
        match &self.portal_inlet {
            None => {
                let state = PortalInletState::new();
                self.portal_inlet.replace(state.clone());
                state
            }
            Some(state) => state.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ShellState {
    pub port: Port,
    pub core_requests: Arc<DashSet<Uuid>>,
    pub fabric_requests: Arc<DashSet<Uuid>>,
}

impl ShellState {
    pub fn new(port: Port) -> Self {
        Self {
            port,
            core_requests: Arc::new(DashSet::new()),
            fabric_requests: Arc::new(DashSet::new()),
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum DriverState {
    None,
    Substance(Substance)
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
