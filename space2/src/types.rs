use alloc::string::ToString;
use core::str::FromStr;
use strum_macros::EnumDiscriminants;
use thiserror::Error;
use crate::err::ErrStrata;
use crate::schema::case::CamelCase;
use crate::types::specific::Specific;

mod class;
mod schema;
mod data;
mod specific;
mod config;
mod authority;






    /// this is most
    #[derive(Clone,Debug,Eq,PartialEq,Hash)]
    pub(crate) struct ExactDef<T> where T: Typical+Into<Cat>
    {
        specific: Specific,
        variant: T
    }



    #[derive(Clone,Debug,Error)]
    pub enum TypeErr {
        NotFound,
    }


    /// meaning where does this Type definition come from
    pub enum DefSrc {
        Builtin,
        Ext
    }




pub mod meta {
    use crate::types::data::Data;
    use crate::types::{Exact, Typical};
    use crate::types::schema::Schema;


    /// information about the type
    #[derive(Clone,Debug)]
    pub struct Meta<T> where T: Typical{
        exact: Exact,
        defs: Defs
    }

    #[derive(Clone,Debug)]
    struct Defs {
       data: Data,
       schema: Schema,
    }

}
pub(crate) trait Typical {
    fn category(&self) -> Cat;
}

/// a `Variant` is a unique `Type` in a within a `Category`
/// `Data` & `Class` categories and their enum variants ... i.e. [Data::Raw],[Class::_Ext]
/// are the actual `Type` `Variants`
/// Variants are always CamelCase
//pub(crate) trait Variant: Typical+Clone+FromStr<Err=strum::ParseError>+ToString+From<CamelCase>+Into<CamelCase> {}
pub(crate) trait Variant: Typical+Clone+ToString+From<CamelCase>+Into<CamelCase> {}



#[derive(Clone,Debug,Eq,PartialEq,Hash,EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Cat))]
#[strum_discriminants(derive(Hash))]
pub enum Exact {
    Data(ExactDef<data::Data>),
    Class(ExactDef<class::Class>),
}

impl Typical for Exact {
    fn category(&self) -> Cat {
        self.into()
    }
}


/*
/// reexports
pub type Class = private::ExactDef<class::Class>;
pub type Data = private::ExactDef<data::Data>;
pub type Schema = private::ExactDef<schema::Schema>;

 */










