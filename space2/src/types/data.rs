use core::str::FromStr;
use strum_macros::EnumDiscriminants;
use crate::schema::case::CamelCase;
use crate::types;
use crate::types::{Cat, Typical};

#[derive(Clone,Debug,Eq,PartialEq,Hash,EnumDiscriminants,strum_macros::Display,strum_macros::IntoStaticStr)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Variant))]
#[strum_discriminants(derive(Hash,strum_macros::EnumString))]
pub(super) enum Data {
    Raw,
    #[strum(to_string = "{0}")]
    _Ext(CamelCase)
}


impl FromStr for Data {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match Variant::from_str(s) {
            Ok(variant) => Ok(variant.into()),
            Err(_) => Ok(CamelCase::from_str(s)?.into())
        }
    }
}


impl types::Variant for Data {

}

impl From<CamelCase> for Data {
    fn from(src: CamelCase) -> Self {
        Self::_Ext(src)
    }
}



impl Typical for Data {
    fn category(&self) -> Cat {
        Cat::Data
    }
}


impl FromStr for Data {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

impl Into<CamelCase> for Data {
    fn into(self) -> CamelCase {
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





