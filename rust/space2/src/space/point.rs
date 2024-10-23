use crate::lib::std::string::String;
use crate::lib::std::vec::Vec;
use crate::space::case::{DomainCase, SkewerCase};
pub type Point = PointDef<HyperSegment,PointSeg>;
pub struct PointDef<Route,Seg> {
    route: Route,
    segments: Vec<Seg>
}



pub enum PointSeg {

}


#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum HyperSegment {
    This,
    Space,
    Remote,
    Global,
    Domain(DomainCase),
    Tag(SkewerCase),
    Star(String),
}