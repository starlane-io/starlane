use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

pub type Point = PointDef<RouteSeg,PointSeg>;
pub struct PointDef<Route,Seg> {
    route: Route,
    segments: Vec<Seg>
}



pub enum PointSeg {

}


#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum RouteSeg {
    This,
    Local,
    Remote,
    Global,
    Domain(String),
    Tag(String),
    Star(String),
}