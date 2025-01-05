use crate::types::{private, TypeCategory, SrcDef, TypeKind, Type};
use core::str::FromStr;
use rustls::client::verify_server_cert_signed_by_trust_anchor;
use serde_derive::{Deserialize, Serialize};
use strum::ParseError;
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::kind::Specific;
use starlane_space::parse::{camel_chars, from_camel};
use starlane_space::types::PointKindDefSrc;
use crate::parse::{camel_case, CamelCase, Res};
use crate::parse::util::Span;
use crate::types::class::{ClassKind, ClassType};
use crate::types::parse::delim::schema;

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display, Serialize,Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SchemaType))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
pub enum SchemaKind {
    Bytes,
    Text,
    /// a `BindConfig` for a class
    BindConfig,
    #[strum(to_string = "{0}")]
    _Ext(CamelCase),
}



impl From<CamelCase> for SchemaKind {
    fn from(camel: CamelCase) -> Self {
        ///
        match SchemaType::from_str(camel.as_str()) {
            /// this Ok match is actually an Error
            Ok(SchemaType::_Ext) => panic!("ClassType: not CamelCase '{}'",camel),
            Ok(discriminant) => Self::try_from(discriminant).unwrap(),
            /// if no match then it is an extension: [ClassKind::_Ext]
            Err(_) => SchemaKind::_Ext(camel),
        }
    }
}

impl TryFrom<SchemaType> for SchemaKind {
    type Error = ();

    fn try_from(source: SchemaType) -> Result<Self, Self::Error> {
        match source {
            SchemaType::_Ext=> Err(()),
            /// true we are doing a naughty [Result::unwrap] of a [CamelCase::from_str] but
            /// a non [CamelCase] from [SchemaType::to_string] should be impossible unless some
            /// developer messed up
            source=> Ok(Self::_Ext(CamelCase::from_str(source.to_string().as_str()).unwrap()))
        }
    }
}

impl FromStr for SchemaKind {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let camel =  CamelCase::from_str(s)?;
        Ok(Self::from(camel))
    }
}




impl Into<TypeKind> for SchemaKind {
    fn into(self) -> TypeKind {
        TypeKind::Schema(self)
    }
}

impl private::Kind for SchemaKind {
    type Type = Schema;

    fn category(&self) -> TypeCategory {
        TypeCategory::Schema
    }

    fn parse<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        from_camel(input)
    }


    fn type_kind(&self) -> TypeKind {
        TypeKind::Schema(self.clone())
    }
}


impl Into<CamelCase> for SchemaKind {
    fn into(self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }
}

impl Into<TypeCategory> for SchemaKind {
    fn into(self) -> TypeCategory {
        TypeCategory::Schema
    }
}


pub type Schema = private::Exact<SchemaKind>;


pub type BindConfigSrc = PointKindDefSrc<SchemaKind>;


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
