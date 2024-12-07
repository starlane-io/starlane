#![cfg(feature="types2")]

use strum_macros::EnumDiscriminants;
use thiserror::Error;

mod class;
mod data;

pub mod registry;
pub mod specific;



#[derive(Clone, Debug, Error)]
pub enum TypeErr {
    #[error("type unknown: '{0}'")]
    Unknown(String),
}

impl TypeErr {
    pub fn unknown(src: impl ToString) -> Self {
        Self::Unknown(src.to_string())
    }
}

/// meaning where does this Type definition come from
pub enum DefSrc {
    Builtin,
    Ext,
}




pub(crate) mod private {
    use std::collections::HashMap;
    use crate::parse::CamelCase;
    use crate::config;
    use crate::kind::Specific;
    use crate::point::Point;
    use crate::types::Cat;

    pub(crate) trait Typical {
        fn category(&self) -> super::Cat;
    }
    /// a `Variant` is a unique `Type` in a within a `Category`
    /// `Data` & `Class` categories and their enum variants ... i.e. [Data::Raw],[Class::_Ext]
    /// are the actual `Type` `Variants`
    /// Variants are always CamelCase
    pub(crate) trait Variant: Typical + Clone + ToString + From<CamelCase> + Into<CamelCase> { }


    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    pub(crate) struct KindDef<T>
    where
        T: Typical + Into<Cat>,
    {
        specific: Specific,
        variant: T,
    }

    /// C: Category Type (Class or Data)
    /// T: Type
    /// A: either a unique Artifact identifier [Point] or the thing itself [config::BindConfig]
    pub(crate) struct Meta<C,T,S> {
        r#type: T,
        category: C,
        refs: HashMap<S,Ref>
    }

    pub struct Ref {
        point: Point,
    }



}





#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Cat))]
#[strum_discriminants(derive(Hash))]
pub enum Type {
    Data(DataKind),
    Class(Kind),
}

pub use class::Class;
pub use class::Kind;
pub use data::Data;
pub use data::DataKind;




