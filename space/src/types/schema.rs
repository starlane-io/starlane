use crate::types::{private, TypeCategory, SrcDef, TypeKind, Type};
use core::str::FromStr;
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::kind::Specific;
use starlane_space::types::PointKindDefSrc;
use crate::parse::CamelCase;
use crate::types::private::{KindVariantDef, Exact};

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SchemaType))]
#[strum_discriminants(derive(
    Clone,
    Debug,
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

impl FromStr for SchemaKind {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn ext(s: &str) -> Result<SchemaKind,ParseErrs> {
            Ok(SchemaKind::_Ext(CamelCase::from_str(s)?.into()))
        }

        match SchemaType::from_str(s) {
            /// this Ok match is actually an Error!
            Ok(SchemaType::_Ext) => ext(s),
            Ok(variant) => ext(variant.into()),
            Err(_) => ext(s),
        }
    }
}


impl From<CamelCase> for SchemaKind {
    fn from(src: CamelCase) -> Self {
        /// it should not be possible for this to fail
        Self::from_str(src.as_str()).unwrap()
    }
}

impl private::Kind for SchemaKind {
    type Type = Schema;

    fn category(&self) -> TypeCategory {
        TypeCategory::Schema
    }

    fn type_kind(&self) -> TypeKind {
        TypeKind::Schema(self.clone())
    }

    fn factory() -> impl Fn(Exact<Self>) -> Type {
        |t| Type::Schema(t)
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

pub type BindConfig = SchemaKind::BindConfig;

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
