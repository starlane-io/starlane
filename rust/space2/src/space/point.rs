use alloc::string::String;
use alloc::vec::Vec;
use crate::space::parse::case::{DomainCase, SkewerCase};

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
    Domain(DomainCase),
    Tag(SkewerCase),
    Star(String),
}