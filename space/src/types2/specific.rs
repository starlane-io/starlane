use crate::err::ParseErrs0;
use crate::parse::util::{new_span, preceded, result, Span};
use crate::parse::{domain, filename, skewer_case, version, Domain, Res, SkewerCase};
use crate::selector::VersionReq;
use crate::types::archetype::Archetype;
use crate::types::class::Class;
use crate::types::scope::Segment;
use crate::types::{Absolute, Type};
use crate::types2::scope::SlicePath;
use getset::Getters;
use nom::bytes::complete::tag;
use nom::combinator::{all_consuming, opt};
use nom::multi::{separated_list0, separated_list1};
use nom::sequence::{pair, tuple, Tuple};
use serde::{Deserialize, Serialize};
use starlane_space::loc::Version;
use starlane_space::parse::consume_point;
use starlane_space::selector::Pattern;
use std::fmt::{Display, Formatter, Write};
use std::hash::Hash;
use std::path::PathBuf;
use std::str::FromStr;

pub type Release = ReleaseDef<Publisher, Package, Version>;
pub type Slice = SliceDef<Publisher, Package, Version, SlicePath>;
pub type File = FileDef<Slice, FilePath>;

impl FromStr for Release {
    type Err = ParseErrs0;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let i = new_span(s);
        let (_, release) = all_consuming(release)(i)?;
        Ok(release)
    }
}

pub fn release<S>(i: S) -> Res<S, Release>
where
    S: Span,
{
    ((domain, tag(":"), skewer_case, tag(":"), version))
        .parse(i)
        .map(|(next, (publisher, _, package, _, version))| {
            (
                next,
                Release {
                    publisher,
                    package,
                    version,
                },
            )
        })
}

#[cfg(test)]
#[test]
fn test() {
    Slice::mock_default();
    Slice::mock_0();
    Slice::mock_1();
    println!("SpecificLoc::mock_default() -> {}", Slice::mock_default());
    println!("SpecificLoc::mock_0() -> {}", Slice::mock_1());
    println!("SpecificLoc::mock_1() -> {}", Slice::mock_0());
}

#[cfg(test)]
impl Slice {
    pub fn mock_default() -> Self {
        result(Self::parser(new_span(
            "starlane.io:uberscott:1.0.1::main:7.0.7",
        )))
        .unwrap()
    }

    pub fn mock_0() -> Self {
        result(Self::parser(new_span(
            "lavalordgames.com:astrobattle:3.0.1::backend",
        )))
        .unwrap()
    }

    pub fn mock_1() -> Self {
        result(Self::parser(new_span(
            "punch-line.app:jokes:10.0.1::a-material",
        )))
        .unwrap()
    }
}

pub type Publisher = Domain;
pub type Package = SkewerCase;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Hash, Getters)]
#[get = "pub"]
pub struct ReleaseDef<Publisher, Package, Version>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
{
    publisher: Publisher,
    package: Package,
    version: Version,
}

impl Release {
    pub fn filename(&self) -> String {
        format!("{}_{}_{}", self.publisher, self.package, self.version)
    }
}

impl<Publisher, Package, Version> Display for ReleaseDef<Publisher, Package, Version>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.publisher, self.package, self.version)?;
        Ok(())
    }
}

impl<Publisher, Package, Version> ReleaseDef<Publisher, Package, Version>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
{
    pub fn new(contributor: Publisher, package: Package, version: Version) -> Self {
        Self {
            publisher: contributor,
            package,
            version,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Hash, Getters)]
#[get = "pub"]
pub struct SliceDef<Publisher, Package, Version, SlicePath>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SlicePath: Archetype,
{
    release: ReleaseDef<Publisher, Package, Version>,
    slices: SlicePath,
}

impl<Publisher, Package, Version, SliceSegment> Display
    for SliceDef<Publisher, Package, Version, SliceSegment>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SliceSegment: Archetype,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.release)?;
        let slices = self.slices.to_string();

        if !slices.is_empty() {
            write!(f, "::{}", self.slices)?;
        }

        Ok(())
    }
}

impl<Publisher, Package, Version, SlicePath> Archetype
    for SliceDef<Publisher, Package, Version, SlicePath>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SlicePath: Archetype + Default,
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
            opt(preceded(tag("::"), SlicePath::parser)),
        ))(input)
        .map(|(next, (contributor, _, package, _, version, slices))| {
            let slices = slices.unwrap_or_else(|| SlicePath::default());
            (
                next,
                SliceDef {
                    release: ReleaseDef {
                        publisher: contributor,
                        package,
                        version,
                    },
                    slices,
                },
            )
        })
    }
}

impl<Publisher, Package, Version, Slices> SliceDef<Publisher, Package, Version, Slices>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    Slices: Archetype,
{
    pub fn new(contributor: Publisher, package: Package, version: Version, slices: Slices) -> Self {
        Self {
            release: ReleaseDef {
                publisher: contributor,
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

impl<Publisher, Package, Version, SliceSegment> Into<ReleaseDef<Publisher, Package, Version>>
    for SliceDef<Publisher, Package, Version, SliceSegment>
where
    Publisher: Archetype,
    Package: Archetype,
    Version: Archetype,
    SliceSegment: Archetype,
{
    fn into(self) -> ReleaseDef<Publisher, Package, Version> {
        self.release
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Hash, Getters)]
#[get = "pub"]
pub struct FileDef<Slice, FilePath>
where
    Slice: Archetype,
    FilePath: Archetype,
{
    slice: Slice,
    path: FilePath,
}

impl<Slice, FilePath> Display for FileDef<Slice, FilePath>
where
    Slice: Archetype,
    FilePath: Archetype,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.slice)?;
        write!(f, "{}", self.path)?;
        Ok(())
    }
}
impl<Slice, FilePath> Archetype for FileDef<Slice, FilePath>
where
    Slice: Archetype,
    FilePath: Archetype,
{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        pair(Slice::parser, FilePath::parser)(input)
            .map(|(next, (slice, path))| (next, FileDef { slice, path }))
    }
}

impl Release {
    pub fn to_path(&self) -> PathBuf {
        let mut path = String::new();
        path.push_str(&self.publisher.to_string());
        path.push_str("/");
        path.push_str(&self.package.to_string());
        path.push_str("/");
        path.push_str(&self.version.to_string());
        PathBuf::from(path)
    }
}

impl Slice {
    pub fn to_path(&self) -> PathBuf {
        let mut path = String::new();
        path.push_str(self.release.to_path().to_str().unwrap());
        path.push_str("/");
        path.push_str(self.slices.filename().as_str());
        PathBuf::from(path)
    }
}

impl File {
    pub fn to_path(&self) -> PathBuf {
        let mut path = String::new();
        path.push_str(self.slice.to_path().to_str().unwrap());
        path.push_str(self.path.to_path().to_str().unwrap());
        PathBuf::from(path)
    }
}

pub type SliceSelector = SliceDef<PublisherSelector, PackageSelector, VersionPattern, SlicePattern>;

pub type PublisherSelector = Pattern<Publisher>;
pub type PackageSelector = Pattern<Package>;
pub type VersionPattern = Pattern<VersionReq>;
pub type SlicePattern = Pattern<SlicePath>;

pub type FilePattern = Pattern<FilePath>;
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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum RootSegment {
    Main,
    Segment(Segment),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FileSegment(String);

impl Display for FileSegment {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Archetype for FileSegment {
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        filename(input).map(|(next, segment)| ((next, Self(segment.to_string()))))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct FilePath {
    pub segments: Vec<FileSegment>,
}

impl FilePath {
    pub fn to_path(&self) -> PathBuf {
        let mut path = PathBuf::new();
        path.push("/");
        for (index, segment) in self.segments.iter().enumerate() {
            path.push(segment.to_string());
            if (index < self.segments.len() - 1) {
                path.push("/");
            }
        }
        path
    }
}

impl Default for FilePath {
    fn default() -> Self {
        Self { segments: vec![] }
    }
}

impl FilePath {
    pub fn new(segments: Vec<FileSegment>) -> Self {
        Self { segments }
    }

    pub fn push(&self, segment: FileSegment) -> Self {
        let mut segments = self.segments.clone();
        segments.push(segment);
        Self { segments }
    }

    pub fn insert(&mut self, segment: FileSegment) {
        self.segments.insert(0, segment);
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &FileSegment> {
        self.segments.iter()
    }

    pub fn first(&self) -> Option<&FileSegment> {
        self.segments.first()
    }

    pub fn remove_first(&mut self) -> Option<FileSegment> {
        if self.segments.len() > 0 {
            Some(self.segments.remove(0))
        } else {
            None
        }
    }

    pub fn path(&self) -> String {
        let mut rtn = String::new();
        rtn.push_str("/");
        for (index, segment) in self.segments.iter().enumerate() {
            rtn.push_str(&segment.to_string());
            if index < self.segments.len() - 1 {
                rtn.push_str("/");
            }
        }
        rtn
    }
}

impl Display for FilePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path())
    }
}

impl Archetype for FilePath {
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        preceded(tag("/"), separated_list0(tag("/"), FileSegment::parser))(input)
            .map(|(next, segments)| (next, FilePath::new(segments)))
    }
}
