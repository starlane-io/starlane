use std::sync::Arc;
use dashmap::DashMap;
use mesh_portal_versions::version::v0_0_1::id::id::{Layer, Point, ToPoint};
use mesh_portal_versions::version::v0_0_1::id::Traversal;
use mesh_portal_versions::version::v0_0_1::wave::Wave;
use crate::portal::PortalOutlet;

#[derive(Clone)]
pub struct GuestSkel {
    pub portals: Arc<DashMap<Point,PortalOutlet>>
}

impl GuestSkel {
    pub fn towards_fabric( &self, mut traversal: Traversal<Wave> ) {
        traversal.layer = Layer::Guest;
        todo!() // somehow send to HOST
    }

    pub fn towards_core( &self, mut traversal: Traversal<Wave> ) {
        traversal.layer = Layer::Guest;
        match self.portals.get(&traversal.to().clone().to_point() ) {
            Some(outlet) => {
                outlet.value().towards_core(traversal)
            }
            None => {}
        }
    }
}