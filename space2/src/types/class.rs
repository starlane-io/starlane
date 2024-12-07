use crate::schema::case::CamelCase;
use crate::types::{Cat, Typical};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Class {
    Root,
    Platform,
    Foundation,
    /// Dependencies are external bits that can be downloaded and added to a Starlane instance.
    /// Adding new capabilities to Starlane via external software is the main intended use case
    /// for a Dependency (both Foundation binaries and WebAssembly alike)
    ///
    /// A Dependency can be `downloaded`, `installed`, `initialized` and `started` (what those
    /// phases actually mean to the Dependency itself is custom behavior)  The job of the Dependency
    /// create the prerequisite conditions for it child [Class::Provider]s
    Dependency,
    /// Provider `provides` something to Starlane.  Providers enable Starlane to extend itself by
    /// Providing new functionality that the core Starlane binary did not ship with.
    ///
    /// In particular Providers are meant to install WebAssembly Components, Drivers for new
    /// Classes, 3rd party software implementations... etc;
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
    _Ext(CamelCase),
}

impl Typical for Class {
    fn category(&self) -> Cat {
        Cat::Class
    }
}


#[cfg(feature = "parse")]
mod parse {
    use crate::err::ErrStrata;
    use crate::schema::case::CamelCase;
    use crate::types::class::Class;
    use core::str::FromStr;

    impl FromStr for Class {
        type Err = ErrStrata;

        fn from_str(src: &str) -> Result<Self, Self::Err> {
            Ok(Self(CamelCase::from_str(src)?))
        }
    }
}
