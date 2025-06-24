use crate::types::def::Defs;
use crate::types::scope::Segment;
use crate::types::specific::SpecificLoc;
use crate::types::Type;
use derive_builder::Builder;
use std::collections::HashMap;

#[derive(Clone, Builder)]
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

#[derive(Clone, Builder)]
pub struct Slice {
    segment: Segment,
    children: Box<Vec<Slice>>,
    defs: HashMap<Type, Defs>
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
    self.defs.insert(r#type,def);
  }
}

pub mod parse {
    
}