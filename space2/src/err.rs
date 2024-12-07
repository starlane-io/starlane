use core::fmt::{Display, Formatter};
use strum_macros::EnumDiscriminants;
use thiserror::Error;

pub use eyre::eyre as err;


pub type ParseErrs = eyre::Error;

#[derive(Error,Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ErrKind))]
#[strum_discriminants(derive(Hash))]
pub enum ErrStrata {
    /// an unexpected system failure appears to be the root cause
    Sys,
    /// an agent (human or code) has done something wrong such as:
    /// * requesting something that isn't found
    /// * violating permission
    /// (I'm sure there will be more examples)
    Agent,
}



pub struct SpaceErr {

}




impl Display for ErrStrata {
    fn fmt(&self, _: &mut Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}




#[cfg(feature="serde")]
mod serde {
    use crate::schema::case::Version;
    use serde::de::Visitor;

    use std::fmt::Formatter;
    use std::str::FromStr;
}