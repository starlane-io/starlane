use crate::types::{private, DataPoint, Exact, Abstract, ExactGen};
use crate::types::AbstractDiscriminant;
use core::str::FromStr;
use std::borrow::Borrow;
use derive_builder::Builder;
use derive_name::Name;
use nom::Parser;
use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::parse::from_camel;
use crate::parse::{camel_case, camel_chars, CamelCase, NomErr, Res};
use crate::parse::util::Span;
use crate::point::Point;
use crate::types::class::service::Service;
use crate::types::private::Delimited;
use crate::types::schema::SchemaDiscriminant;
use crate::types::specific::Specific;

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

pub mod service {
    use derive_name::Name;
    use serde_derive::{Deserialize, Serialize};
    use strum_macros::{EnumDiscriminants, EnumString};
    use starlane_space::types::private::Variant;
    use crate::parse::CamelCase;
    use crate::types::Abstract;
    use crate::types::class::{Class, ClassDiscriminant};
    use crate::types::private::Super;

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

        fn parent(&self) -> Super<Self::Root> {
            Super::Root(self.clone().into())
        }
    }

}



impl Delimited for Class {
    fn type_delimiters() -> (&'static str, &'static str) {
        ("<",">")
    }
}

impl private::Generic for Class {
    type Abstract = Class;
    type Discriminant =ClassDiscriminant;

    fn discriminant(&self) -> AbstractDiscriminant {
        AbstractDiscriminant::Class
    }


    fn parse<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        from_camel(input)
    }
}

/*
impl Into<Abstract> for Class {
    fn into(self) -> Abstract {
        Abstract::Class(self)
    }
}

 */

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
