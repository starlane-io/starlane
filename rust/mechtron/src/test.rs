use crate as mechtron;
use crate::err::MechErr;

#[derive(starlane_primitive_macros::MechErr)]
pub struct MyErr {
    pub message: String,
}

#[test]
pub fn test() {}
