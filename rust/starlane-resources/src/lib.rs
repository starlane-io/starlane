use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use std::sync::Arc;

use nom::{AsChar, InputTakeAtPosition, IResult};
use nom::branch::alt;
use nom::bytes::complete::{tag, take, take_until, take_while};
use nom::character::complete::{alpha0, alpha1, anychar, digit0, digit1, one_of};
use nom::character::is_digit;
use nom::combinator::{eof, not, opt};
use nom::error::{context, ErrorKind, ParseError, VerboseError};
use nom::multi::{many0, many1, many_m_n, separated_list1};
use nom::sequence::{delimited, preceded, terminated, tuple};
use serde::Deserialize;
use serde::Serialize;

use starlane_macros::resources;

use crate::error::Error;

pub mod error;
pub mod parse;






#[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq,Hash)]
pub struct ResourceAddress {
    path: ResourcePath
}

impl ResourceAddress {

    pub fn root() -> Self {
       ResourcePath::Root.into()
    }


    pub fn new( path: ResourcePath ) -> Self {
        Self {
            path: path
        }
    }

    pub fn append( &self, string: String, resource_type: ResourceType ) -> Result<Self,Error> {
        let address = format!("{}:{}<{}>",self.path.to_string(), string, resource_type.to_string() );
        let path = ResourcePath::from_str(address.as_str())?;
        Ok(Self{
            path: path
        })
    }

    pub fn parent(&self) -> Option<ResourceAddress> {
        match self.path.parent() {
            Option::None => Option::None,
            Option::Some(parent) => Option::Some(parent.into())
        }
    }

    pub fn resource_type(&self) -> ResourceType {
        self.path.resource_type()
    }

    pub fn to_parts_string(&self) -> String {
        self.path.to_string()
    }

}

impl ToString for ResourceAddress {
    fn to_string(&self) -> String {
        format!("{}<{}>", self.path.to_string(), self.path.resource_type().to_string())
    }
}

impl FromStr for ResourceAddress{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ResourcePath::from_str(s)?.into())
    }
}

impl From<ResourcePath> for ResourceAddress {
    fn from(path: ResourcePath) -> Self {
        Self{
            path: path
        }
    }
}




pub struct ResourceAddressKind {
    path: ResourcePath,
    kind: ResourceKind
}

impl ResourceAddressKind {
    pub fn new( path: ResourcePath, kind: ResourceKind ) -> Self {
        Self {
            path: path,
            kind: kind
        }
    }
}

impl ToString for ResourceAddressKind {
    fn to_string(&self) -> String {
        format!("{}{}", self.path.to_string(), self.kind.to_string())
    }
}

impl FromStr for ResourceAddressKind{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = ResourcePath::from_str(s)?;
        let (leftover,(_,kind)) = parse_path(s)?;
        Ok(Self{
            path: path,
            kind: kind.try_into()?
        })
    }
}


pub type Res<T, U> = IResult<T, U, VerboseError<T>>;

static RESOURCE_ADDRESS_DELIM : &str  = ":";

fn alphanumerichyphen1<T>(i: T) -> Res<T, T>
    where
        T: InputTakeAtPosition,
        <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-') && !(char_item.is_alpha() || char_item.is_dec_digit() )
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
            !(char_item == '/') && !(char_item == '.') && !(char_item == '_') && !(char_item == '-') && !(char_item.is_alpha() || char_item.is_dec_digit() )
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
            !(char_item == '.') && !(char_item == '/') && !(char_item == ':') && !(char_item == '-') && !(char_item.is_alpha() || char_item.is_dec_digit() )
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
            !(char_item == '-') && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit() )
        },
        ErrorKind::AlphaNumeric,
    )
}

fn path( input: &str ) -> Res<&str,Path> {
    context("path",
      preceded(tag("/"),pathchar))(input).map( |(input,path)| {
        let path = format!("/{}",path);
        let path = Path::new( path.as_str() );
        (input,path)
    } )
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
        Self{
            string: string.to_string()
        }
    }
}

impl FromStr for DomainCase {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (remaining,domain) = domain(s)?;
        if remaining.len() > 0 {
            Err(format!("remainig text '{}' when parsing domain: '{}'",remaining,s).into())
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


fn zero( input: &str ) -> Res<&str,&str> {
    context("zero", tag("0") )(input)
}




/*
fn integer( input: &str) -> Res<&str,String> {
    context( "int",
             alt( (tag("0"),tuple((one_of("123456789"), opt(digit1)) ))) )(input).map( |(input,output)|{})
}

 */

fn version_major_minor_patch(input: &str) -> Res<&str, (usize,usize,usize)> {
    context(
        "version_major_minor_patch",
        tuple((
            terminated(digit1, tag(".")),
            terminated(digit1, tag(".")),
            terminated(digit1, not(digit1)),
        )),
    )(input)
        .map(|(next_input, mut res)| (next_input, (res.0.parse().unwrap(), res.1.parse().unwrap(), res.2.parse().unwrap())))
}

fn version(input: &str) -> Res<&str, Version> {
    context(
        "version",
        tuple((
            version_major_minor_patch,
            opt(preceded(tag("-"), skewer)),
        )),
    )(input)
        .map(|(next_input, ((major,minor,patch),release))| {

            (
                next_input,
                Version::new(
                    major,
                    minor,
                    patch,
                    release
                )

            )
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
    )(input).map( |(input, (rt,more) )| {

        let kind = match &more {
            None => { Option::None }
            Some((kind,_)) => {
                Option::Some((*kind).clone().to_string())
            }
        };
        let spec = match &more {
            None => { Option::None }
            Some((_,Option::Some(spec))) => {
                Option::Some(spec.clone())
            }
            _ => Option::None
        };
        (input, ResourceKindParts {
            resource_type: rt.to_string(),
            kind: kind,
            specific: spec
        })
    } )
}


pub fn parse_key(input: &str) -> Res<&str, (Vec<ResourcePathSegment>)> {
    context(
        "key",
        separated_list1( nom::character::complete::char(':'), alt( (path_part,version_part,domain_part,skewer_part) ) )
    )(input)
}

pub fn parse_resource_path(input: &str) -> Res<&str, (Vec<ResourcePathSegment>)> {
    context(
        "address-path",
        separated_list1( nom::character::complete::char(':'), alt( (path_part,version_part,domain_part,skewer_part) ) )
    )(input)
}

pub fn parse_path(input: &str) -> Res<&str, (Vec<ResourcePathSegment>, ResourceKindParts)> {
    context(
        "address",
        tuple( (parse_resource_path, parse_kind) ),
    )(input)
}

fn skewer( input: &str ) -> Res<&str, SkewerCase > {
    context(
        "skewer-case",
        loweralphanumerichyphen1
    )(input).map( |(input, skewer)|{
        (input, SkewerCase::new(skewer))
    })
}


fn skewer_part( input: &str ) -> Res<&str, ResourcePathSegment> {
    context(
        "skewer-case-part",
        skewer
    )(input).map( |(input, skewer)|{
        (input, ResourcePathSegment::SkewerCase(skewer))
    })
}

fn version_part( input: &str ) -> Res<&str, ResourcePathSegment> {
    context(
        "version-part",
        version
    )(input).map( |(input, version )|{
        (input, ResourcePathSegment::Version(version))
    })
}

fn domain_part( input: &str ) -> Res<&str, ResourcePathSegment> {
    context(
        "domain-part",
       domain
    )(input).map( |(input, domain)|{
        (input, ResourcePathSegment::Domain(domain))
    })
}


fn path_part( input: &str ) -> Res<&str, ResourcePathSegment> {
    context(
        "path-part",
         path
    )(input).map( |(input, path)|{
        (input, ResourcePathSegment::Path(path))
    })
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Specific {
    pub vendor: DomainCase,
    pub product: String,
    pub variant: String,
    pub version: Version
}

impl ToString for Specific {
    fn to_string(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.vendor.to_string(), self.product, self.variant, self.version.to_string()
        )
    }
}

impl FromStr for Specific {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover, specific) = specific(s)?;
        if leftover.len() != 0 {
            Err(format!("could not process '{}' portion of specific '{}'", leftover, s).into())
        } else {
            Ok(specific)
        }
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceKindParts{
    pub resource_type: String,
    pub kind: Option<String>,
    pub specific: Option<Specific>
}


impl ToString for ResourceKindParts {
    fn to_string(&self) -> String {
        if self.specific.is_some() && self.kind.is_some(){
            format!("<{}<{}<{}>>>", self.resource_type, self.kind.as_ref().unwrap().to_string(), self.specific.as_ref().unwrap().to_string() )
        } else if self.kind.is_some() {
            format!("<{}<{}>>", self.resource_type, self.kind.as_ref().unwrap().to_string() )
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
            return Err(format!("ResourceKindParts ERROR: could not parse extra: '{}' in string '{}'", leftover, s ).into());
        }
        Ok(rtn)
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

impl ToString for SkewerCase{
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl SkewerCase {

    fn new( string: &str ) -> Self {
        Self{
            string: string.to_string()
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
        let (remaining,skewer) = skewer(s)?;
        if remaining.len() > 0 {
            Err(format!("could not parse skewer because of remaining: '{}' in skewer: '{}'",remaining,s).into() )
        } else {
            Ok(skewer)
        }
    }
}


#[derive(Debug,Clone, Serialize, Deserialize, Eq, PartialEq,Hash)]
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
            ResourcePathSegment::SkewerCase(_) => {
                *self == Self::SkewerCase
            }
            ResourcePathSegment::Path(_) => *self == Self::Path,
            ResourcePathSegment::Version(_) => *self == Self::Version,
            ResourcePathSegment::Email(_) => *self == Self::Email,
            ResourcePathSegment::Domain(_) => *self == Self::Domain,
        }
    }

    pub fn from_str(&self, s: &str) -> Result<ResourcePathSegment, Error> {
        let (leftover,part) = path_part(s)?;
        if leftover.len() > 0 {
            Err(format!("could not parse entire path string: leftover: '{}' from '{}'",leftover,s).into())
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
    patterns: HashMap<AddressPattern,T>,
}


impl <T> ParentAddressPatternRecognizer<T> {
    pub fn try_from( &self, pattern: &AddressPattern ) -> Result<T,Error>{
        unimplemented!()
//        self.patterns.get(pattern ).cloned().ok_or(Error{message:"Could not find a match for ParentAddressPatternRecognizer".to_string()})
    }
}

#[derive(Clone,Eq,PartialEq,Hash)]
pub struct AddressPattern{
    pattern: Vec<ResourcePathSegmentKind>
}

impl From<Vec<ResourcePathSegment>> for AddressPattern{
    fn from(parts: Vec<ResourcePathSegment>) -> Self {
        Self {
            pattern: parts.iter().map(|p| p.clone().to_kind()).collect()
        }
    }
}

impl From<Vec<ResourcePathSegmentKind>> for AddressPattern{
    fn from(parts: Vec<ResourcePathSegmentKind>) -> Self {
        Self {
            pattern: parts
        }
    }
}

pub struct KeyBit{
    pub key_type: String,
    pub id: u64
}

pub struct KeyBits{
    pub key_type: String,
    pub bits: Vec<KeyBit>
}


impl KeyBits{


    pub fn parse_key_bits(input: &str) -> Res<&str, Vec<KeyBit>> {
        context(
            "key-bits",

            separated_list1( nom::character::complete::char(':'), Self::parse_key_bit ) )(input)
    }

    pub fn parse_key_bit(input: &str) -> Res<&str, KeyBit> {
        context(
            "key-bit",
            tuple( (alpha1, digit1) ),
        )(input).map( |(input, (key_type,id))|{
            (input,
             KeyBit{
                 key_type: key_type.to_string(),
                 id: id.parse().unwrap() // should not have an error since we know it is a digit
             })
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

    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let mut bin = bincode::serialize(self)?;
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
        Ok(Path::from_str(s)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Version {
    major: usize,
    minor: usize,
    patch: usize,
    release: Option<SkewerCase>
}

impl Version {
    fn new(major: usize, minor:usize, patch:usize, release: Option<SkewerCase>) -> Self {
        Self{
            major,
            minor,
            patch,
            release
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
                format!("{}.{}.{}",self.major,self.minor,self.patch)
            }
            Some(release) => {
                format!("{}.{}.{}-{}",self.major,self.minor,self.patch,release.to_string())
            }
        }
    }
}

impl FromStr for Version {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (remaining,version) = version(s)?;
        if remaining.len() > 0 {
            Err(format!("could not parse '{}' portion for version string '{}", remaining, s).into())
        } else {
            Ok(version)
        }
    }
}


#[cfg(test)]
mod tests {
    use std::convert::TryInto;
    use std::str::FromStr;

    use crate::{domain, DomainCase, KeyBits, parse_resource_path, path, ResourceAddress, ResourceAddressKind, ResourcePathSegment, SkewerCase, Specific, version};
    use crate::{AppKey, DatabaseKey, DatabaseKind, DatabasePath, ResourceKey, ResourceKind, ResourcePath, ResourceType, RootKey, SpaceKey, SubSpaceKey};
    use crate::error::Error;

    #[test]
    fn test_kind() -> Result<(),Error> {
        let specific = Specific::from_str( "mysql.org:mysql:innodb:1.0.1")?;
        let kind = DatabaseKind::Relational(specific);
        println!("kind: {}", kind.to_string() );

        let parsed_kind = ResourceKind::from_str(kind.to_string().as_str())?;
        let parsed_kind:DatabaseKind = parsed_kind.try_into()?;
        assert_eq!(kind,parsed_kind);
        Ok(())
    }



        #[test]
    fn test_key_bit() -> Result<(),Error> {
        let (leftover,bit) = KeyBits::parse_key_bit("ss0")?;

        assert_eq!(leftover.len(),0);

        assert_eq!(bit.key_type,"ss".to_string());
        assert_eq!(bit.id,0);

        Ok(())
    }


    #[test]
    fn test_key_bits() -> Result<(),Error> {
        let (leftover,bits) = KeyBits::parse_key_bits("ss0:e53:sub73")?;

        assert_eq!(leftover.len(),0);

        let bit = bits.get(0).unwrap();
        assert_eq!(bit.key_type,"ss".to_string());
        assert_eq!(bit.id,0);

        let bit = bits.get(1).unwrap();
        assert_eq!(bit.key_type,"e".to_string());
        assert_eq!(bit.id,53);

        let bit = bits.get(2).unwrap();
        assert_eq!(bit.key_type,"sub".to_string());
        assert_eq!(bit.id,73);


        Ok(())
    }


    #[test]
    fn test_key() -> Result<(),Error>{
        let space_key = SpaceKey::new(RootKey::new(),0);
        let space_key: ResourceKey = space_key.into();
        println!("space_key.to_string() {}", space_key.to_string() );
        let sub_space_key = SubSpaceKey::new(space_key.try_into()?, 0 );
        let sub_space_key:ResourceKey  = sub_space_key.into();
        println!("sub_space_key.to_string() {}", sub_space_key.to_string() );
        let app_key = AppKey::new( sub_space_key.try_into()?, 1 );
        let app_key_cp = app_key.clone();
        let app_key: ResourceKey = app_key.into();
        println!("app_key.to_string() {}", app_key.to_string() );
        let db_key = DatabaseKey::new( app_key.try_into()?, 77 );
        println!("db_key.to_string() {}", db_key.to_string() );
        let db_key: ResourceKey = db_key.into();
        println!("db_key.to_snake_case() {}", db_key.to_snake_case() );
        println!("db_key.to_skewer_case() {}", db_key.to_skewer_case() );

        let db_key2 = ResourceKey::from_str(db_key.to_string().as_str())?;

        assert_eq!( db_key, db_key2 );

        let app_key: AppKey = db_key.parent().unwrap().try_into()?;
        println!("parent {}", app_key.to_string() );

        assert_eq!( app_key_cp, app_key );

        Ok(())
    }


        #[test]
    fn test_version() -> Result<(),Error>{
        let (leftover,version)= version("1.3.4-beta")?;
        assert_eq!(leftover.len(),0);

        assert_eq!( version.major, 1 );
        assert_eq!( version.minor, 3 );
        assert_eq!( version.patch, 4 );
        assert_eq!( version.release, Option::Some(SkewerCase::new("beta")) );

        Ok(())
    }
    #[test]
    fn test_path() -> Result<(),Error>{
        let (leftover,path)= path("/end/of-the/World.xyz")?;

        assert_eq!(leftover.len(),0);

        assert_eq!("/end/of-the/World.xyz".to_string(), path.to_string() );


        Ok(())
    }

    #[test]
    fn test_domain() -> Result<(),Error>{
        let (leftover,domain)= domain("hello-kitty.com")?;

        assert_eq!(leftover.len(),0);

        assert_eq!("hello-kitty.com".to_string(), domain.to_string() );


        Ok(())
    }

        #[test]
    fn address_path() -> Result<(),Error>{
        let (leftover,address)= parse_resource_path("starlane.io:some-skewer-case:1.3.4-beta:/the/End/of/the.World")?;

        assert_eq!(address.get(0), Option::Some(&ResourcePathSegment::Domain(DomainCase::new("starlane.io"))));
        assert_eq!(address.get(1), Option::Some(&ResourcePathSegment::SkewerCase(SkewerCase::new("some-skewer-case"))));
        assert_eq!(leftover.len(),0);

        if let ResourcePathSegment::Version(version) = address.get(2).cloned().unwrap() {
            assert_eq!(version.major, 1);
            assert_eq!(version.minor, 3);
            assert_eq!(version.patch, 4);
            assert_eq!(version.release, Option::Some(SkewerCase::new("beta")));
        } else {
            assert!(false);
        }


            if let ResourcePathSegment::Path(path) = address.get(3).cloned().unwrap() {
                assert_eq!("/the/End/of/the.World".to_string(), path.to_string() );
            } else {
                assert!(false);
            }

            Ok(())
    }

    #[test]
    fn test_address_parent_resolution( ) -> Result<(),Error>{

        let path = ResourcePath::from_str( "space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>")?;
        let parent = path.parent()?.unwrap();
        assert_eq!( parent.resource_type(), ResourceType::App );

        let path = ResourcePath::from_str( "space:sub-space:database<Database<Relational>>")?;
        let parent = path.parent()?.unwrap();

        assert_eq!( parent.resource_type(), ResourceType::SubSpace );

        Ok(())
    }


    #[test]
    fn test_address_kind( ) -> Result<(),Error>{

        let address = ResourceAddressKind::from_str( "space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>")?;
        assert_eq!("space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>".to_string(), address.to_string() );

        Ok(())
    }


    #[test]
    fn test_resource_address( ) -> Result<(),Error>{

        let address = ResourceAddress::from_str( "space:sub-space:some-app:database<Database<Relational<mysql.org:mysql:innodb:1.0.0>>>")?;
        assert_eq!("space:sub-space:some-app:database<Database>".to_string(), address.to_string() );

        Ok(())
    }
}

pub enum ResourceStatePersistenceManager {
    None,
    Store,
    Host,
}





#[derive(Debug, Clone, Serialize, Deserialize,Eq,PartialEq,Hash)]
pub enum ResourceIdentifier {
    Key(ResourceKey),
    Address(ResourceAddress),
}

impl ResourceIdentifier{

    pub fn is_key(&self) -> bool {
        match self {
            ResourceIdentifier::Key(_) => {
                true
            }
            ResourceIdentifier::Address(_) => {
                false
            }
        }
    }

    pub fn is_address(&self) -> bool {
        match self {
            ResourceIdentifier::Key(_) => {
                false
            }
            ResourceIdentifier::Address(_) => {
                true
            }
        }
    }


    pub fn key_or(self,error_message: &str ) -> Result<ResourceKey,Error> {
        match self {
            ResourceIdentifier::Key(key) => {
                Ok(key)
            }
            ResourceIdentifier::Address(_) => {
                Err(error_message.into())
            }
        }
    }

    pub fn address_or(self,error_message: &str ) -> Result<ResourceAddress,Error> {
        match self {
            ResourceIdentifier::Key(_) => {
                Err(error_message.into())
            }
            ResourceIdentifier::Address(address) => {
                Ok(address)
            }
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
    pub struct Space();
    #[resource(parents(Space))]
    #[resource(prefix="ss")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Store)]
    pub struct SubSpace();

    #[resource(parents(SubSpace))]
    #[resource(prefix="app")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::None)]
    pub struct App();

    #[resource(parents(App))]
    #[resource(prefix="act")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::None)]
    pub struct Actor();

    #[resource(parents(SubSpace,App))]
    #[resource(prefix="fs")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct FileSystem();

    #[resource(parents(FileSystem))]
    #[resource(prefix="f")]
    #[resource(ResourcePathSegmentKind::Path)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct File();

    #[resource(parents(SubSpace,App))]
    #[resource(prefix="db")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct Database();

    #[resource(parents(Space))]
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
    pub struct ArtifactBundleVersions();

    #[resource(parents(ArtifactBundleVersions))]
    #[resource(prefix="ab")]
    #[resource(ResourcePathSegmentKind::SkewerCase)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct ArtifactBundle();

    #[resource(parents(ArtifactBundle))]
    #[resource(prefix="a")]
    #[resource(ResourcePathSegmentKind::Path)]
    #[resource(ResourceStatePersistenceManager::Host)]
    pub struct Artifact();

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
    pub enum FileKind{
        Directory,
        File
    }


    #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
    pub enum ArtifactKind{
        Raw,
        DomainConfig
    }

    #[derive(Clone,Debug,Eq,PartialEq,Hash,Serialize,Deserialize)]
    pub enum ArtifactBundleKind{
        Final,
        Volatile
    }


}

impl ArtifactPath{
    pub fn path(&self)->Path {
        if let ResourcePathSegment::Path( path ) = self.parts.last().unwrap() {
            path.clone()
        } else {
            panic!("expected last segment to be a path")
        }
    }
}

