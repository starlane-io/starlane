use crate::types::{private, DataPoint, ExtType, Type, Ext, Case, Schema};
use crate::types::AbstractDiscriminant;
use core::str::FromStr;
use std::borrow::Borrow;
use derive_builder::Builder;
use derive_name::Name;
use nom::combinator::{cut, fail, into};
use nom::Parser;
use nom::sequence::delimited;
use serde_derive::{Deserialize, Serialize};
use strum::ParseError;
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::parse::{delim_kind_lex, from_camel};
use starlane_space::types::BlockParser;
use starlane_space::types::private::Generic;
use crate::parse::{camel_case, camel_chars, lex_block, unwrap_block, CamelCase, NomErr, Res};
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::Span;
use crate::point::Point;
use crate::types::class::service::Service;
use crate::types::parse::TzoParser;
use crate::types::private::{Parsers, Variant};
use crate::types::schema::SchemaDiscriminant;

#[derive(Clone, Eq,PartialEq,Hash,Debug, EnumDiscriminants, strum_macros::Display, Serialize, Deserialize,Name, strum_macros::EnumString )]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ClassDiscriminant))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
pub enum Class {
    Root,
    #[strum(disabled)]
    #[strum(to_string = "Service<{0}>")]
    Service(Service),
    Platform,
    Foundation,
    /// Dependencies are external bits that can be downloaded and added to a Starlane instance.
    /// Adding new capabilities to Starlane via external software is the starlane intended use case
    /// for a Dependency (both Foundation binaries and WebAssembly alike)
    ///
    /// A Dependency can be `downloaded`, `installed`, `initialized` and `started` (what those
    /// phases actually mean to the Dependency itself is custom behavior)  The job of the Dependency
    /// create the prerequisite conditions for it child [ClassDiscriminant::Provider]s
    Dependency,
    /// Provider `provides` something to Starlane.  Providers enable Starlane to extend itself by
    /// Providing new functionality that the core Starlane binary did not ship with.
    ///
    /// In particular Providers are meant to install WebAssembly Components, Drivers for new
    /// Classes, 3rd party software implementations... etc.
    ///
    Provider,
    /// meaning the [Host] of an execution environment, VM, Wasm host
    /// [Host] is a class and a layer in the message traversal
    Host,
    /// The [Guest] which executes inside a [Host].  A single Guest instance may provide the execution
    /// for any number of other Classes that it provides
    Guest,
    Database,
    Plugin,
    Global,
    Registry,
    Star,
    Driver,
    Portal,
    Control,
    App,
    Wasm,
    Repository,
    Artifact,
    Base,
    User,
    Role,
    Group,
    FileStore,
    Directory,
    File,
    #[strum(disabled)]
    #[strum(to_string = "{0}")]
    _Ext(CamelCase),
}

impl TzoParser for Class {
    fn inner<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        ClassParsers::new().parse(input)
    }
}

impl BlockParser for Class {
    fn block() -> NestedBlockKind {
        NestedBlockKind::Angle
    }
}

impl Generic for Class {
    type Discriminant = ClassDiscriminant;
    type Segment = CamelCase;

    fn abstract_discriminant(&self) -> AbstractDiscriminant {
        AbstractDiscriminant::Class
    }

    fn convention() -> Case {
        Case::CamelCase
    }

    fn block_kind() -> NestedBlockKind {
        NestedBlockKind::Angle
    }
}



impl TryFrom<ClassDiscriminant> for Class{
    type Error = strum::ParseError;

    fn try_from(disc: ClassDiscriminant) -> Result<Self, Self::Error> {
        match disc {
            ClassDiscriminant::_Ext =>  Err(strum::ParseError::VariantNotFound),
            _ => Class::from_str(disc.to_string().as_str())
        }
    }
}

pub struct ClassParsers;

impl ClassParsers {
    fn new() -> Self {
        Self
    }
}

impl Parsers for ClassParsers {
    type Output = Class;
    type Discriminant = ClassDiscriminant;
    type Variant = CamelCase;


    fn block<I,F,O>(f: F) -> impl FnMut(I) -> Res<I, O> where F: FnMut(I) -> Res<I,O>+Copy, I: Span {
        unwrap_block(BlockKind::Nested(NestedBlockKind::Angle),f)
    }

    fn segment<I>(input: I) -> Res<I, Self::Variant>
    where
        I: Span
    {
        camel_case(input)
    }

    fn discriminant<I>(input: I) -> Res<I, Self::Discriminant>
    where
        I: Span
    {
      let (next,segment) = Self::segment(input)?;
      Ok((next,ClassDiscriminant::from_str(segment.as_str()).unwrap_or_else(|_| ClassDiscriminant::_Ext)))
    }

    fn create(disc: Self::Discriminant, variant: Self::Variant) -> Result<Self::Output, strum::ParseError> {
        match disc {
            Self::Discriminant::Service => Ok(Service::from(variant).into()),
            _ => Err(strum::ParseError::VariantNotFound)
        }
    }

    fn block_kind() -> NestedBlockKind {
        NestedBlockKind::Angle
    }
}


impl Class {
    pub fn from_variant( variant: CamelCase, sub: CamelCase ) -> Result<Class,ParseErrs> {
        match variant.as_str() {
            "Service" => {
                Ok(Class::Service(Service::from_str(sub.as_str())?))
            }
            oops => Err(ParseErrs::new(format!("Class variant not found: '{}'", oops)))
        }
    }
}

pub mod service {
    use std::str::FromStr;
    use derive_name::Name;
    use nom::combinator::into;
    use nom::Parser;
    use serde_derive::{Deserialize, Serialize};
    use strum::ParseError;
    use strum_macros::{EnumDiscriminants, EnumString};
    use starlane_space::types::private::Variant;
    use crate::err::ParseErrs;
    use crate::parse::{camel_case, from_camel, CamelCase, NomErr, Res};
    use crate::parse::util::Span;
    use crate::types::Type;
    use crate::types::class::{Class, ClassDiscriminant};

    /// variants for [super::Class::Service]
    #[derive(
        Clone,
        Debug,
        Eq,
        PartialEq,
        Hash,
        EnumDiscriminants,
        EnumString,
        strum_macros::Display,
        Serialize,
        Deserialize,
        Name
    )]
    #[strum_discriminants(vis(pub))]
    #[strum_discriminants(name(Discriminant))]
    #[strum_discriminants(derive(
        Hash,
        strum_macros::EnumString,
        strum_macros::ToString,
    ))]
    #[non_exhaustive]
    pub enum Service {
        /// an external facing web service such as `Nginx`
        Web,
        /// a ref to a `Database Cluster` that serves [super::Class::Database] instances... NOT the same as
        Database,
        /// example: a `KeyCloak` instance which provides [super::Class::UserBase] which
        /// are instances of `KeyCloak Realms`
        UserBase,
        #[strum(disabled)]
        #[strum(to_string = "{0}")]
        _Ext(CamelCase)
    }

    impl Into<Class> for Service {
        fn into(self) -> Class{
            Class::Service(self)
        }
    }


    impl Variant for Service {
        type Root = Class;
        type Discriminant = Discriminant;



    }

    impl From<CamelCase> for Service{
        fn from(camel: CamelCase) -> Self {

            match Discriminant::from_str(camel.as_str()) {
                /// this Ok match is actually an Error
                Ok(Discriminant::_Ext) => panic!("Service: not CamelCase '{}'",camel),
                Ok(discriminant) => match Self::try_from(discriminant.to_string().as_str()) {
                    Ok(service) => service,
                    Err(err) => panic!("Service: invalid service: {}", err)
                }
                /// if no match then it is an extension: [Service::_Ext]
                Err(_) => Service::_Ext(camel),
            }
        }
    }

}







impl From<CamelCase> for Class {
    fn from(camel: CamelCase) -> Self {

        match ClassDiscriminant::from_str(camel.as_str()) {
            /// this Ok match is actually an Error
            Ok(ClassDiscriminant::_Ext) => panic!("ClassDiscriminant: not CamelCase '{}'",camel),
            Ok(discriminant) => Self::try_from(discriminant.to_string().as_str()).unwrap(),
            /// if no match then it is an extension: [Class::_Ext]
            Err(_) => Class::_Ext(camel),
        }
    }
}


/*
impl TryFrom<ClassDiscriminant> for Class{
    type Error = ParseErrs;

    fn try_from(d: ClassDiscriminant) -> Result<Self, Self::Error> {
        match &d {
            ClassDiscriminant::_Ext => Err(ParseErrs::new("cannot convert from 'Discriminant' to 'Class'")),
            ClassDiscriminant::Service => Err(ParseErrs::new("cannot convert from 'Discriminant::Service' to 'Class' which has variants...")),
            /// parse and hope for the best
            _ => {
                Class::from_str(d.to_string().as_str())
            }
        }

    }
}

 */







impl From<CamelCase> for ClassDiscriminant {
    fn from(src: CamelCase) -> Self {
        /// it should not be possible for this to fail
        Self::from_str(src.as_str()).unwrap()
    }
}


impl Into<CamelCase> for ClassDiscriminant {
    fn into(self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }
}




#[derive(Clone, Serialize, Deserialize)]
pub struct ClassDef;








/*
mod parse {
    use crate::types::class::Class;
    use core::str::FromStr;
    use crate::err::SpaceErr;
    use crate::parse::CamelCase;

    impl FromStr for Class {
        type Err = SpaceErr;

        fn from_str(src: &str) -> Result<Self, Self::Err> {
            Ok(Self(CamelCase::from_str(src)?))
        }
    }
}

 */
