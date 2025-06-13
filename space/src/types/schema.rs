use crate::types::{private, AbstractDiscriminant, SrcDef, Abstract, Exact, ExactGen};
use core::str::FromStr;
use derive_name::Name;
use rustls::client::verify_server_cert_signed_by_trust_anchor;
use serde_derive::{Deserialize, Serialize};
use strum::ParseError;
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::parse::{camel_chars, from_camel};
use starlane_space::types::parse::schema;
use starlane_space::types::PointKindDefSrc;
use crate::parse::{camel_case, CamelCase, Res};
use crate::parse::util::Span;
use crate::types::class::{Class, ClassDiscriminant};
use crate::types::private::{Delimited, Generic};

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::EnumString, strum_macros::Display, Serialize,Deserialize,Name)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SchemaDiscriminant))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
pub enum Schema {
    Bytes,
    Text,
    /// a [Schema::BindConfig] definition for a [Class]
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
impl Delimited for Schema {
    fn type_delimiters() -> (&'static str, &'static str) {
        ("[","]")
    }
}


impl Generic for Schema {
    type Abstract = Schema;
    type Discriminant = SchemaDiscriminant;

    fn discriminant(&self) -> super::AbstractDiscriminant {
        self.clone().into()
    }

    fn parse<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        schema(input)
    }

}

/*
impl Into<TypeKind>  for SchemaKind {
    fn into(self) -> TypeKind {
        TypeKind::Schema(self)
    }
}

 */


impl From<CamelCase> for Schema {
    fn from(camel: CamelCase) -> Self {
        ///
        match SchemaDiscriminant::from_str(camel.as_str()) {
            /// this Ok match is actually an Error
            Ok(SchemaDiscriminant::_Ext) => panic!("SchemaDiscriminant: not CamelCase '{}'",camel),
            Ok(discriminant) => Self::try_from(discriminant.to_string().as_str()).unwrap(),
            /// if no match then it is an extension: [Class::_Ext]
            Err(_) => Schema::_Ext(camel),
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



impl Into<CamelCase> for Schema {
    fn into(self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }
}

impl Into<AbstractDiscriminant> for Schema {
    fn into(self) -> AbstractDiscriminant {
        AbstractDiscriminant::Schema
    }
}

pub type BindConfigSrc = PointKindDefSrc<Schema>;


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
pub struct SchemaDef;
