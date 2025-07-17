use crate::parse::util::Span;
use crate::parse::{CamelCase, Res};
use crate::types::archetype::Archetype;
use crate::types::class::Class;
use crate::types::parse::Delimited;
use crate::types::TypeDisc;
use core::str::FromStr;
use derive_name::Name;
use serde_derive::{Deserialize, Serialize};
use starlane_space::parse::from_camel;
use starlane_space::types::PointKindDefSrc;
use strum_macros::{Display, EnumDiscriminants};
use crate::parse2;
use crate::point::Point;
use crate::types2::Primitive;

#[derive(
    Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, Serialize, Deserialize, Name, Display,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(DataType))]
#[strum_discriminants(derive(
    Serialize,
    Deserialize,
    Hash,
    strum_macros::EnumString,
    strum_macros::IntoStaticStr,
    strum_macros::Display
))]
#[non_exhaustive]
pub enum Data {
    #[strum(to_string = "{0}")]
    Primitive(Primitive),
    #[strum(to_string = "{0}")]
    Point(Point),
    #[strum(to_string = "[u8,???]")]
    Bytes(Vec<u8>),
    #[strum(to_string = "{0}")]
    Config(Config),
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(CamelCase),
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    EnumDiscriminants,
    strum_macros::EnumString,
    strum_macros::Display,
    Serialize,
    Deserialize,
    Name,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ConfigKind))]
#[strum_discriminants(derive(
    Serialize,
    Deserialize,
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
pub enum Config {
    /// [Config::Pack]
    Pack,
    /// [Config::Slice] config (child of a [ConfigKind::Pack])
    Slice,
    /// [Config::Bind] config describes how a [Type] plugs into Starlane
    Bind,
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(CamelCase),
}

pub enum StringType {
    Camel,
    Skewer,
    Snake,
}

/*
impl Into<TypeKind> for SchemaKind {
    fn into(self) -> TypeKind {
        TypeKind::Schema(self)
    }
}

 */

impl Delimited for DataType {
    fn delimiters() -> (&'static str, &'static str) {
        ("[", "]")
    }
}

impl Archetype for DataType {
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
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

impl From<CamelCase> for DataType {
    fn from(camel: CamelCase) -> Self {
        ///
        match DataType::from_str(camel.as_str()) {
            /// this Ok match is actually an Error
            Ok(DataType::_Ext) => panic!("DataDisc : not CamelCase '{}'", camel),
            Ok(discriminant) => Self::try_from(discriminant.to_string().as_str()).unwrap(),
            /// if no match then it is an extension: [Class::_Ext]
            Err(_) => DataType::_Ext,
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

impl Into<CamelCase> for DataType {
    fn into(self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }
}

impl Into<TypeDisc> for DataType {
    fn into(self) -> TypeDisc {
        TypeDisc::Data
    }
}

pub type BindConfigSrc = PointKindDefSrc<DataType>;

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
