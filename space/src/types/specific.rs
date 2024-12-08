use std::collections::HashMap;
use std::hash::Hash;
use serde::{Deserialize, Serialize};
use crate::kind::Specific;
use crate::point::Point;
use crate::types::class::ClassKind;
use crate::types::private::MetaDef;
use crate::types::SchemaKind;

#[derive(Clone, Serialize, Deserialize)]
pub struct SpecificMeta {
    pub specific: Specific,
    pub type_defs: TypeDefs
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeDefs {
    pub schema: HashMap<SchemaKind, MetaDef<SchemaKind>>,
    pub schema2: HashMap<ClassKind, MetaDef<ClassKind>>,
}


