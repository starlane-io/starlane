use std::collections::HashMap;
use std::hash::Hash;
use serde::{Deserialize, Serialize};
use crate::kind::Specific;
use crate::types::class::ClassKind;
use crate::types::private::MetaDefs;
use crate::types::SchemaKind;

#[derive(Clone, Serialize, Deserialize)]
pub struct SpecificMeta {
    pub specific: Specific,
    pub type_defs: TypeDefs
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeDefs {
    pub schema: HashMap<SchemaKind, MetaDefs>,
    pub schema2: HashMap<ClassKind, MetaDefs>,
}


