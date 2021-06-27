use std::sync::Arc;

use futures::future::{err, join_all, ok};
use futures::prelude::*;

use crate::error::Error;
use crate::id::Id;
use crate::proto::{local_tunnels, ProtoStar, ProtoTunnel};
use crate::star::{Star, StarKey};
use crate::template::{StarTemplate, StarTemplateSelector};

pub struct Constellation {
    pub name: String,
    pub stars: Vec<StarTemplate>,
}

impl Constellation{
    pub fn new(name: String) -> Self {
        Self{
            name: name,
            stars: vec![]
        }
    }

    pub fn select( &self, selector: StarTemplateSelector ) -> Option<StarTemplate> {
        for star in &self.stars {
            match &selector {
                StarTemplateSelector::Handle(handle) => {
                    if star.handle == *handle {
                        return Option::Some(star.clone());
                    }
                }
                StarTemplateSelector::Kind(kind) => {
                    if star.kind == *kind {
                        return Option::Some(star.clone());
                    }
                }
            }
        }
        return Option::None;
    }
}

#[cfg(test)]
mod test {
    use tokio::runtime::Runtime;

    #[test]
    pub fn test() {}
}
