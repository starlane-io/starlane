use std::collections::HashMap;
use std::sync::Arc;

use starlane_resources::ResourceIdentifier;

use crate::data::{BinSrc, DataSet};
use crate::message::Fail;
use crate::resource::{ArtifactKind, AssignResourceStateSrc, LocalStateSetSrc, Names, RemoteDataSrc, Resource, ResourceAddress, ResourceArchetype, ResourceAssign, ResourceKey, ResourceKind};
use crate::resource::state_store::StateStore;
use crate::star::core::component::resource::host::Host;
use crate::star::StarSkel;

