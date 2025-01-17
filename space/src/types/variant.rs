use derive_name::Name;
use std::str::FromStr;
use std::fmt::Display;
use std::marker::PhantomData;
use crate::parse::model::NestedBlockKind;
use crate::types::parse::{PrimitiveArchetype, PrimitiveParser, TypeParser};
use crate::types::parse::util::TypeVariantStack;
use crate::types::{Type, TypeDiscriminant};
use class::Class;

pub mod class;
pub mod schema;

pub(crate) trait TypeVariant: TryFrom<TypeVariantStack<Self>>+Name+Clone+Into<Type>+Clone+FromStr+Display{

    type Parser: TypeParser<Self>;
    type Discriminant: FromStr<Err=strum::ParseError>;
    type Segment:  PrimitiveArchetype<Parser:PrimitiveParser>;

    fn max_stack_size() -> usize {
        2usize
    }

    fn of_type() -> &'static TypeDiscriminant;


    fn block() -> &'static NestedBlockKind;

    /// wrap the string value in it's `type` wrapper.
    ///
    /// for example:  [Class::to_string] for [Class::Database] would `Database`, or a variant like
    /// [Class::Service(Service::Database)] to_string would return `Service<Database>` and
    /// [Class::wrapped_string] would return `<Database>` and `<Service<Database>` respectively
    fn wrapped_string(&self) -> String {
        Self::block().wrap(self.to_string())
    }

}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct Identifier<T>(PhantomData<T>) where T: TypeVariant;

