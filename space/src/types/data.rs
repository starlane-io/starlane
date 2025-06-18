use crate::types::{private, SrcDef, Abstract, Full, FullGeneric, AbstractDisc};
use core::str::FromStr;
use derive_name::Name;
use nom::bytes::complete::tag;
use nom::sequence::delimited;
use serde_derive::{Deserialize, Serialize};
use strum::ParseError;
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::parse::{camel_chars, from_camel};
use starlane_space::types::parse::data;
use starlane_space::types::PointKindDefSrc;
use crate::parse::{camel_case, CamelCase, Res};
use crate::parse::util::Span;
use crate::types::class::{Class, ClassDiscriminant};
use crate::types::parse::delim::delim;
use crate::types::private::{Delimited, Parsable};

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::EnumString, strum_macros::Display, Serialize,Deserialize,Name)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(DataDisc))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
pub enum Data {
    Bytes,
    Text,
    /// a [Data::BindConfig] definition for a [Class]
    BindConfig,
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(CamelCase),
}



/*
impl Into<TypeKind> for SchemaKind {
    fn into(self) -> TypeKind {
        TypeKind::Schema(self)
    }
}

 */

impl Delimited for Data {
    fn delimiters() -> (&'static str, &'static str) {
        ("[","]")
    }
}

impl private::Parsable for Data {

    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        from_camel(input)
    }

}

/*
impl Into<TypeKind>  for SchemaKind {
    fn into(self) -> TypeKind {
        TypeKind::Schema(self)
    }
}

 */


impl From<CamelCase> for Data {
    fn from(camel: CamelCase) -> Self {
        ///
        match DataDisc::from_str(camel.as_str()) {
            /// this Ok match is actually an Error
            Ok(DataDisc::_Ext) => panic!("DataDiscriminant: not CamelCase '{}'",camel),
            Ok(discriminant) => Self::try_from(discriminant.to_string().as_str()).unwrap(),
            /// if no match then it is an extension: [Class::_Ext]
            Err(_) => Data::_Ext(camel),
        }
    }
}







/*
impl Into<TypeKind> for SchemaKind {
    fn into(self) -> TypeKind {
        TypeKind::Schema(self)
    }
}

 */



impl Into<CamelCase> for Data {
    fn into(self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }
}

impl Into<AbstractDisc> for Data {
    fn into(self) -> AbstractDisc{
        AbstractDisc::Data
    }
}

pub type BindConfigSrc = PointKindDefSrc<Data>;


/*

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
#[derive(Clone, Serialize, Deserialize)]
pub struct DataDef;
