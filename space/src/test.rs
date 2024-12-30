
//#![starlane_macros::silly]

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use starlane_macros::{ route, show_streams};


#[test]
pub fn test()
{

}
pub struct MyStr {
  meat_ball: bool
}



#[show_streams]
trait MyTrait {}


#[show_streams(i_have_things_to_say)]
trait Trait2{}


/*
#[proxy]
impl MyStr {
    pub fn weird_day(&self) -> &'static str {
        "yikes!"
    }
}

 */


/*



 */


#[proxy]
pub trait My {
  fn some_method(&self, limits: u8) -> Result<(),()> {
  }
}