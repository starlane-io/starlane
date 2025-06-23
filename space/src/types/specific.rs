use std::fmt::{Display, Formatter};
use std::hash::Hash;
use getset::Getters;
use indexmap::Equivalent;
use nom::bytes::complete::tag;
use nom::combinator::opt;
use nom::multi::{separated_list0, separated_list1};
use nom::sequence::{delimited, tuple};
use serde_derive::{Deserialize, Serialize};
use starlane_space::loc::VersionSegLoc;
use starlane_space::selector::Pattern;
use crate::cache::ArtifactLoc;
use crate::parse::{Domain, Res, SkewerCase};
use crate::parse::util::{preceded, Span};
use crate::selector::VersionReq;
use crate::types::{scope, TagWrap};
use crate::types::archetype::Archetype;
use crate::types::def::SliceLoc;
use crate::types::scope::Segment;
use crate::types::tag::VersionTag;

pub type SpecificLoc = SpecificScaffold<ContributorSegLoc, PackageSegLoc, VersionSegLoc,SliceLoc>;

pub type ContributorSegLoc = Domain;
pub type PackageSegLoc = SkewerCase;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash,Getters)]
#[getset(get = "pub",)]
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

impl <Contributor,Package,Version,SliceSeg> Archetype for SpecificScaffold<Contributor,Package,Version,SliceSeg> where Contributor: Archetype, Package: Archetype, Version: Archetype, SliceSeg: Archetype
{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        tuple((Contributor::parser,tag(":"),Package::parser,tag(":"),Version::parser,opt(preceded(tag("::"),separated_list1( tag(":"), SliceSeg::parser)))))(input).map(|(next,(contributor,_,package,_,version, slices))|{
             let slices = slices.unwrap_or_else(|| vec![]);
            (next, SpecificScaffold {contributor,package,version,slices})
        })
    }
}


impl <Contributor,Package,Version,SliceSeg> SpecificScaffold<Contributor,Package,Version,SliceSeg> where Contributor: Archetype, Package: Archetype, Version: Archetype, SliceSeg: Archetype
{
    pub fn new(contributor: Contributor, package: Package, version: Version, slices: Vec<SliceSeg>) -> Self  {
        Self { contributor, package, version, slices }
    }
}

pub type SpecificSelector = SpecificScaffold<ContributorSelector,PackageSelector,VersionPattern,SlicePattern>;

pub type ContributorSelector = Pattern<ContributorSegLoc>;
pub type PackageSelector = Pattern<PackageSegLoc>;
pub type VersionPattern = Pattern<VersionReq>;
pub type SlicePattern = Pattern<SliceLoc>;

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