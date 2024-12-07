use alloc::string::{String, ToString};
use strum_macros::EnumDiscriminants;
use thiserror::Error;
use crate::kind::Specific;

//pub mod select;

mod class;
pub mod registry;

pub mod specific;



pub mod data;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct ExactDef<T>
where
    T: private::Typical + Into<Cat>,
{
    specific: Specific,
    variant: T,
}

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
    use crate::schema::case::CamelCase;
    use alloc::string::ToString;

    pub(crate) trait Typical {
        fn category(&self) -> super::Cat;
    }
    /// a `Variant` is a unique `Type` in a within a `Category`
    /// `Data` & `Class` categories and their enum variants ... i.e. [Data::Raw],[Class::_Ext]
    /// are the actual `Type` `Variants`
    /// Variants are always CamelCase
    pub(crate) trait Variant:
        Typical + Clone + ToString + From<CamelCase> + Into<CamelCase>
    {
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Cat))]
#[strum_discriminants(derive(Hash))]
pub enum Exact {
    Data(ExactDef<data::Data>),
    Class(ExactDef<class::Class>),
}

/*
/// reexports
pub type Class = private::ExactDef<class::Class>;
pub type Data = private::ExactDef<data::Data>;
pub type Schema = private::ExactDef<schema::Schema>;

 */
