use crate::types::def::Defs;
use crate::types::scope::Segment;
use crate::types::Type;
use crate::types2::specific::Release;
use derive_builder::Builder;
use std::collections::HashMap;

#[derive(Clone, Builder)]
pub struct Package {
    release: Release,
    title: String,
    slices: Vec<Slice>,
}

impl Package {
    pub fn new(release: Release, title: impl AsRef<str>) -> Self {
        {
            let title = title.as_ref().to_string();
            Self {
                release,
                title,
                slices: Default::default(),
            }
        }
    }

    pub fn add_slice(&mut self, slice: Slice) {
        self.slices.push(slice);
    }
}

#[derive(Clone, Builder)]
pub struct Slice {
    segment: Segment,
    children: Box<Vec<Slice>>,
    defs: HashMap<Type, Defs>,
}

impl Slice {
    pub fn new(segment: Segment) -> Self {
        Self {
            segment,
            children: Box::new(Default::default()),
            defs: Default::default(),
        }
    }

    pub fn add_child(&mut self, child: Slice) {
        self.children.push(child);
    }

    pub fn add_def(&mut self, r#type: Type, def: Defs) {
        self.defs.insert(r#type, def);
    }
}

pub mod parse {}
