use crate::guest::GuestSkel;
use crate::host::HostSkel;
use crate::star::StarSkel;
use dashmap::DashMap;
use mesh_portal_versions::version::v0_0_1::id::id::{Layer, Port, TraversalLayer, Uuid};
use mesh_portal_versions::version::v0_0_1::id::Traversal;
use mesh_portal_versions::version::v0_0_1::wave::{Exchanger, Ping, Pong, UltraWave, Wave};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot::Sender;
use cosmic_hyperlane::HyperwayInterchange;
use crate::state::{PortalInletState, PortalOutletState, PortalShellState, TunnelState};

/// the portal endpoint that is within the Fabric
pub struct PortalInlet {
    pub skel: StarSkel,
    pub state: PortalInletState,
}

impl PortalInlet {
    pub fn new(skel: StarSkel, state: PortalInletState) -> Self {
        Self { skel, state }
    }
}

#[async_trait]
impl TraversalLayer for PortalInlet {
    fn port(&self) -> &Port {
        todo!()
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        todo!()
    }

    async fn inject(&self, wave: UltraWave) {
        todo!()
    }

    fn exchanger(&self) -> &Arc<DashMap<Uuid, Sender<Pong>>> {
        todo!()
    }

    async fn delivery_directed(&self, request: Ping) {
        todo!()
    }
}

/// The mid-portion of the Portal [Between Inlet & Outlet]
pub struct TunnelOutlet {
    state: TunnelState,
    pub skel: StarSkel,
    towards_core: Arc<HyperwayInterchange>,
}

impl TunnelOutlet {
    pub fn new(skel: StarSkel, towards_core: Arc<HyperwayInterchange>, state: TunnelState) -> Self {
        Self {
            skel,
            towards_core,
            state,
        }
    }
}

#[async_trait]
impl TraversalLayer for TunnelOutlet {
    fn port(&self) -> &Port {
        todo!()
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        todo!()
    }

    async fn inject(&self, wave: UltraWave) {
        todo!()
    }

    fn exchanger(&self) -> &Arc<DashMap<Uuid, Sender<Pong>>> {
        todo!()
    }

}

pub struct TunnelInlet {
    skel: HostSkel,
    state: TunnelState,
}

impl TunnelInlet {
    pub fn new(skel: HostSkel, state: TunnelState) -> Self {
        Self { skel, state }
    }
}

#[async_trait]
impl TraversalLayer for TunnelInlet {
    fn port(&self) -> &Port {
        todo!()
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        todo!()
    }

    async fn inject(&self, wave: UltraWave) {
        todo!()
    }

    fn exchanger(&self) -> &Arc<DashMap<Uuid, Sender<Pong>>> {
        todo!()
    }

    async fn delivery_directed(&self, request: Ping) {
        todo!()
    }
}

pub struct PortalOutlet {
    pub skel: GuestSkel,
    pub state: PortalOutletState,
}

impl PortalOutlet {
    pub fn new(skel: StarSkel, state: PortalOutletState) -> Self {
        let skel = GuestSkel::from_star_skel(&skel);
        Self { skel, state }
    }
}

#[async_trait]
impl TraversalLayer for PortalOutlet {
    fn port(&self) -> &Port {
        todo!()
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        todo!()
    }

    async fn inject(&self, wave: UltraWave) {
        todo!()
    }

    fn exchanger(&self) -> &Exchanger {
        todo!()
    }

    async fn delivery_directed(&self, request: Ping) {
        todo!()
    }
}

pub struct PortalShell {
    pub skel: StarSkel,
    pub state: PortalShellState,
}

impl PortalShell {
    pub fn new(skel: StarSkel, state: PortalShellState) -> Self {
        Self { skel, state }
    }
}

#[async_trait]
impl TraversalLayer for PortalShell {
    fn port(&self) -> &Port {
        todo!()
    }

    async fn traverse_next(&self, traversal: Traversal<UltraWave>) {
        todo!()
    }

    async fn inject(&self, wave: UltraWave) {
        todo!()
    }

    fn exchanger(&self) -> &Exchanger {
        todo!()
    }

    async fn delivery_directed(&self, request: Ping) {
        todo!()
    }
}
