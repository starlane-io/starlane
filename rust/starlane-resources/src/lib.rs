use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;

use nom::{AsChar, InputTakeAtPosition, IResult};
use nom::branch::alt;
use nom::bytes::complete::{tag, take, take_until, take_while};
use nom::character::complete::{alpha0, alpha1, anychar, digit0, digit1, one_of};
use nom::character::is_digit;
use nom::combinator::{not, opt, eof};
use nom::error::{context, ErrorKind, ParseError, VerboseError};
use nom::multi::{many0, many1, many_m_n, separated_list1};
use nom::sequence::{delimited, preceded, terminated, tuple};
use serde::Deserialize;
use serde::Serialize;

use crate::error::Error;
use std::sync::Arc;

pub mod error;
mod parse;

pub type Domain = String;
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


fn host(input: &str) -> Res<&str, Domain> {
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

pub fn parse_address_path(input: &str) -> Res<&str, (Vec<ResourceAddressPart>)> {
    context(
        "address-path",
        separated_list1( nom::character::complete::char(':'), alt( (path_part,version_part,domain_part,skewer_part) ) )
    )(input)
}

pub fn parse_address(input: &str) -> Res<&str, (Vec<ResourceAddressPart>,ResourceKindParts)> {
    context(
        "address",
        tuple( (parse_address_path,parse_kind) ),
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


fn skewer_part( input: &str ) -> Res<&str, ResourceAddressPart> {
    context(
        "skewer-case-part",
        skewer
    )(input).map( |(input, skewer)|{
        (input, ResourceAddressPart::SkewerCase(skewer))
    })
}

fn version_part( input: &str ) -> Res<&str, ResourceAddressPart> {
    context(
        "version-part",
        version
    )(input).map( |(input, version )|{
        (input, ResourceAddressPart::Version(version))
    })
}

fn domain_part( input: &str ) -> Res<&str, ResourceAddressPart> {
    context(
        "domain-part",
       domain
    )(input).map( |(input, domain)|{
        (input, ResourceAddressPart::Domain(domain))
    })
}


fn path_part( input: &str ) -> Res<&str, ResourceAddressPart> {
    context(
        "path-part",
         path
    )(input).map( |(input, path)|{
        (input, ResourceAddressPart::Path(path))
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

impl Into<ResourceAddressPart> for SkewerCase {
    fn into(self) -> ResourceAddressPart {
        ResourceAddressPart::SkewerCase(self)
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


#[derive(Clone, Serialize, Deserialize, Eq, PartialEq,Hash)]
pub enum ResourceAddressPartKind {
    Domain,
    SkewerCase,
    Email,
    Version,
    Path,
}


impl ToString for ResourceAddressPartKind {
    fn to_string(&self) -> String {
        match self {
            ResourceAddressPartKind::Domain => "Domain".to_string(),
            ResourceAddressPartKind::SkewerCase => "Skewer".to_string(),
            ResourceAddressPartKind::Version => "Version".to_string(),
            ResourceAddressPartKind::Path => "Path".to_string(),
            ResourceAddressPartKind::Email => "Email".to_string(),
        }
    }
}

impl ResourceAddressPartKind {
    pub fn matches(&self, part: &ResourceAddressPart) -> bool {
        match part {
            ResourceAddressPart::SkewerCase(_) => {
                *self == Self::SkewerCase
            }
            ResourceAddressPart::Path(_) => *self == Self::Path,
            ResourceAddressPart::Version(_) => *self == Self::Version,
            ResourceAddressPart::Email(_) => *self == Self::Email,
            ResourceAddressPart::Domain(_) => *self == Self::Domain,
        }
    }

    pub fn from_str(&self, s: &str) -> Result<ResourceAddressPart, Error> {
        if s.contains(RESOURCE_ADDRESS_DELIM) {
            return Err(format!(
                "resource part cannot contain resource address delimeter '{}' as in '{}'",
                RESOURCE_ADDRESS_DELIM, s
            )
                .into());
        }
        match self {

            ResourceAddressPartKind::SkewerCase => {
                Ok(ResourceAddressPart::SkewerCase(SkewerCase::from_str(s)?))
            }

            ResourceAddressPartKind::Path => Ok(ResourceAddressPart::Path(Path::from_str(s)?)),
            ResourceAddressPartKind::Version => {
                Ok(ResourceAddressPart::Version(Version::from_str(s)?))
            }

            ResourceAddressPartKind::Email => {
                Ok(ResourceAddressPart::Email(
                    s.to_string().trim().to_lowercase(),
                ))
            }
           ResourceAddressPartKind::Domain => {
                Ok(ResourceAddressPart::Domain(DomainCase::from_str(s)?))
            }
        }
    }
}




#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ResourceAddressPart {
    SkewerCase(SkewerCase),
    Domain(DomainCase),
    Path(Path),
    Version(Version),
    Email(String),
}

impl ResourceAddressPart {
    pub fn to_kind(self) -> ResourceAddressPartKind {
        match self {
            ResourceAddressPart::SkewerCase(_) => ResourceAddressPartKind::SkewerCase,
            ResourceAddressPart::Domain(_) => ResourceAddressPartKind::Domain,
            ResourceAddressPart::Path(_) => ResourceAddressPartKind::Path,
            ResourceAddressPart::Version(_) => ResourceAddressPartKind::Version,
            ResourceAddressPart::Email(_) => ResourceAddressPartKind::Email,
        }
    }
}

impl ToString for ResourceAddressPart {
    fn to_string(&self) -> String {
        match self {
            ResourceAddressPart::SkewerCase(skewer) => skewer.to_string(),
            ResourceAddressPart::Path(path) => path.to_string(),
            ResourceAddressPart::Version(version) => version.to_string(),
            ResourceAddressPart::Email(email) => email.to_string(),
            ResourceAddressPart::Domain(domain) => domain.to_string(),
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
    pattern: Vec<ResourceAddressPartKind>
}

impl From<Vec<ResourceAddressPart>> for AddressPattern{
    fn from(parts: Vec<ResourceAddressPart>) -> Self {
        Self {
            pattern: parts.iter().map(|p| p.clone().to_kind()).collect()
        }
    }
}

impl From<Vec<ResourceAddressPartKind>> for AddressPattern{
    fn from(parts: Vec<ResourceAddressPartKind>) -> Self {
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
            many1( KeyBits::parse_key_bit )
        )(input)
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

impl Into<ResourceAddressPart> for Path {
    fn into(self) -> ResourceAddressPart {
        ResourceAddressPart::Path(self)
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

impl Into<ResourceAddressPart> for Version {
    fn into(self) -> ResourceAddressPart {
        ResourceAddressPart::Version(self)
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
    use crate::error::Error;
    use crate::{parse_address_path, ResourceAddressPart, SkewerCase, version, path, domain, DomainCase};


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
        let (leftover,address)= parse_address_path("starlane.io:some-skewer-case:1.3.4-beta:/the/End/of/the.World")?;

        assert_eq!(address.get(0), Option::Some(&ResourceAddressPart::Domain(DomainCase::new("starlane.io"))));
        assert_eq!(address.get(1), Option::Some(&ResourceAddressPart::SkewerCase(SkewerCase::new("some-skewer-case"))));
        assert_eq!(leftover.len(),0);

        if let ResourceAddressPart::Version(version) = address.get(2).cloned().unwrap() {
            assert_eq!(version.major, 1);
            assert_eq!(version.minor, 3);
            assert_eq!(version.patch, 4);
            assert_eq!(version.release, Option::Some(SkewerCase::new("beta")));
        } else {
            assert!(false);
        }


            if let ResourceAddressPart::Path(path) = address.get(3).cloned().unwrap() {
                assert_eq!("/the/End/of/the.World".to_string(), path.to_string() );
            } else {
                assert!(false);
            }

            Ok(())
    }
}
