use std::collections::HashSet;
use std::sync::Arc;

use crate::{Path, ResourcePathSegment};


pub type Meta = HashSet<String,String>;
pub type Binary = Arc<Vec<u8>>;
pub type DataSchema = DataAspectType;

pub enum DataAspectType{
    Meta,
    Binary
}

#[derive(Clone)]
pub enum DataAspect{
    Meta(Meta),
    Binary(Binary)
}

