use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::marker::PhantomData;
use indexmap::Equivalent;
use serde_derive::{Deserialize, Serialize};
use starlane_space::loc::Version;
use starlane_space::selector::Pattern;
use starlane_space::types::parse::TypeParser;
use crate::parse::{Domain, Res, SkewerCase};
use crate::parse::util::Span;
use crate::types::class::{Class, ClassDef};
use crate::types::id::Id;
use crate::types::Schema;
use crate::types::scope::Scope;
use crate::types::schema::SchemaDef;

trait SpecificVariant {
    type Contributor: TypeParser+Clone;
    type Package: TypeParser+Clone;
    type Version: TypeParser+Clone;
}

#[derive(Debug, Clone, Hash,Eq, PartialEq)]
pub struct SpecificExt<V> where V: SpecificVariant {
    phantom: PhantomData<V>,
    pub contributor: V::Contributor,
    pub package: V::Package,
    pub version: V::Version,
}

impl <V> Display for SpecificExt<V> where V: SpecificVariant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", format!("{}:{}:{}", self.contributor, self.package, self.version))
    }
}



impl <V> TypeParser for SpecificExt<V> where V: SpecificVariant {
    fn inner<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        todo!()
    }
}

pub type Specific = SpecificExt<variants::Identifier>;
pub type SpecificSelector = SpecificExt<variants::Selector>;
pub type SpecificCtx = SpecificExt<variants::Ctx>;


pub mod variants {
    use crate::loc::Version;
    use crate::parse::{domain, Domain, Res, SkewerCase};
    use crate::parse::util::Span;
    use crate::selector::{Pattern, VersionReq};
    use crate::types::parse::TypeParser;
    use crate::types::specific::{SpecificVariant};
    use crate::types::tag::{TagWrap, VersionTag};

    pub type Contributor = Domain;


    pub type Package = SkewerCase;


    pub type ContributorSelector = Pattern<Contributor>;
    pub type PackageSelector = Pattern<Package>;
    pub type VersionSelector = TagWrap<Pattern<VersionReq>,VersionTag>;

    pub type ContributorCtx = Domain;
    pub type PackageCtx = Domain;
    pub type VersionCtx  = TagWrap<Version,VersionTag>;


    #[derive(Clone,Eq,PartialEq,Hash,Debug)]
    pub struct Identifier;
    #[derive(Clone)]
    pub(super) struct Selector;
    #[derive(Clone)]
    pub(super) struct Ctx;

    impl SpecificVariant for Identifier {
        type Contributor = Contributor;
        type Package = Package;
        type Version = Version;
    }

    /*
    impl SpecificVariant for Selector {
        type Contributor = ContributorSelector;
        type Package = PackageSelector;
        type Version = VersionSelector;
    }
    impl SpecificVariant for Ctx {
        type Contributor = ContributorCtx;
        type Package = PackageCtx;
        type Version = VersionCtx;
    }

     */
}


#[derive(Clone, Serialize, Deserialize)]
pub struct MetaDefs;

#[derive(Clone, Serialize, Deserialize)]
pub struct SpecificMeta {
    //pub specific: Specific,
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



impl Specific {
    pub fn parse<I>(input: I) -> Res<I,Self> where I: Span {
        todo!()
        //parse::specific(input)
    }
}

impl Equivalent<Specific> for &Specific {
    fn equivalent(&self, specific: &Specific) -> bool {
        *self == specific
    }
}











impl <V> SpecificExt<V> where V: SpecificVariant {
    pub fn new(contributor: V::Contributor, package: V::Package, version: V::Version) -> Self  {
        Self { contributor, package, version, phantom: PhantomData::default() }
    }
}







/*
impl SpecificSelector {
    pub fn parse<I>(input: I) -> Res<I,Self> where I: Span {
        parse::specific_selector(input)
    }
}


pub(crate) mod parse {
    use nom::sequence::tuple;
    use nom_supreme::tag::complete::tag;
    use starlane_space::types::parse::TypeParser;
    use starlane_space::types::tag::VersionTag;
    use super::{Specific, SpecificCtx, SpecificExt};
    use crate::parse::{pattern, version_req, Res};
    use crate::parse::util::Span;
    use crate::parse::domain as contributor;
    use crate::parse::skewer_case as package;
    use crate::parse::version as version;
    use crate::types::tag::AbstractTag;
    use super::SpecificSelector;



    /*
    /// parse the general structure of a [Specific] including: [SpecificSelector]...
    pub fn specific_gen<I,C,P,V>(contributor: impl FnMut(I) -> Res<I,C>,package: impl FnMut(I) -> Res<I,P>,version: impl FnMut(I) -> Res<I,V>, input: I ) -> Res<I, SpecificExt<C,P,V>> where I: Span {
        tuple((
            contributor,
            tag(":"),
            package,
            tag(":"),
            version,
        ))(input).map(|(next,(contributor,_,package,_,version))|{
            (next, SpecificExt::new(contributor, package, version))
        })
    }

    pub fn specific<I>(input: I) -> Res<I, Specific> where I: Span {
        specific_gen(contributor, package, version, input)
    }


    pub fn specific_ctx<I>(input: I) -> Res<I, SpecificCtx> where I: Span {
        specific_gen(contributor, package, VersionTag::wrap(version), input)
    }


    pub fn specific_selector<I: Span>(input: I) -> Res<I, SpecificSelector> {
        specific_gen(pattern(contributor), pattern(package), VersionTag::wrap(pattern(version_req)), input)
    }


     */

    /*

    #[cfg(test)]
    pub mod test {
        use crate::parse::util::{new_span, result};
        use crate::types::specific::{Specific, SpecificCtx};

        #[test]
        pub fn test_specific() {

            let specific = result(Specific::parse(new_span("uberscott.com:my-package:1.3.7"))).unwrap();
            println!("{}", specific);

            let specific = result(SpecificCtx::parse(new_span("uberscott.com:my-package:#[latest]"))).unwrap();
            println!("{}", specific);

        }

    }

     */

}

 */