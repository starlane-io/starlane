use core::str::FromStr;
use strum_macros::EnumDiscriminants;
use crate::schema::case::CamelCase;
use crate::types;
use crate::types::{Cat, Typical};

#[derive(Clone,Debug,Eq,PartialEq,Hash,EnumDiscriminants,strum_macros::Display,strum_macros::IntoStaticStr)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Variant))]
#[strum_discriminants(derive(Hash,strum_macros::EnumString,strum_macros::ToString,strum_macros::IntoStaticStr))]
pub(super) enum Data {
    Raw,
    #[strum(to_string = "{0}")]
    _Ext(CamelCase)
}



impl FromStr for Data {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn ext( s: &str ) -> Result<Data,eyre::Error> {
            Ok(Data::_Ext(CamelCase::from_str(s)?.into()))
        }

        match Variant::from_str(s) {
            /// this Ok match is actually an Error!
            Ok(Variant::_Ext) => ext(s),
            Ok(variant) => ext(variant.into()),
            Err(_) => ext(s)
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



impl Into<CamelCase> for Data {
    fn into(self) -> CamelCase {
        todo!()
    }
}


impl Into<Cat> for Data {
    fn into(self) -> Cat {
        Cat::Data
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





