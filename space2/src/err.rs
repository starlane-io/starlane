use core::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Error,Debug)]
pub struct SpaceErr { }

impl Display for SpaceErr {
    fn fmt(&self, _: &mut Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}

#[derive(Error,Debug)]
pub struct ParseErrs { }


impl Display for ParseErrs {
    fn fmt(&self, _: &mut Formatter<'_>) -> core::fmt::Result {
        todo!()
    }
}


#[cfg(feature="serde")]
mod serde {
    use std::fmt::Formatter;
    use std::str::FromStr;
    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
    use serde::de::Visitor;
    use crate::schema::case::Version;

}