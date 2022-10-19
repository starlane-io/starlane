use crate as mechtron;
use crate::err::MechErr;

#[derive(cosmic_macros_primitive::MechErr)]
pub struct MyErr {
    pub message: String,
}

#[test]
pub fn test() {}
