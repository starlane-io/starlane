use crate::types::{private, DataPoint, Type, TypeKind};
use crate::types::TypeCategory;
use core::str::FromStr;
use std::borrow::Borrow;
use derive_builder::Builder;
use strum_macros::EnumDiscriminants;
use starlane_space::types::SchemaKind;
use starlane_space::types::schema::BindConfig;
use crate::kind::Specific;
use crate::parse::CamelCase;
use crate::point::Point;
use crate::types::private::Exact;

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ClassType))]
#[strum_discriminants(derive(
    Clone,
    Debug,
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
#[non_exhaustive]
pub enum ClassKind {

    Root,
    Platform,
    Foundation,
    /// Dependencies are external bits that can be downloaded and added to a Starlane instance.
    /// Adding new capabilities to Starlane via external software is the main intended use case
    /// for a Dependency (both Foundation binaries and WebAssembly alike)
    ///
    /// A Dependency can be `downloaded`, `installed`, `initialized` and `started` (what those
    /// phases actually mean to the Dependency itself is custom behavior)  The job of the Dependency
    /// create the prerequisite conditions for it child [ClassType::Provider]s
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
    Plugin,
    Service,
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
    #[strum(to_string = "{0}")]
    _Ext(CamelCase),
}

impl Into<TypeKind> for ClassKind {
    fn into(self) -> TypeKind {
        todo!()
    }
}

impl private::Kind for ClassKind  {
    type Type = Class;

    fn category(&self) -> TypeCategory {
        TypeCategory::Class
    }


    fn factory() -> impl Fn(Exact<Self>) -> Type {
        |t| Type::Class(t)
    }
}


impl FromStr for ClassKind {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn ext( s: &str ) -> Result<ClassKind,eyre::Error> {
            Ok(ClassKind::_Ext(CamelCase::from_str(s)?.into()))
        }

        match ClassType::from_str(s) {
            /// this Ok match is actually an Error!
            Ok(ClassType::_Ext) => ext(s),
            Ok(variant) => ext(variant.into()),
            Err(_) => ext(s)
        }
    }
}





impl From<CamelCase> for ClassType {
    fn from(src: CamelCase) -> Self {
        /// it should not be possible for this to fail
        Self::from_str(src.as_str()).unwrap()
    }
}


impl Into<CamelCase> for ClassType {
    fn into(self) -> CamelCase {
        CamelCase::from_str(self.to_string().as_str()).unwrap()
    }
}


impl Into<TypeCategory> for ClassType {
    fn into(self) -> TypeCategory {
        TypeCategory::Class
    }
}




pub type Class = private::Exact<ClassKind>;



//#[cfg(feature = "parse")]
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
