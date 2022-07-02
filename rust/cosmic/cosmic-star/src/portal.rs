use crate::host::HostSkel;
use crate::star::StarSkel;
use crate::state::{PortalInletState, PortalOutletState, PortalShellState, TunnelState};
use cosmic_hyperlane::HyperwayInterchange;
use dashmap::DashMap;
use cosmic_api::id::id::{Layer, Port, TraversalLayer, Uuid};
use cosmic_api::id::Traversal;
use cosmic_api::wave::{
    DirectedWave, Exchanger, Ping, Pong, UltraWave, Wave,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::oneshot::Sender;

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

    fn exchanger(&self) -> &Exchanger {
        todo!()
    }

    async fn delivery_directed(&self, direct: Traversal<DirectedWave>) {}
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

    fn exchanger(&self) -> &Exchanger{
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

    fn exchanger(&self) -> &Exchanger {
        todo!()
    }

    async fn delivery_directed(&self, traversal: Traversal<DirectedWave>) {
        todo!()
    }
}

pub struct PortalOutlet {
    pub skel: StarSkel,
    pub state: PortalOutletState,
}

impl PortalOutlet {
    pub fn new(skel: StarSkel, state: PortalOutletState) -> Self {
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


    async fn delivery_directed(&self, traversal: Traversal<DirectedWave>) {
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

    async fn delivery_directed(&self, traversal: Traversal<DirectedWave>) {
        todo!()
    }
}
