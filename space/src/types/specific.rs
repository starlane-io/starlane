use std::fmt::{Display, Formatter};
use std::hash::Hash;
use indexmap::Equivalent;
use nom::bytes::complete::tag;
use nom::sequence::tuple;
use serde_derive::{Deserialize, Serialize};
use starlane_space::loc::VersionSegLoc;
use starlane_space::selector::Pattern;
use crate::cache::ArtifactLoc;
use crate::parse::{Domain, Res, SkewerCase};
use crate::parse::util::Span;
use crate::selector::VersionReq;
use crate::types::TagWrap;
use crate::types::archetype::Archetype;
use crate::types::def::SliceLoc;
use crate::types::scope::Segment;
use crate::types::tag::VersionTag;

pub type SpecificLoc = SpecificScaffold<ContributorSegLoc, PackageSegLoc, VersionSegLoc,SliceLoc>;


impl Equivalent<SpecificLoc> for &SpecificLoc {
    fn equivalent(&self, specific: &SpecificLoc) -> bool {
        *self == specific
    }
}


pub type SpecificLocCtx = SpecificScaffold<ContributorSegLoc, PackageSegLoc,TagWrap<VersionSegLoc,VersionTag>>;
pub type ContributorSegLoc = Domain;
pub type PackageSegLoc = SkewerCase;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct SpecificScaffold<Contributor,Package,Version,SliceSegment> where Contributor: Archetype, Package: Archetype, Version: Archetype, SliceSegment: Archetype 
{
    contributor: Contributor,
    package: Package,
    version: Version,
    slices: Vec<SliceSegment>
}

impl<Contributor, Package, Version,SliceSegment> Display for SpecificScaffold<Contributor, Package, Version, SliceSegment> where Contributor: Archetype, Package: Archetype, Version: Archetype, SliceSegment: Archetype
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.contributor, self.package, self.version)?;
        
        /// this is a bit weird, but the delimiter between `version` & `slice` needs
        /// two colons `::` ... the second one is prepended in the segment for loop 
        if !self.slices.is_empty() {
            write!(f, ":")?;
        }
        
        for seg in self.slices.iter() {
            write!(f, ":{}",seg)?;
        }
        Ok(())
    }
}

impl <Contributor,Package,Version> Archetype for SpecificScaffold<Contributor,Package,Version> where Contributor: Archetype, Package: Archetype, Version: Archetype
{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        tuple((Contributor::parser,tag(":"),Package::parser,tag(":"),Version::parser))(input).map(|(next,(contributor,_,package,_,version))|{
            (next, SpecificScaffold {contributor,package,version})
        })
    }
}


impl <Contributor,Package,Version> SpecificScaffold<Contributor,Package,Version> where Contributor: Archetype, Package: Archetype, Version: Archetype
{
    pub fn new(contributor: Contributor, package: Package, version: Version) -> Self  {
        Self { contributor, package, version }
    }
}

pub type SpecificSelector = SpecificScaffold<ContributorSelector,PackageSelector,VersionPattern>;

pub type ContributorSelector = Pattern<ContributorSegLoc>;
pub type PackageSelector = Pattern<PackageSegLoc>;
pub type VersionPattern = Pattern<VersionReq>;

/*
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

 */