use core::fmt::{Display, Formatter};
use core::ops::Deref;
use core::str::FromStr;
use strum_macros::EnumDiscriminants;
use crate::schema::case::CamelCase;
use crate::types::{Cat, Category, Typical};

#[derive(Clone,Debug,Eq,PartialEq,Hash,EnumDiscriminants,strum_macros::Display,strum_macros::IntoStaticStr)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Variant))]
#[strum_discriminants(derive(Hash,EnumString))]
pub(super) enum Data {
    Raw,
    #[strum(to_string = "{0}")]
    _Ext(CamelCase)
}


impl Typical for Data {
    fn category() -> Cat {
        Cat::Data
    }
}


impl FromStr for Data {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl Category for Data  {
    fn new(src: CamelCase) -> Self {
        todo!()
    }
}





/*
#[cfg(feature="parse")]
mod parse {
    use core::str::FromStr;
    use crate::err::SpaceErr;
    use crate::schema::case::CamelCase;
    use crate::types::class::Class;

    impl FromStr for Class {
        type Err = SpaceErr;

        fn from_str(src: &str) -> Result<Self, Self::Err> {
            CamelCase::from_str(src)

            Ok(Self(CamelCase::from_str(src)?))
        }
    }

}


 */





