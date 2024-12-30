use crate::kind::Specific;
use crate::types::class::ClassKind;
use crate::types::SchemaKind;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;

#[derive(Clone, Serialize, Deserialize)]
pub struct SpecificMeta {
    pub specific: Specific,
    pub type_defs: TypeDefs
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeDefs {
    pub schema: HashMap<SchemaKind, MetaDefs>,
}


