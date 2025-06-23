use std::collections::HashMap;
use crate::types::specific::SpecificLoc;
use derive_builder::Builder;
use serde_derive::{Deserialize, Serialize};
use crate::types::def::Meta;
use crate::types::scope::Segment;
use crate::types::Type;

#[derive(Clone, Serialize, Deserialize, Builder)]
pub struct Package {
    specific: SpecificLoc,
    title: String,
    slices: Vec<Slice>,
}

impl Package {
    pub fn new(specific: SpecificLoc, title: impl AsRef<str>) -> Self {
        {
            let specific = specific.root();
            let title = title.as_ref().to_string();
            Self {
                specific,
                title,
                slices: Default::default(),
            }
        }
    }
  
    pub fn add_slice(&mut self, slice: Slice) {
      self.slices.push(slice);
    }
}

#[derive(Clone, Serialize, Deserialize, Builder)]
pub struct Slice {
    segment: Segment,
    children: Box<Vec<Slice>>,
    defs: HashMap<Type,Meta>
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

  pub fn add_def(&mut self, r#type: Type, def: Meta) {
    self.defs.insert(r#type,def);
  }
}
