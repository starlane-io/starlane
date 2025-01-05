use crate::types::{private, DataPoint, Type, TypeKind};
use crate::types::TypeCategory;
use core::str::FromStr;
use std::borrow::Borrow;
use derive_builder::Builder;
use nom::Parser;
use strum_macros::EnumDiscriminants;
use starlane_space::err::ParseErrs;
use starlane_space::parse::from_camel;
use starlane_space::types::SchemaKind;
use crate::kind::Specific;
use crate::parse::{camel_case, camel_chars, CamelCase, NomErr, Res};
use crate::parse::util::Span;
use crate::point::Point;
use crate::types::private::{Exact, Kind};
use crate::types::schema::SchemaType;

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ClassType))]
#[strum_discriminants(derive(
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
    /// Adding new capabilities to Starlane via external software is the starlane intended use case
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

impl TryFrom<ClassType> for ClassKind {
    type Error = ();

    fn try_from(source: ClassType) -> Result<Self, Self::Error> {
        match source {
            ClassType::_Ext=> Err(()),
            /// true we are doing a naughty [Result::unwrap] of a [CamelCase::from_str] but
            /// a non [CamelCase] from [SchemaType::to_string] should be impossible unless some
            /// developer messed up
            source=> Ok(Self::_Ext(CamelCase::from_str(source.to_string().as_str()).unwrap()))
        }
    }
}

impl private::Kind for ClassKind  {
    type Type = Class;

    fn category(&self) -> TypeCategory {
        TypeCategory::Class
    }


    fn parse<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        from_camel(input)
    }

    /*
    fn parser<I>(input:I ) -> Res<I, Self>
    where
        I: Span
    {
        let (next,kind) = camel_chars(input)?;
        let kind = Self::from_str(kind.to_string().as_str())?;
        Ok((next,kind))
    }

     */


    fn type_kind(&self) -> TypeKind {
        todo!()
    }
}

impl From<CamelCase> for ClassKind {
    fn from(camel: CamelCase) -> Self {
        ///
        match ClassType::from_str(camel.as_str()) {
            /// this Ok match is actually an Error
            Ok(ClassType::_Ext) => panic!("ClassType: not CamelCase '{}'",camel),
            Ok(discriminant) => Self::try_from(discriminant).unwrap(),
            /// if no match then it is an extension: [ClassKind::_Ext]
            Err(_) => ClassKind::_Ext(camel),
        }
    }
}


impl FromStr for ClassKind {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::from(CamelCase::from_str(s)?))
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
