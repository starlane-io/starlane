use std::collections::HashMap;
use std::hash::Hash;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, ToString};
use crate::kind::Specific;
use crate::point::Point;
use crate::types::class::Class;
use crate::types::private::MetaDef;
use crate::types::SchemaKind;

pub struct SpecificMeta {
    pub specific: Specific,
    pub type_defs: TypeDefs<Class>
}

#[derive( Clone, Serialize, Deserialize)]
pub struct TypeDefs<C> where C: Clone+Eq+PartialEq+Hash {
    pub schema: HashMap<SchemaKind, MetaDef<Point>>,
    pub schema2: HashMap<C, MetaDef<Point>>,
    pub clzz: HashMap<Class, MetaDef<Point>>,
}


