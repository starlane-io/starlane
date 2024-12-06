
pub struct SpaceErr {

}


#[cfg(feature="serde")]
mod serde {
    use std::fmt::Formatter;
    use std::str::FromStr;
    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
    use serde::de::Visitor;
    use crate::schema::case::Version;

}