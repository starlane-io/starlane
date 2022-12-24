use cosmic_space::err::SpaceErr;
use serde::{Deserialize, Serialize};

#[derive(Serialize,Deserialize)]
pub struct Composite {

}

#[rpc_sync]
pub trait Blah {
    fn me(&self) -> Result<(),SpaceErr>;
    fn one(&self, a: u8) -> Result<(),SpaceErr>;
    fn set_composite(&self, comp: Composite) -> Result<(),SpaceErr>;
    fn get_composite(&self) -> Result<Composite,SpaceErr>;
}
