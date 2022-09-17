use crate::loc::{Point};
use crate::particle::Status;
use crate::{SetProperties, SetRegistry, Strategy};
use crate::kind::Kind;

#[derive(Clone)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
    pub strategy: Strategy,
    pub status: Status,
}
