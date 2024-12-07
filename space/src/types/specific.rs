use std::iter::Map;
use serde::{Deserialize, Serialize};
use crate::kind::Specific;
use crate::point::Point;
use crate::types::class::Class;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecificMeta {
    pub specific: Specific,
    pub type_defs: TypeDefs
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeDefs {
    pub class: Map<Class,Meta<Point>>,
    pub data: Map<Class,Meta<Point>>,
}




#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta<B> {
    config: B,
}
