#[macro_use]
extern crate strum_macros;

use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use std::sync::Arc;

use nom::{AsChar, InputTakeAtPosition, IResult};
use nom::branch::alt;
use nom::bytes::complete::{tag, take};
use nom::character::complete::{alpha0, alpha1, anychar, digit0, digit1, one_of};
use nom::combinator::{not, opt};
use nom::error::{context, ErrorKind, VerboseError};
use nom::multi::{many1, many_m_n, separated_list0, separated_list1};
use nom::sequence::{delimited, preceded, terminated, tuple};
use serde::Deserialize;
use serde::Serialize;

use starlane_macros::resources;

use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use crate::message::Fail;
use semver::SemVerError;

pub mod data;
pub mod error;
pub mod parse;
pub mod message;
pub mod http;

pub enum Galaxy{
    Local,
    Default,
    Exact(DomainCase)
}



#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct ResourceAddress {
    path: ResourcePath,
}

impl ResourceAddress {
    pub fn root() -> Self {
        ResourcePath::Root.into()
    }

    pub fn new(path: ResourcePath) -> Self {
        Self { path: path }
    }

    pub fn append(&self, string: String, resource_type: ResourceType) -> Result<Self, Error> {
        let address = format!(
            "{}:{}<{}>",
            self.path.to_string(),
            string,
            resource_type.to_string()
        );
        let path = ResourcePath::from_str(address.as_str())?;
        Ok(Self { path: path })
    }

    pub fn parent(&self) -> Option<ResourceAddress> {
        match self.path.parent() {
            Option::None => Option::None,
            Option::Some(parent) => Option::Some(parent.into()),
        }
    }

    pub fn ancestor_of_type(&self, resource_type: ResourceType) -> Result<ResourceAddress, Error> {
        if self.resource_type() == resource_type {
            return Ok(self.clone());
        } else if let Option::Some(parent) = self.parent() {
            parent.ancestor_of_type(resource_type)
        } else {
            Err(format!(
                "does not have ancestor of type {}",
                resource_type.to_string()
            )
            .into())
        }
    }

    pub fn sub_space(&self) -> Result<ResourceAddress, Error> {
        self.ancestor_of_type(ResourceType::SubSpace)
    }

    pub fn space(&self) -> Result<ResourceAddress, Error> {
        self.ancestor_of_type(ResourceType::Space)
    }

    pub fn resource_type(&self) -> ResourceType {
        self.path.resource_type()
    }

    pub fn to_parts_string(&self) -> String {
        self.path.to_string()
    }

    pub fn name(&self) -> String {
        self.path.name()
    }

    pub fn last_to_string(&self) -> String {
        self.name()
    }
}

impl ToString for ResourceAddress {
    fn to_string(&self) -> String {
        format!(
            "{}<{}>",
            self.path.to_string(),
            self.path.resource_type().to_string()
        )
    }
}

impl FromStr for ResourceAddress {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ResourcePath::from_str(s)?.into())
    }
}

impl From<ResourcePath> for ResourceAddress {
    fn from(path: ResourcePath) -> Self {
        Self { path: path }
    }
}

impl ResourceKey {
    pub fn ancestor_of_type(&self, resource_type: ResourceType) -> Result<ResourceKey, Error> {
        if self.resource_type() == resource_type {
            return Ok(self.clone());
        } else if let Option::Some(parent) = self.parent() {
            parent.ancestor_of_type(resource_type)
        } else {
            Err(format!(
                "does not have ancestor of type {}",
                resource_type.to_string()
            )
            .into())
        }
    }
}

impl ResourceKind {
    pub fn init_clap_config(&self) -> Option<ArtifactPath> {
        Option::None
    }
}

pub struct ResourceAddressKind {
    path: ResourcePath,
    kind: ResourceKind,
}

impl ResourceAddressKind {
    pub fn new(path: ResourcePath, kind: ResourceKind) -> Self {
        Self {
            path: path,
            kind: kind,
        }
    }

    pub fn kind(&self) -> ResourceKind {
        self.kind.clone()
    }
}

impl ToString for ResourceAddressKind {
    fn to_string(&self) -> String {
        format!("{}{}", self.path.to_string(), self.kind.to_string())
    }
}

impl FromStr for ResourceAddressKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = ResourcePath::from_str(s)?;
        let (_leftover, (_, kind)) = parse_path(s)?;
        Ok(Self {
            path: path,
            kind: kind.try_into()?,
        })
    }
}

pub type Res<T, U> = IResult<T, U, VerboseError<T>>;

static RESOURCE_ADDRESS_DELIM: &str = ":";

fn alphanumerichyphen1<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-') && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

fn pathchar<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '/')
                && !(char_item == '.')
                && !(char_item == '_')
                && !(char_item == '-')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

fn address<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '.')
                && !(char_item == '/')
                && !(char_item == ':')
                && !(char_item == '-')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

fn loweralphanumerichyphen1<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

fn path(input: &str) -> Res<&str, Path> {
    context("path", preceded(tag("/"), pathchar))(input).map(|(input, path)| {
        let path = format!("/{}", path);
        let path = Path::new(path.as_str());
        (input, path)
    })
}

fn host(input: &str) -> Res<&str, String> {
    context(
        "host",
        alt((
            tuple((many1(terminated(alphanumerichyphen1, tag("."))), alpha1)),
            tuple((many_m_n(1, 1, alphanumerichyphen1), take(0 as usize))),
        )),
    )(input)
    .map(|(next_input, mut res)| {
        if !res.1.is_empty() {
            res.0.push(res.1);
        }
        (next_input, res.0.join("."))
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct DomainCase {
    string: String,
}

impl DomainCase {
    fn new(string: &str) -> Self {
        Self {
            string: string.to_string(),
        }
    }
}

impl FromStr for DomainCase {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (remaining, domain) = domain(s)?;
        if remaining.len() > 0 {
            Err(format!("remainig text '{}' when parsing domain: '{}'", remaining, s).into())
        } else {
            Ok(domain)
        }
    }
}

impl ToString for DomainCase {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

fn domain(input: &str) -> Res<&str, DomainCase> {
    context(
        "domain",
        tuple((
            many1(terminated(loweralphanumerichyphen1, tag("."))),
            loweralphanumerichyphen1,
        )),
    )(input)
    .map(|(next_input, mut res)| {
        if !res.1.is_empty() {
            res.0.push(res.1);
        }
        (next_input, DomainCase::new(res.0.join(".").as_str()))
    })
}

fn zero(input: &str) -> Res<&str, &str> {
    context("zero", tag("0"))(input)
}

/*
fn integer( input: &str) -> Res<&str,String> {
    context( "int",
             alt( (tag("0"),tuple((one_of("123456789"), opt(digit1)) ))) )(input).map( |(input,output)|{})
}

 */

fn version_major_minor_patch(input: &str) -> Res<&str, (usize, usize, usize)> {
    context(
        "version_major_minor_patch",
        tuple((
            terminated(digit1, tag(".")),
            terminated(digit1, tag(".")),
            terminated(digit1, not(digit1)),
        )),
    )(input)
    .map(|(next_input, res)| {
        (
            next_input,
            (
                res.0.parse().unwrap(),
                res.1.parse().unwrap(),
                res.2.parse().unwrap(),
            ),
        )
    })
}

fn version(input: &str) -> Res<&str, Version> {
    context(
        "version",
        tuple((version_major_minor_patch, opt(preceded(tag("-"), skewer)))),
    )(input)
    .map(|(next_input, ((major, minor, patch), release))| {
        (next_input, Version::new(major, minor, patch, release))
    })
}

fn specific(input: &str) -> Res<&str, Specific> {
    context(
        "specific",
        tuple((
            terminated(domain, tag(":")),
            terminated(loweralphanumerichyphen1, tag(":")),
            terminated(loweralphanumerichyphen1, tag(":")),
            version,
        )),
    )(input)
    .map(|(next_input, (vendor, product, variant, version))| {
        (
            next_input,
            Specific {
                vendor: vendor,
                product: product.to_string(),
                variant: variant.to_string(),
                version: version,
            },
        )
    })
}

pub fn parse_kind(input: &str) -> Res<&str, ResourceKindParts> {
    context(
        "kind",
        delimited(
            tag("<"),
            tuple((
                alpha1,
                opt(delimited(
                    tag("<"),
                    tuple((alpha1, opt(delimited(tag("<"), specific, tag(">"))))),
                    tag(">"),
                )),
            )),
            tag(">"),
        ),
    )(input)
    .map(|(input, (rt, more))| {
        let kind = match &more {
            None => Option::None,
            Some((kind, _)) => Option::Some((*kind).clone().to_string()),
        };
        let spec = match &more {
            None => Option::None,
            Some((_, Option::Some(spec))) => Option::Some(spec.clone()),
            _ => Option::None,
        };
        (
            input,
            ResourceKindParts {
                resource_type: rt.to_string(),
                kind: kind,
                specific: spec,
            },
        )
    })
}

pub fn parse_key(input: &str) -> Res<&str, Vec<ResourcePathSegment>> {
    context(
        "key",
        separated_list1(
            nom::character::complete::char(':'),
            alt((path_part, version_part, domain_part, skewer_part)),
        ),
    )(input)
}

pub fn parse_resource_path(input: &str) -> Res<&str, Vec<ResourcePathSegment>> {
    context(
        "address-path",
        separated_list0(
            nom::character::complete::char(':'),
            alt((path_part, version_part, domain_part, skewer_part)),
        ),
    )(input)
}

pub fn parse_path(input: &str) -> Res<&str, (Vec<ResourcePathSegment>, ResourceKindParts)> {
    context("address", tuple((parse_resource_path, parse_kind)))(input)
}

fn skewer(input: &str) -> Res<&str, SkewerCase> {
    context("skewer-case", loweralphanumerichyphen1)(input)
        .map(|(input, skewer)| (input, SkewerCase::new(skewer)))
}

fn skewer_part(input: &str) -> Res<&str, ResourcePathSegment> {
    context("skewer-case-part", skewer)(input)
        .map(|(input, skewer)| (input, ResourcePathSegment::SkewerCase(skewer)))
}

fn version_part(input: &str) -> Res<&str, ResourcePathSegment> {
    context("version-part", version)(input)
        .map(|(input, version)| (input, ResourcePathSegment::Version(version)))
}

fn domain_part(input: &str) -> Res<&str, ResourcePathSegment> {
    context("domain-part", domain)(input)
        .map(|(input, domain)| (input, ResourcePathSegment::Domain(domain)))
}

fn path_part(input: &str) -> Res<&str, ResourcePathSegment> {
    context("path-part", path)(input).map(|(input, path)| (input, ResourcePathSegment::Path(path)))
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Specific {
    pub vendor: DomainCase,
    pub product: String,
    pub variant: String,
    pub version: Version,
}

impl ToString for Specific {
    fn to_string(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.vendor.to_string(),
            self.product,
            self.variant,
            self.version.to_string()
        )
    }
}

impl FromStr for Specific {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover, specific) = specific(s)?;
        if leftover.len() != 0 {
            Err(format!(
                "could not process '{}' portion of specific '{}'",
                leftover, s
            )
            .into())
        } else {
            Ok(specific)
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceKindParts {
    pub resource_type: String,
    pub kind: Option<String>,
    pub specific: Option<Specific>,
}

impl ToString for ResourceKindParts {
    fn to_string(&self) -> String {
        if self.specific.is_some() && self.kind.is_some() {
            format!(
                "<{}<{}<{}>>>",
                self.resource_type,
                self.kind.as_ref().unwrap().to_string(),
                self.specific.as_ref().unwrap().to_string()
            )
        } else if self.kind.is_some() {
            format!(
                "<{}<{}>>",
                self.resource_type,
                self.kind.as_ref().unwrap().to_string()
            )
        } else {
            format!("<{}>", self.resource_type)
        }
    }
}

impl FromStr for ResourceKindParts {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover, rtn) = parse_kind(s)?;
        if leftover.len() > 0 {
            return Err(format!(
                "ResourceKindParts ERROR: could not parse extra: '{}' in string '{}'",
                leftover, s
            )
            .into());
        }
        Ok(rtn)
    }
}

impl Into<ResourceAddress> for ResourceAddressKind {
    fn into(self) -> ResourceAddress {
        self.path.into()
    }
}

/*
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceAddressKind {
    pub address: ResourceAddress,
    pub kind: ResourceKind
}

impl FromStr for ResourceAddressKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover,(address,kind)) = parse_address(s)?;
        if leftover.len() > 0 {
            return Err(format!("Parse Error for ResourceAddressKind: leftover '{}' when parsing '{}'",leftover,s).into());
        }

        let kind = ResourceKind::try_from(kind)?;
        let address = format!("{}::<{}>",address,kind.resource_type().to_string());
        let address = ResourceAddress::from_str(address.as_str())?;

        Ok(ResourceAddressKind{
            address,
            kind
        })
    }

}

impl Into<ResourceAddress> for ResourceAddressKind {
    fn into(self) -> ResourceAddress {
        self.address
    }
}
 */

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SkewerCase {
    string: String,
}

impl ToString for SkewerCase {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl SkewerCase {
    fn new(string: &str) -> Self {
        Self {
            string: string.to_string(),
        }
    }
}

impl Into<ResourcePathSegment> for SkewerCase {
    fn into(self) -> ResourcePathSegment {
        ResourcePathSegment::SkewerCase(self)
    }
}

impl FromStr for SkewerCase {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (remaining, skewer) = skewer(s)?;
        if remaining.len() > 0 {
            Err(format!(
                "could not parse skewer because of remaining: '{}' in skewer: '{}'",
                remaining, s
            )
            .into())
        } else {
            Ok(skewer)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ResourcePathSegmentKind {
    Domain,
    SkewerCase,
    Email,
    Version,
    Path,
}

impl ToString for ResourcePathSegmentKind {
    fn to_string(&self) -> String {
        match self {
            ResourcePathSegmentKind::Domain => "Domain".to_string(),
            ResourcePathSegmentKind::SkewerCase => "Skewer".to_string(),
            ResourcePathSegmentKind::Version => "Version".to_string(),
            ResourcePathSegmentKind::Path => "Path".to_string(),
            ResourcePathSegmentKind::Email => "Email".to_string(),
        }
    }
}

impl ResourcePathSegmentKind {
    pub fn matches(&self, part: &ResourcePathSegment) -> bool {
        match part {
            ResourcePathSegment::SkewerCase(_) => *self == Self::SkewerCase,
            ResourcePathSegment::Path(_) => *self == Self::Path,
            ResourcePathSegment::Version(_) => *self == Self::Version,
            ResourcePathSegment::Email(_) => *self == Self::Email,
            ResourcePathSegment::Domain(_) => *self == Self::Domain,
        }
    }

    pub fn from_str(&self, s: &str) -> Result<ResourcePathSegment, Error> {
        let (leftover, part) = path_part(s)?;
        if leftover.len() > 0 {
            Err(format!(
                "could not parse entire path string: leftover: '{}' from '{}'",
                leftover, s
            )
            .into())
        } else {
            Ok(part)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ResourcePathSegment {
    SkewerCase(SkewerCase),
    Domain(DomainCase),
    Path(Path),
    Version(Version),
    Email(String),
}

impl ResourcePathSegment {
    pub fn to_kind(self) -> ResourcePathSegmentKind {
        match self {
            ResourcePathSegment::SkewerCase(_) => ResourcePathSegmentKind::SkewerCase,
            ResourcePathSegment::Domain(_) => ResourcePathSegmentKind::Domain,
            ResourcePathSegment::Path(_) => ResourcePathSegmentKind::Path,
            ResourcePathSegment::Version(_) => ResourcePathSegmentKind::Version,
            ResourcePathSegment::Email(_) => ResourcePathSegmentKind::Email,
        }
    }
}

impl ToString for ResourcePathSegment {
    fn to_string(&self) -> String {
        match self {
            ResourcePathSegment::SkewerCase(skewer) => skewer.to_string(),
            ResourcePathSegment::Path(path) => path.to_string(),
            ResourcePathSegment::Version(version) => version.to_string(),
            ResourcePathSegment::Email(email) => email.to_string(),
            ResourcePathSegment::Domain(domain) => domain.to_string(),
        }
    }
}

pub struct ParentAddressPatternRecognizer<T> {
    patterns: HashMap<AddressPattern, T>,
}

impl<T> ParentAddressPatternRecognizer<T> {
    pub fn try_from(&self, _pattern: &AddressPattern) -> Result<T, Error> {
        unimplemented!()
        //        self.patterns.get(pattern ).cloned().ok_or(Error{message:"Could not find a match for ParentAddressPatternRecognizer".to_string()})
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct AddressPattern {
    pattern: Vec<ResourcePathSegmentKind>,
}

impl From<Vec<ResourcePathSegment>> for AddressPattern {
    fn from(parts: Vec<ResourcePathSegment>) -> Self {
        Self {
            pattern: parts.iter().map(|p| p.clone().to_kind()).collect(),
        }
    }
}

impl From<Vec<ResourcePathSegmentKind>> for AddressPattern {
    fn from(parts: Vec<ResourcePathSegmentKind>) -> Self {
        Self { pattern: parts }
    }
}

pub struct KeyBit {
    pub key_type: String,
    pub id: u64,
}

pub struct KeyBits {
    pub key_type: String,
    pub bits: Vec<KeyBit>,
}

impl KeyBits {
    pub fn parse_key_bits(input: &str) -> Res<&str, Vec<KeyBit>> {
        context(
            "key-bits",
            separated_list1(nom::character::complete::char(':'), Self::parse_key_bit),
        )(input)
    }

    pub fn parse_key_bit(input: &str) -> Res<&str, KeyBit> {
        context("key-bit", tuple((alpha1, digit1)))(input).map(|(input, (key_type, id))| {
            (
                input,
                KeyBit {
                    key_type: key_type.to_string(),
                    id: id.parse().unwrap(), // should not have an error since we know it is a digit
                },
            )
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Path {
    string: String,
}

impl Path {
    fn new(string: &str) -> Self {
        Path {
            string: string.to_string(),
        }
    }

    pub fn make_absolute(string: &str) -> Result<Self, Error> {
        if string.starts_with("/") {
            Path::from_str(string)
        } else {
            Path::from_str(format!("/{}", string).as_str())
        }
    }

    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let bin = bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn is_absolute(&self) -> bool {
        self.string.starts_with("/")
    }

    pub fn cat(&self, path: &Path) -> Result<Self, Error> {
        if self.string.ends_with("/") {
            Path::from_str(format!("{}{}", self.string.as_str(), path.string.as_str()).as_str())
        } else {
            Path::from_str(format!("{}/{}", self.string.as_str(), path.string.as_str()).as_str())
        }
    }

    pub fn parent(&self) -> Option<Path> {
        let mut copy = self.string.clone();
        if copy.len() <= 1 {
            return Option::None;
        }
        copy.remove(0);
        let split = self.string.split("/");
        if split.count() <= 1 {
            Option::None
        } else {
            let mut segments = vec![];
            let mut split = copy.split("/");
            while let Option::Some(segment) = split.next() {
                segments.push(segment);
            }
            if segments.len() <= 1 {
                return Option::None;
            } else {
                segments.pop();
                let mut string = String::new();
                for segment in segments {
                    string.push_str("/");
                    string.push_str(segment);
                }
                Option::Some(Path::new(string.as_str()))
            }
        }
    }

    pub fn last_segment(&self) -> Option<String> {
        let split = self.string.split("/");
        match split.last() {
            None => Option::None,
            Some(last) => Option::Some(last.to_string()),
        }
    }

    pub fn to_relative(&self) -> String {
        let mut rtn = self.string.clone();
        rtn.remove(0);
        rtn
    }
}

impl Into<ResourcePathSegment> for Path {
    fn into(self) -> ResourcePathSegment {
        ResourcePathSegment::Path(self)
    }
}

impl TryInto<Arc<Vec<u8>>> for Path {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for Path {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<Self>(&value)?)
    }
}

impl TryFrom<&str> for Path {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Path::from_str(value)?)
    }
}

impl TryFrom<String> for Path {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Ok(Path::from_str(value.as_str())?)
    }
}

impl ToString for Path {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for Path {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover, path) = path(s)?;
        if !leftover.is_empty() {
            return Err(format!("could not parse '{}' from path {}", leftover, s).into());
        }
        Ok(path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Version {
    major: usize,
    minor: usize,
    patch: usize,
    release: Option<SkewerCase>,
}

impl Version {
    fn new(major: usize, minor: usize, patch: usize, release: Option<SkewerCase>) -> Self {
        Self {
            major,
            minor,
            patch,
            release,
        }
    }
}

impl Version {
    pub fn as_semver(&self) -> Result<semver::Version, Error> {
        Ok(semver::Version::parse(self.to_string().as_str())?)
    }
}

impl Into<ResourcePathSegment> for Version {
    fn into(self) -> ResourcePathSegment {
        ResourcePathSegment::Version(self)
    }
}

impl ToString for Version {
    fn to_string(&self) -> String {
        match &self.release {
            None => {
                format!("{}.{}.{}", self.major, self.minor, self.patch)
            }
            Some(release) => {
                format!(
                    "{}.{}.{}-{}",
                    self.major,
                    self.minor,
                    self.patch,
                    release.to_string()
                )
            }
        }
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (remaining, version) = version(s)?;
        if remaining.len() > 0 {
            Err(format!(
                "could not parse '{}' portion for version string '{}",
                remaining, s
            )
            .into())
        } else {
            Ok(version)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::str::FromStr;

    use crate::{
        domain, DomainCase, KeyBits, parse_resource_path, path, ResourceAddress, ResourceAddressKind,
        ResourcePathSegment, SkewerCase, Specific, version,
    };
    use crate::{
        AppKey, DatabaseKey, DatabaseKind, ResourceKey, ResourceKind, ResourcePath, ResourceType,
        RootKey, SpaceKey, SubSpaceKey,
    };
    use crate::error::Error;

    #[test]
    fn test_kind() -> Result<(), Error> {
        let specific = Specific::from_str("mysql.org:mysql:innodb:1.0.1")?;
        let kind = DatabaseKind::Relational(specific);
        println!("kind: {}", kind.to_string());

        let parsed_kind = ResourceKind::from_str(kind.to_string().as_str())?;
        let parsed_kind: DatabaseKind = parsed_kind.try_into()?;
        assert_eq!(kind, parsed_kind);
        Ok(())
    }

    #[test]
    fn test_key_bit() -> Result<(), Error> {
        let (leftover, bit) = KeyBits::parse_key_bit("ss0")?;

        assert_eq!(leftover.len(), 0);

        assert_eq!(bit.key_type, "ss".to_string());
        assert_eq!(bit.id, 0);

        Ok(())
    }

    #[test]
    fn test_key_bits() -> Result<(), Error> {
        let (leftover, bits) = KeyBits::parse_key_bits("ss0:e53:sub73")?;

        assert_eq!(leftover.len(), 0);

        let bit = bits.get(0).unwrap();
        assert_eq!(bit.key_type, "ss".to_string());
        assert_eq!(bit.id, 0);

        let bit = bits.get(1).unwrap();
        assert_eq!(bit.key_type, "e".to_string());
        assert_eq!(bit.id, 53);

        let bit = bits.get(2).unwrap();
        assert_eq!(bit.key_type, "sub".to_string());
        assert_eq!(bit.id, 73);

        Ok(())
    }

    #[test]
    fn test_key() -> Result<(), Error> {
        let space_key = SpaceKey::new(RootKey::new(), 0);
        let space_key: ResourceKey = space_key.into();
        println!("space_key.to_string() {}", space_key.to_string());
        let sub_space_key = SubSpaceKey::new(space_key.try_into()?, 0);
        let sub_space_key: ResourceKey = sub_space_key.into();
        println!("sub_space_key.to_string() {}", sub_space_key.to_string());
        let app_key = AppKey::new(sub_space_key.try_into()?, 1);
        let app_key_cp = app_key.clone();
        let app_key: ResourceKey = app_key.into();
        println!("app_key.to_string() {}", app_key.to_string());
        let db_key = DatabaseKey::new(app_key.try_into()?, 77);
        println!("db_key.to_string() {}", db_key.to_string());
        let db_key: ResourceKey = db_key.into();
        println!("db_key.to_snake_case() {}", db_key.to_snake_case());
        println!("db_key.to_skewer_case() {}", db_key.to_skewer_case());

        let db_key2 = ResourceKey::from_str(db_key.to_string().as_str())?;

        assert_eq!(db_key, db_key2);

        let app_key: AppKey = db_key.parent().unwrap().try_into()?;
        println!("parent {}", app_key.to_string());

        assert_eq!(app_key_cp, app_key);

        Ok(())
    }

    #[test]
    fn test_version() -> Result<(), Error> {
        let (leftover, version) = version("1.3.4-beta")?;
        assert_eq!(leftover.len(), 0);

        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 3);
        assert_eq!(version.patch, 4);
        assert_eq!(version.release, Option::Some(SkewerCase::new("beta")));

        Ok(())
    }
    #[test]
    fn test_path() -> Result<(), Error> {
        let (leftover, path) = path("/end/of-the/World.xyz")?;

        assert_eq!(leftover.len(), 0);

        assert_eq!("/end/of-the/World.xyz".to_string(), path.to_string());

        Ok(())
    }

    #[test]
    fn test_domain() -> Result<(), Error> {
        let (leftover, domain) = domain("hello-kitty.com")?;

        assert_eq!(leftover.len(), 0);

        assert_eq!("hello-kitty.com".to_string(), domain.to_string());

        Ok(())
    }

    #[test]
    fn address_path() -> Result<(), Error> {
        let (leftover, address) =
            parse_resource_path("starlane.io:some-skewer-case:1.3.4-beta:/the/End/of/the.World")?;

        assert_eq!(
            address.get(0),
            Option::Some(&ResourcePathSegment::Domain(DomainCase::new("starlane.io")))
        );
        assert_eq!(
            address.get(1),
            Option::Some(&ResourcePathSegment::SkewerCase(SkewerCase::new(
                "some-skewer-case"
            )))
        );
        assert_eq!(leftover.len(), 0);

        if let ResourcePathSegment::Version(version) = address.get(2).cloned().unwrap() {
            assert_eq!(version.major, 1);
            assert_eq!(version.minor, 3);
            assert_eq!(version.patch, 4);
            assert_eq!(version.release, Option::Some(SkewerCase::new("beta")));
        } else {
            assert!(false);
        }

        if let ResourcePathSegment::Path(path) = address.get(3).cloned().unwrap() {
            assert_eq!("/the/End/of/the.World".to_string(), path.to_string());
        } else {
            assert!(false);
        }

        Ok(())
    }

    #[test]
    fn test_address_parent_resolution() -> Result<(), Error> {
        let path = ResourcePath::from_str(
            "space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>",
        )?;
        let parent = path.parent().unwrap();
        assert_eq!(parent.resource_type(), ResourceType::App);

        let path = ResourcePath::from_str("space:sub-space:database<Database<Relational>>")?;
        let parent = path.parent().unwrap();

        assert_eq!(parent.resource_type(), ResourceType::SubSpace);

        Ok(())
    }

    #[test]
    fn test_address_kind() -> Result<(), Error> {
        let address = ResourceAddressKind::from_str(
            "space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>",
        )?;
        assert_eq!(
            "space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>"
                .to_string(),
            address.to_string()
        );

        Ok(())
    }

    #[test]
    fn test_resource_address() -> Result<(), Error> {
        let address = ResourceAddress::from_str(
            "space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>",
        )?;
        assert_eq!(
            "space:sub-space:some-app:database<Database>".to_string(),
            address.to_string()
        );

        Ok(())
    }
}

pub enum ResourceStatePersistenceManager {
    None,
    Store,
    Host,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ResourceIdentifier {
    Key(ResourceKey),
    Address(ResourceAddress),
}

impl ResourceIdentifier {
    pub fn is_key(&self) -> bool {
        match self {
            ResourceIdentifier::Key(_) => true,
            ResourceIdentifier::Address(_) => false,
        }
    }

    pub fn is_address(&self) -> bool {
        match self {
            ResourceIdentifier::Key(_) => false,
            ResourceIdentifier::Address(_) => true,
        }
    }

    pub fn key_or(self, error_message: &str) -> Result<ResourceKey, Error> {
        match self {
            ResourceIdentifier::Key(key) => Ok(key),
            ResourceIdentifier::Address(_) => Err(error_message.into()),
        }
    }

    pub fn address_or(self, error_message: &str) -> Result<ResourceAddress, Error> {
        match self {
            ResourceIdentifier::Key(_) => Err(error_message.into()),
            ResourceIdentifier::Address(address) => Ok(address),
        }
    }

    /*
    pub async fn to_key(mut self, starlane_api: &StarlaneApi ) -> Result<ResourceKey,Error> {
        match self{
            ResourceIdentifier::Key(key) => {Ok(key)}
            ResourceIdentifier::Address(address) => {
                Ok(starlane_api.fetch_resource_key(address).await?)
            }
        }
    }

    pub async fn to_address(mut self, starlane_api: &StarlaneApi ) -> Result<ResourceAddress,Error> {
        match self{
            ResourceIdentifier::Address(address) => {Ok(address)}
            ResourceIdentifier::Key(key) => {
                Ok(starlane_api.fetch_resource_address(key).await?)
            }
        }
    }

     */
}

impl ResourceIdentifier {
    pub fn parent(&self) -> Option<ResourceIdentifier> {
        match self {
            ResourceIdentifier::Key(key) => match key.parent() {
                None => Option::None,
                Some(parent) => Option::Some(parent.into()),
            },
            ResourceIdentifier::Address(address) => match address.parent() {
                None => Option::None,
                Some(parent) => Option::Some(parent.into()),
            },
        }
    }

    pub fn resource_type(&self) -> ResourceType {
        match self {
            ResourceIdentifier::Key(key) => key.resource_type(),
            ResourceIdentifier::Address(address) => address.resource_type(),
        }
    }
}

impl From<ResourceAddress> for ResourceIdentifier {
    fn from(address: ResourceAddress) -> Self {
        ResourceIdentifier::Address(address)
    }
}

impl From<ResourceKey> for ResourceIdentifier {
    fn from(key: ResourceKey) -> Self {
        ResourceIdentifier::Key(key)
    }
}

/*
impl TryInto<ResourceKey> for ResourceIdentifier {
    type Error = Error;

    fn try_into(self) -> Result<ResourceKey, Self::Error> {
        match self {
            ResourceIdentifier::Key(key) => Ok(key),
            ResourceIdentifier::Address(address) => Err(format!("resource identifier is not a key.  Instead got address: {}",address.to_string()).into()),
        }
    }
}

 */

impl TryInto<ResourceAddress> for ResourceIdentifier {
    type Error = Error;

    fn try_into(self) -> Result<ResourceAddress, Self::Error> {
        match self {
            ResourceIdentifier::Key(_) => Err("resource identifier is not an address".into()),
            ResourceIdentifier::Address(address) => Ok(address),
        }
    }
}

impl ToString for ResourceIdentifier {
    fn to_string(&self) -> String {
        match self {
            ResourceIdentifier::Key(key) => key.to_string(),
            ResourceIdentifier::Address(address) => address.to_string(),
        }
    }
}

resources! {
    #[resource(parents(Root))]
    #[resource(prefix="s")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Store)]
    #[resource(state(meta::Meta))]
    pub struct Space();

    #[resource(parents(Space))]
    #[resource(prefix="ss")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Store)]
    #[resource(state(meta::Meta))]
    pub struct SubSpace();

    #[resource(parents(SubSpace))]
    #[resource(prefix="app")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::None)]
    pub struct App();

    #[resource(parents(App))]
    #[resource(prefix="mt")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::None)]
    pub struct Mechtron();

    #[resource(parents(SubSpace,App))]
    #[resource(prefix="fs")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct FileSystem();

    #[resource(parents(FileSystem))]
    #[resource(prefix="f")]
    #[resource(ResourcePathSegmentKind::Path)]
    #[resource(ResourceStatePersistenceManager::Host)]
    #[resource(state(meta::Meta))]
    #[resource(state(content::Binary))]
    pub struct File();

    #[resource(parents(SubSpace,App))]
    #[resource(prefix="db")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct Database();

    #[resource(parents(SubSpace,App))]
    #[resource(prefix="auth")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct Auth();


    #[resource(parents(Root))]
    #[resource(prefix="d")]
    #[resource(ResourcePathSegmentKind::Domain)]
    #[resource(ResourceStatePersistenceManager::None)]
    pub struct Domain();

    #[resource(parents(SubSpace))]
    #[resource(prefix="p")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::None)]
    pub struct Proxy();

    #[resource(parents(SubSpace))]
    #[resource(prefix="abv")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::None)]
    #[resource(state(content::Binary))]
    pub struct ArtifactBundleVersions();

    #[resource(parents(ArtifactBundleVersions))]
    #[resource(prefix="ab")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    #[resource(state(content::Binary))]
    pub struct ArtifactBundle();

    #[resource(parents(ArtifactBundle))]
    #[resource(prefix="a")]
    #[resource(ResourcePathSegmentKind::Path)]
    #[resource(ResourceStatePersistenceManager::Host)]
    #[resource(state(content::Binary))]
    pub struct Artifact();

    #[resource(parents(Space))]
    #[resource(prefix="ub")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct UserBase();


    #[resource(parents(Space))]
    #[resource(prefix="u")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct User();



    #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
    pub enum DatabaseKind{
        Relational(Specific)
    }

    #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
    pub enum AuthKind{
        OAuth(Specific)
    }


    #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
    pub enum FileKind{
        Directory,
        File
    }


    #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
    pub enum ArtifactKind{
        Raw,
        DomainConfig,
        AppConfig,
        MechtronConfig,
        BindConfig,
        Wasm,
    }

    #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
    pub enum ArtifactBundleKind{
        Final,
        Volatile
    }


}

impl ArtifactPath {
    pub fn path(&self) -> Path {
        if let ResourcePathSegment::Path(path) = self.parts.last().unwrap() {
            path.clone()
        } else {
            panic!("expected last segment to be a path")
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetaSelector {
    None,
    Name(String),
    Label(LabelSelector),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelSelector {
    pub labels: HashSet<LabelSelection>,
}

impl ResourceSelector {
    pub fn new() -> Self {
        let fields = HashSet::new();
        ResourceSelector {
            meta: MetaSelector::None,
            fields: fields,
        }
    }

    pub fn resource_types(&self) -> HashSet<ResourceType> {
        let mut rtn = HashSet::new();
        for field in &self.fields {
            if let FieldSelection::Type(resource_type) = field {
                rtn.insert(resource_type.clone());
            }
        }
        rtn
    }

    pub fn add(&mut self, field: FieldSelection) {
        self.fields.retain(|f| !f.is_matching_kind(&field));
        self.fields.insert(field);
    }

    pub fn is_empty(&self) -> bool {
        if !self.fields.is_empty() {
            return false;
        }

        match &self.meta {
            MetaSelector::None => {
                return true;
            }
            MetaSelector::Name(_) => {
                return false;
            }
            MetaSelector::Label(labels) => {
                return labels.labels.is_empty();
            }
        };
    }

    pub fn name(&mut self, name: String) -> Result<(), Error> {
        match &mut self.meta {
            MetaSelector::None => {
                self.meta = MetaSelector::Name(name.clone());
                Ok(())
            }
            MetaSelector::Name(_) => {
                self.meta = MetaSelector::Name(name.clone());
                Ok(())
            }
            MetaSelector::Label(_selector) => {
                Err("Selector is already set to a LABEL meta selector".into())
            }
        }
    }

    pub fn add_label(&mut self, label: LabelSelection) -> Result<(), Error> {
        match &mut self.meta {
            MetaSelector::None => {
                self.meta = MetaSelector::Label(LabelSelector {
                    labels: HashSet::new(),
                });
                self.add_label(label)
            }
            MetaSelector::Name(_) => Err("Selector is already set to a NAME meta selector".into()),
            MetaSelector::Label(selector) => {
                selector.labels.insert(label);
                Ok(())
            }
        }
    }

    pub fn add_field(&mut self, field: FieldSelection) {
        self.fields.insert(field);
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum LabelSelection {
    Exact(Label),
}

impl LabelSelection {
    pub fn exact(name: &str, value: &str) -> Self {
        LabelSelection::Exact(Label {
            name: name.to_string(),
            value: value.to_string(),
        })
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldSelection {
    Identifier(ResourceIdentifier),
    Type(ResourceType),
    Kind(ResourceKind),
    Specific(Specific),
    Owner(UserKey),
    Parent(ResourceIdentifier),
}



impl ToString for FieldSelection {
    fn to_string(&self) -> String {
        match self {
            FieldSelection::Identifier(id) => id.to_string(),
            FieldSelection::Type(rt) => rt.to_string(),
            FieldSelection::Kind(kind) => kind.to_string(),
            FieldSelection::Specific(specific) => specific.to_string(),
            FieldSelection::Owner(owner) => owner.to_string(),
            FieldSelection::Parent(parent) => parent.to_string(),
        }
    }
}

impl FieldSelection {
    pub fn is_matching_kind(&self, field: &FieldSelection) -> bool {
        match self {
            FieldSelection::Identifier(_) => {
                if let FieldSelection::Identifier(_) = field {
                    return true;
                }
            }
            FieldSelection::Type(_) => {
                if let FieldSelection::Type(_) = field {
                    return true;
                }
            }
            FieldSelection::Kind(_) => {
                if let FieldSelection::Kind(_) = field {
                    return true;
                }
            }
            FieldSelection::Specific(_) => {
                if let FieldSelection::Specific(_) = field {
                    return true;
                }
            }
            FieldSelection::Owner(_) => {
                if let FieldSelection::Owner(_) = field {
                    return true;
                }
            }
            FieldSelection::Parent(_) => {
                if let FieldSelection::Parent(_) = field {
                    return true;
                }
            }
        };
        return false;
    }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceArchetype {
    pub kind: ResourceKind,
    pub specific: Option<Specific>,
    pub config: Option<ConfigSrc>,
}

impl ResourceArchetype {
    pub fn from_resource_type(kind: ResourceKind) -> Self {
        ResourceArchetype {
            kind: kind,
            specific: Option::None,
            config: Option::None,
        }
    }

    pub fn root() -> ResourceArchetype {
        ResourceArchetype {
            kind: ResourceKind::Root,
            specific: Option::None,
            config: Option::None,
        }
    }

    pub fn valid(&self) -> Result<(), Fail> {
        if self.kind.resource_type() == ResourceType::Root {
            return Err(Fail::CannotCreateNothingResourceTypeItIsThereAsAPlaceholderDummy);
        }
        Ok(())
    }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCreate {
    pub parent: ResourceIdentifier,
    pub key: KeyCreationSrc,
    pub address: AddressCreationSrc,
    pub archetype: ResourceArchetype,
    pub state_src: AssignResourceStateSrc<DataSet<BinSrc>>,
    pub registry_info: Option<ResourceRegistryInfo>,
    pub owner: Option<UserKey>,
    pub strategy: ResourceCreateStrategy,
}

impl ResourceCreate {
    pub fn create(
        archetype: ResourceArchetype,
        state_src: AssignResourceStateSrc<DataSet<BinSrc>>,
    ) -> Self {
        ResourceCreate {
            parent: ResourceKey::Root.into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::None,
            archetype: archetype,
            state_src: state_src,
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::Create,
        }
    }

    pub fn ensure_address(
        archetype: ResourceArchetype,
        src: AssignResourceStateSrc<DataSet<BinSrc>>,
    ) -> Self {
        ResourceCreate {
            parent: ResourceKey::Root.into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::None,
            archetype: archetype,
            state_src: src,
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::Ensure,
        }
    }

    pub fn validate(&self) -> Result<(), Fail> {
        let resource_type = self.archetype.kind.resource_type();

        self.archetype.valid()?;

        if let KeyCreationSrc::Key(key) = &self.key {
            if key.resource_type() != resource_type {
                return Err(Fail::ResourceTypeMismatch("ResourceCreate: key: KeyCreationSrc::Key(key) resource type != init.archetype.kind.resource_type()".into()));
            }
        }

        Ok(())
    }



    pub fn keyed_or(self, message: &str) -> Result<Self, Error> {
        if self.parent.is_key() {
            return Ok(self);
        } else {
            Err(message.into())
        }
    }
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceStatus {
    Unknown,
    Preparing,
    Ready,
}

impl ToString for ResourceStatus {
    fn to_string(&self) -> String {
        match self {
            Self::Unknown => "Unknown".to_string(),
            Self::Preparing => "Preparing".to_string(),
            Self::Ready => "Ready".to_string(),
        }
    }
}

impl FromStr for ResourceStatus {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Unknown" => Ok(Self::Unknown),
            "Preparing" => Ok(Self::Preparing),
            "Ready" => Ok(Self::Ready),
            what => Err(format!("not recognized: {}", what).into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AddressCreationSrc {
    None,
    Append(String),
    Appends(Vec<String>),
    Just(String),
    Exact(ResourceAddress),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyCreationSrc {
    None,
    Key(ResourceKey),
}

#[derive(Clone, Serialize, Deserialize)]
pub enum KeySrc {
    None,
    Key(ResourceKey),
    Address(ResourceAddress),
}

/// can have other options like to Initialize the state data
#[derive(Debug, Clone, Serialize, Deserialize, strum_macros::Display)]
pub enum AssignResourceStateSrc<DATASET> {
    Stateless,
    Direct(DATASET),
    CreateArgs(String),
}

impl TryInto<LocalStateSetSrc> for AssignResourceStateSrc<DataSet<BinSrc>> {
    type Error = Error;

    fn try_into(self) -> Result<LocalStateSetSrc, Self::Error> {
        match self {
            AssignResourceStateSrc::Direct(state) => Ok(LocalStateSetSrc::Some(state.try_into()?)),
            AssignResourceStateSrc::Stateless => Ok(LocalStateSetSrc::None),
            _ => Err(format!("cannot turn {}", self.to_string()).into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceStub {
    pub key: ResourceKey,
    pub address: ResourceAddress,
    pub archetype: ResourceArchetype,
    pub owner: Option<UserKey>,
}

impl ResourceStub {
    pub fn root() -> ResourceStub {
        ResourceStub {
            key: ResourceKey::Root,
            address: ResourceAddress::root(),
            archetype: ResourceArchetype::root(),
            owner: Option::None,
        }
    }
}



impl From<Resource> for ResourceStub {
    fn from(resource: Resource) -> Self {
        ResourceStub {
            key: resource.key,
            address: resource.address,
            archetype: resource.archetype,
            owner: resource.owner,
        }
    }
}

impl ResourceStub {
    pub fn validate(&self, resource_type: ResourceType) -> bool {
        self.key.resource_type() == resource_type
            && self.address.resource_type() == resource_type
            && self.archetype.kind.resource_type() == resource_type
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssignKind {
    Create,
    // eventually we want to allow for Assignments where things are 'Moved' as well
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAssign<S> {
    pub kind: AssignKind,
    pub stub: ResourceStub,
    pub state_src: S,
}

impl<S> ResourceAssign<S> {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn archetype(&self) -> ResourceArchetype {
        self.stub.archetype.clone()
    }
}

#[derive(Clone)]
pub struct Resource {
    key: ResourceKey,
    address: ResourceAddress,
    archetype: ResourceArchetype,
    state_src: DataSet<BinSrc>,
    owner: Option<UserKey>,
}

impl Resource {
    pub fn new(
        key: ResourceKey,
        address: ResourceAddress,
        archetype: ResourceArchetype,
        state_src: DataSet<BinSrc>,
    ) -> Resource {
        Resource {
            key: key,
            address: address,
            state_src: state_src,
            archetype: archetype,
            owner: Option::None, // fix later
        }
    }

    pub fn key(&self) -> ResourceKey {
        self.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.address.clone()
    }

    pub fn resource_type(&self) -> ResourceType {
        self.key.resource_type()
    }

    pub fn state_src(&self) -> DataSet<BinSrc> {
        self.state_src.clone()
    }
}

impl From<DataSet<BinSrc>> for LocalStateSetSrc {
    fn from(src: DataSet<BinSrc>) -> Self {
        LocalStateSetSrc::Some(src)
    }
}

#[derive(Clone)]
pub enum LocalStateSetSrc {
    None,
    Some(DataSet<BinSrc>),
    AlreadyHosted,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum RemoteDataSrc {
    None,
    Memory(Arc<Vec<u8>>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceCreateStrategy {
    Create,
    Ensure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Unique {
    Sequence,
    Index,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSelector {
    pub meta: MetaSelector,
    pub fields: HashSet<FieldSelection>,
}

impl ResourceSelector {

    pub fn children_selector(parent: ResourceIdentifier) -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Parent(parent));
        selector
    }

    pub fn children_of_type_selector(parent: ResourceIdentifier, child_type: ResourceType) -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Parent(parent));
        selector.add_field(FieldSelection::Type(child_type));
        selector
    }

    pub fn app_selector() -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Type(ResourceType::App));
        selector
    }

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigSrc {
    None,
    Artifact(ArtifactPath)
}

impl ToString for ConfigSrc {
    fn to_string(&self) -> String {
        match self {
            ConfigSrc::None => {
                "None".to_string()
            }
            ConfigSrc::Artifact(address) => {
                let address: ResourceAddress = address.clone().into();
                address.to_string()
            }
        }
    }
}

impl FromStr for ConfigSrc {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if "None" == s {
            Ok(Self::None)
        } else {
            let address = ResourceAddress::from_str(s)?;
            let address: ArtifactPath = address.try_into()?;
            Ok(Self::Artifact(address))
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LabelConfig {
    pub name: String,
    pub index: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRegistryInfo {
    pub names: Names,
    pub labels: Labels,
}

impl ResourceRegistryInfo {
    pub fn new() -> Self {
        ResourceRegistryInfo {
            names: Names::new(),
            labels: Labels::new(),
        }
    }
}

pub type Labels = HashMap<String, String>;
pub type Names = Vec<String>;

impl From<&str> for Fail {
    fn from(str: &str) -> Self {
        Fail::Error(str.to_string())
    }
}
impl From<String> for Fail {
    fn from(str: String) -> Self {
        Fail::Error(str)
    }
}


impl From<()> for Fail {
    fn from(_error: ()) -> Self {
        Fail::Error("() From Error".to_string())
    }
}

impl From<std::io::Error> for Fail {
    fn from(error: std::io::Error) -> Self {
        Fail::Error(error.to_string())
    }
}

impl From<SemVerError> for Fail {
    fn from(error: SemVerError) -> Self {
        Fail::Error(error.to_string())
    }
}
