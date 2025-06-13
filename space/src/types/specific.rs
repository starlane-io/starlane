use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use indexmap::Equivalent;
use serde_derive::{Deserialize, Serialize};
use starlane_space::loc::Version;
use starlane_space::selector::Pattern;
use crate::parse::{Domain, SkewerCase};
use crate::selector::VersionReq;
use crate::types::class::{Class, ClassDef};
use crate::types::scope::Scope;
use crate::types::{Schema, TagWrap};
use crate::types::schema::SchemaDef;
use crate::types::tag::VersionTag;

#[derive(Clone, Serialize, Deserialize)]
pub struct MetaDefs;

#[derive(Clone, Serialize, Deserialize)]
pub struct SpecificMeta {
    pub specific: Specific,
    pub defs: Definitions
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Definitions {
    pub scopes: HashMap<Scope, TypeDefs>,
}

impl Definitions {

}

#[derive(Clone, Serialize, Deserialize)]
pub struct TypeDefs {
    pub scope: Scope,
    pub class: ClassDefs,
    pub schema: SchemaDefs
}

pub type Defs<A,D>  = HashMap<A,D>;
pub type ClassDefs = Defs<Class,ClassDef>;
pub type SchemaDefs = Defs<Schema,SchemaDef>;


pub type Specific = SpecificGen<Contributor,Package,Version>;


impl Equivalent<Specific> for &Specific {
    fn equivalent(&self, specific: &Specific) -> bool {
        *self == specific
    }
}


pub type SpecificCtx = SpecificGen<Contributor,Package,TagWrap<Version,VersionTag>>;
pub type Contributor = Domain;
pub type Package = SkewerCase;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct SpecificGen<Contributor,Package,Version> {
    pub contributor: Contributor,
    pub package: Package,
    pub version: Version
}

impl <Contributor,Package,Version> Display for SpecificGen<Contributor,Package,Version> where Contributor:Display, Package:Display, Version: Display{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.contributor, self.package, self.version)
    }
}



impl <Contributor,Package,Version> SpecificGen<Contributor,Package,Version> {
    pub fn new(contributor: Contributor, package: Package, version: Version) -> Self  {
        Self { contributor, package, version }
    }
}

pub type SpecificSelector = SpecificGen<ContributorSelector,PackageSelector,VersionPattern>;

pub type ContributorSelector = Pattern<Contributor>;
pub type PackageSelector = Pattern<Package>;
pub type VersionPattern = Pattern<VersionReq>;

pub(crate) mod parse {
    use nom::sequence::tuple;
    use nom_supreme::tag::complete::tag;
    use super::{Specific, SpecificGen};
    use crate::parse::{pattern, version_req, Res};
    use crate::parse::util::Span;
    use crate::parse::domain as contributor;
    use crate::parse::skewer_case as package;
    use crate::parse::version as version;
    use super::SpecificSelector;

    /// parse the general structure of a [Specific] including: [SpecificSelector]...
    pub fn specific_gen<I,C,P,V>(contributor: impl FnMut(I) -> Res<I,C>,package: impl FnMut(I) -> Res<I,P>,version: impl FnMut(I) -> Res<I,V>, input: I ) -> Res<I, SpecificGen<C,P,V>> where I: Span {
        tuple((
            contributor,
            tag(":"),
            package,
            tag(":"),
            version,
        ))(input).map(|(next,(contributor,_,package,_,version))|{
            (next,SpecificGen::new(contributor,package,version))
        })
    }

    pub fn specific<I>(input: I) -> Res<I, Specific> where I: Span {
        specific_gen(contributor, package, version, input)
    }

    pub fn specific_selector<I: Span>(input: I) -> Res<I, SpecificSelector> {
        specific_gen(pattern(contributor), pattern(package), pattern(version_req), input)
    }
}