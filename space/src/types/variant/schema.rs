use crate::types::{private, TypeDiscriminant, SrcDef, Type, ExtType, Ext, Case};
use core::str::FromStr;
use derive_name::Name;
use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::types::{PointKindDefSrc};
use crate::parse::{camel_case, unwrap_block, CamelCase, Res};
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::types::variant::class::{Class, ClassDiscriminant};
use crate::types::parse::{TypeParser, PrimitiveParser, TypeParserImpl};
use crate::types::parse::util::TypeVariantStack;
use crate::types::variant::TypeVariant;

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

impl TypeVariant for Schema {
    type Parser = TypeParserImpl<Self>;
    type Discriminant = SchemaDiscriminant;

    type Segment = CamelCase;

    fn of_type() -> &'static TypeDiscriminant {
        & TypeDiscriminant::Schema
    }

    fn block() -> &'static NestedBlockKind {
        & NestedBlockKind::Square
    }
}

impl From<&CamelCase> for Schema{
    fn from(segment: &CamelCase) -> Self {
        match SchemaDiscriminant::from_str(segment.as_str()) {
            Ok(disc) => Schema::try_from(disc).unwrap(),
            Err(_) => Schema::_Ext(segment.clone()),
        }
    }
}

impl TryFrom<TypeVariantStack<Schema>>  for Schema {
    type Error = ParseErrs;

    fn try_from(stack: TypeVariantStack<Schema>) -> Result<Self, Self::Error> {

        match stack.two()? {
            (disc, None) => Ok(Schema::from(disc)),
            (disc, Some(variant)) => {

                Err(ParseErrs::new("Schema type generic does not yet support variants"))
                /*
                let disc = SchemaDiscriminant::try_from(disc)?;
                match disc {
                    disc => Err(ParseErrs::expected("Class::Discriminant", "a valid variant", format!("Class::Discriminant::{}",disc.to_string())))?
                }

                 */
            }
        }
    }
}

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


impl TryFrom<CamelCase> for SchemaDiscriminant{
    type Error = strum::ParseError;

    fn try_from(camel: CamelCase) -> Result<Self, Self::Error> {
        SchemaDiscriminant::from_str(&camel.as_str())
    }
}


impl Into<CamelCase> for Schema {
    fn into(self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }
}

impl Into<TypeDiscriminant> for Schema {
    fn into(self) -> TypeDiscriminant {
        TypeDiscriminant::Schema
    }
}


struct SchemaParser;


impl TryFrom<SchemaDiscriminant> for Schema {
    type Error = strum::ParseError;

    fn try_from(disc: SchemaDiscriminant) -> Result<Self, Self::Error> {
        match disc {
            SchemaDiscriminant::_Ext =>  Err(strum::ParseError::VariantNotFound),
            _ => Schema::from_str(disc.to_string().as_str())
        }

    }
}




pub type BindConfigSrc = PointKindDefSrc<Schema>;



#[derive(Clone, Serialize, Deserialize)]
pub struct SchemaDef;
