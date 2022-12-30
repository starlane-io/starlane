use cosmic_space::err::SpaceErr;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Composite {}

#[rpc_sync]
pub trait Blah: Send+Sync {
    fn me(&self) -> Result<(), SpaceErr>;
    fn one(&self, a: &u8) -> Result<(), SpaceErr>;
    fn set_composite(&self, comp: &Composite) -> Result<(), SpaceErr>;
    fn get_composite(&self) -> Result<Composite, SpaceErr>;
    fn some_return(&self, comp: &Composite) -> Result<Composite, SpaceErr>;
}
