use std::collections::hash_map::RandomState;
use std::collections::HashMap;

use mesh_portal_serde::version::v0_0_1::generic::resource::command::common::StateSrc;

use crate::error::Error;
use crate::mesh::serde::id::Address;
use crate::resource::{AssignResourceStateSrc, Kind, ResourceAssign, ResourceType};
use crate::star::core::resource::manager::ResourceManager;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;

