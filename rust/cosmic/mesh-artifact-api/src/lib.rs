#![allow(warnings)]
#![no_std]

#[cfg(feature = "std")]
extern crate std;

extern crate alloc;

use alloc::sync::Arc;
use std::ops::Deref;
use std::prelude::rust_2021::Vec;
use mesh_portal::version::latest::config::bind::BindConfig;
use mesh_portal::version::latest::id::Point;
use mesh_portal_versions::version::v0_0_1::id::id::PointSegKind;
use mesh_portal_versions::error::MsgErr;

pub mod file;

#[derive(Clone)]
pub struct Artifact<T> {
   point: Point,
   item: Arc<T>
}

impl <T> Artifact<T> {
  pub fn new( item: T, point: Point ) -> Artifact<T> {
    Artifact {
      point,
      item: Arc::new(item)
    }
  }

  pub fn point(&self) -> &Point {
      &self.point
  }

  pub fn bundle(&self) -> Result<Point,MsgErr> {
      self.point.clone().truncate(PointSegKind::Version)
  }

}


impl <T> Deref for Artifact<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    &self.item
  }
}

/*
impl <From,To> TryInto<Artifact<To>> for Artifact<From> where To: TryFrom<From,Error=anyhow::Error>{
  type Error = anyhow::Error;

  fn try_into(self) -> Result<Artifact<From>, Self::Error> {
     let from = self.item;
     Ok(Artifact::new(To::try_from(self.item)?))
  }
}

 */

#[cfg(test)]
pub mod test {
    #[test]
   pub fn test() {

   }
}