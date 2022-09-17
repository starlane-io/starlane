use crate::{SetProperties, SetRegistry, Strategy};
use crate::id::{Kind, Point};
use crate::particle::Status;

#[derive(Clone)]
pub struct Registration {
    pub point: Point,
    pub kind: Kind,
    pub registry: SetRegistry,
    pub properties: SetProperties,
    pub owner: Point,
    pub strategy: Strategy,
    pub status: Status
}
