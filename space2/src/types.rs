use core::str::FromStr;
use strum_macros::EnumDiscriminants;
use thiserror::Error;
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
    pub(crate) struct ExactDef<T> where T: Typical+Category
    {
        specific: Specific,
        variant: T
    }

    impl <T> ExactDef<T> where T: Category { }


    #[derive(Clone,Debug,Error)]
    pub enum TypeErr {
        NotFound,
    }


    /// meaning where does this Type definition come from
    pub enum DefSrc {
        Builtin,
        Ext
    }


   pub(crate) trait Category: Typical+From<CamelCase>+Into<CamelCase>+FromStr
   {
        fn new(src: CamelCase) -> Self;


       fn src(&self) -> DefSrc {
           if self.as_str()  != "_Ext" {
               DefSrc::Builtin
           } else {
               DefSrc::Ext
           }
       }

       fn ident_src(src: &CamelCase) -> DefSrc {
           if src.as_str()  != "_Ext" {
               DefSrc::Builtin
           } else {
               DefSrc::Ext
           }
       }
   }

    impl <T> From<CamelCase> for T where T: Category
    {
        fn from(src: CamelCase) -> Self {
            Self::new(src)
        }
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
    fn category() -> Cat;
}

#[derive(Clone,Debug,Eq,PartialEq,Hash,EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Cat))]
#[strum_discriminants(derive(Hash))]
pub enum Exact {
    Data(ExactDef<data::Data>),
    Class(ExactDef<class::Class>),
}

impl Typical for Exact {}


/*
/// reexports
pub type Class = private::ExactDef<class::Class>;
pub type Data = private::ExactDef<data::Data>;
pub type Schema = private::ExactDef<schema::Schema>;

 */










