use crate::parse::util::{new_span, preceded, result, Span};
use crate::parse::{Domain, Res, SkewerCase};
use crate::selector::VersionReq;
use crate::types::archetype::Archetype;
use crate::types::scope::Segment;
use getset::Getters;
use nom::bytes::complete::tag;
use nom::combinator::opt;
use nom::multi::separated_list1;
use nom::sequence::tuple;
use serde::{Deserialize, Serialize};
use starlane_space::loc::Version;
use starlane_space::selector::Pattern;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use crate::types::{Absolute, Type};
use crate::types::class::Class;

pub type Specific = SpecificDef<Publisher, Package, Version, Segment>;
pub type Release = ReleaseDef<Publisher, Package, Version >;


#[cfg(test)]
#[test]
fn test() {
    Specific::mock_default();
    Specific::mock_0();
    Specific::mock_1();
    println!("SpecificLoc::mock_default() -> {}", Specific::mock_default());
    println!("SpecificLoc::mock_0() -> {}", Specific::mock_1());
    println!("SpecificLoc::mock_1() -> {}", Specific::mock_0());
}


#[cfg(test)]
impl Specific {
    pub fn mock_default() -> Self {
        result(Self::parser(new_span("starlane.io:uberscott:1.0.1::main:7.0.7"))).unwrap()
    }

    pub fn mock_0() -> Self {
        result(Self::parser(new_span("lavalordgames.com:astrobattle:3.0.1::backend"))).unwrap()
    }

    pub fn mock_1() -> Self {
        result(Self::parser(new_span("punch-line.app:jokes:10.0.1::a-material"))).unwrap()
    }
}

pub type Publisher = Domain;
pub type Package = SkewerCase;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Hash, Getters)]
#[get = "pub"]
pub struct ReleaseDef<Publisher, Package, Version >
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
{
    contributor: Publisher,
    package: Package,
    version: Version,
}

impl<Publisher, Package, Version > Display
for ReleaseDef<Publisher, Package, Version>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.contributor, self.package, self.version)?;
        Ok(())
    }
}

impl<Publisher, Package, Version > ReleaseDef<Publisher, Package, Version>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
{
    pub fn new(
        contributor: Publisher,
        package: Package,
        version: Version,
    ) -> Self {
        Self {
                contributor,
                package,
                version,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Hash, Getters)]
#[get = "pub"]
pub struct SpecificDef<Publisher, Package, Version, SliceSegment>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SliceSegment: Archetype,
{
    release: ReleaseDef<Publisher,Package,Version>,
    slices: Vec<SliceSegment>,
}

impl<Publisher, Package, Version, SliceSegment> Display
    for SpecificDef<Publisher, Package, Version, SliceSegment>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SliceSegment: Archetype,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.release)?;

        /// this is a bit weird, but the delimiter between `version` & `slice` needs
        /// two colons `::` ... the second one is prepended in the segment for loop
        if !self.slices.is_empty() {
            write!(f, ":")?;
        }

        for seg in self.slices.iter() {
            write!(f, ":{}", seg)?;
        }
        Ok(())
    }
}

impl<Publisher, Package, Version, SliceSeg> Archetype
    for SpecificDef<Publisher, Package, Version, SliceSeg>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SliceSeg: Archetype,
{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        tuple((
            Publisher::parser,
            tag(":"),
            Package::parser,
            tag(":"),
            Version::parser,
            opt(preceded(
                tag("::"),
                separated_list1(tag(":"), SliceSeg::parser),
            )),
        ))(input)
        .map(|(next, (contributor, _, package, _, version, slices))| {
            let slices = slices.unwrap_or_else(|| vec![]);
            (
                next,
                SpecificDef {
                    release: ReleaseDef {contributor,
                    package,
                    version},
                    slices,
                },
            )
        })
    }
}

impl<Publisher, Package, Version, SliceSeg> SpecificDef<Publisher, Package, Version, SliceSeg>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SliceSeg: Archetype,
{
    pub fn new(
        contributor: Publisher,
        package: Package,
        version: Version,
        slices: Vec<SliceSeg>,
    ) -> Self {
        Self {
            release: ReleaseDef {
                contributor,
                package,
                version,
            },
            slices,
        }
    }

    ///
    pub fn root(self) -> ReleaseDef<Publisher, Package, Version> {
            self.release
    }
}

impl <Publisher,Package,Version,SliceSegment> Into<ReleaseDef<Publisher,Package,Version>> for SpecificDef<Publisher,Package,Version,SliceSegment> where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SliceSegment: Archetype{
    fn into(self) -> ReleaseDef<Publisher, Package, Version> {
        self.release
    }
}

pub type SpecificSelector =
    SpecificDef<PublisherSelector, PackageSelector, VersionPattern, SlicePattern>;

pub type PublisherSelector = Pattern<Publisher>;
pub type PackageSelector = Pattern<Package>;
pub type VersionPattern = Pattern<VersionReq>;
pub type SlicePattern = Pattern<Segment>;

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

#[derive(Debug,Clone,Eq,PartialEq,Hash)]
pub enum RootSegment{
    Main,
    Segment(Segment),
}





