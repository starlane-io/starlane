use std::convert::TryFrom;
use std::str::FromStr;

use nom::{AsChar, InputTakeAtPosition, IResult};
use nom::branch::alt;
use nom::bytes::complete::{tag, take, take_until, take_while};
use nom::character::complete::{alpha0, alpha1, anychar, digit0, digit1, one_of};
use nom::character::is_digit;
use nom::combinator::{not, opt};
use nom::error::{context, ErrorKind, ParseError, VerboseError};
use nom::multi::{many0, many1, many_m_n};
use nom::sequence::{delimited, preceded, terminated, tuple};
use serde::Deserialize;
use serde::Serialize;

use crate::error::Error;
use crate::resource::{Base64Encoded, DomainCase, Path, ResourceAddress, ResourceKind, ResourceType, Version, ResourceAddressPartKind};
use std::collections::HashMap;


pub type Domain = String;
type Res<T, U> = IResult<T, U, VerboseError<T>>;

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

fn domain(input: &str) -> Res<&str, Domain> {
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
        (next_input, res.0.join("."))
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

fn version_major_minor_patch(input: &str) -> Res<&str, String> {
    context(
        "version_major_minor_patch",
        tuple((
            terminated(digit1, tag(".")),
            terminated(digit1, tag(".")),
            terminated(digit1, not(digit1)),
        )),
    )(input)
    .map(|(next_input, mut res)| (next_input, format!("{}.{}.{}", res.0, res.1, res.2)))
}

fn version(input: &str) -> Res<&str, String> {
    context(
        "version",
        tuple((
            version_major_minor_patch,
            opt(preceded(tag("-"), loweralphanumerichyphen1)),
        )),
    )(input)
    .map(|(next_input, mut res)| {
        (
            next_input,
            match res.1 {
                None => res.0,
                Some(opt) => {
                    format!("{}-{}", res.0, opt)
                }
            },
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

pub fn parse_address(input: &str) -> Res<&str, (&str,ResourceKindParts)> {
    context(
        "address",
         tuple( (take_while(|c| c != '<'),parse_kind)),
    )(input)
}


#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Specific {
    pub vendor: Domain,
    pub product: String,
    pub variant: String,
    pub version: String,
}

impl ToString for Specific {
    fn to_string(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.vendor, self.product, self.variant, self.version
        )
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceKindParts{
    pub resource_type: String,
    pub kind: Option<String>,
    pub specific: Option<Specific>
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

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::resource::{DatabaseKind, ResourceAddress, ResourceKind, ResourceType};
    use crate::resource::address::{domain, host, parse_address, parse_kind, ResourceAddressKind, ResourceKindParts, specific, Specific, version, version_major_minor_patch};

    #[test]
    pub fn test_address_kind() {
        assert_eq!(
            ResourceAddressKind::from_str("space:sub-space:app:database<Database<Relational<mysql.org:mysql:innodb:7.0.1>>>"),
            Ok(ResourceAddressKind{
                    address: ResourceAddress::from_str("space:sub-space:app:database::<Database>").unwrap(),
                    kind: ResourceKind::Database(DatabaseKind::Relational(Specific{
                        vendor: "mysql.org".to_string(),
                        product: "mysql".to_string(),
                        variant: "innodb".to_string(),
                        version: "7.0.1".to_string()
                    }))
                })
        );


    }

    #[test]
    pub fn test_kind() {
        assert_eq!(
            parse_kind("<Database<Relational<mysql.org:mysql:innodb:7.0.1>>>"),
            Ok((
                "",
                ResourceKindParts{
                    resource_type: "Database".to_string(),
                    kind: Option::Some("Relational".to_string()),
                    specific: Option::Some(Specific {
                    vendor: "mysql.org".to_string(),
                    product: "mysql".to_string(),
                    variant: "innodb".to_string(),
                    version: "7.0.1".to_string()
                })}
            ))
        );

        assert_eq!(
            parse_kind("<Database<Relational>>"),
            Ok((
                "",
                ResourceKindParts{
                    resource_type: "Database".to_string(),
                    kind: Option::Some("Relational".to_string()),
                    specific: Option::None }
            ))
        );

        assert_eq!(
            parse_kind("<Database>"),
            Ok((
                "",
                ResourceKindParts{
                    resource_type: "Database".to_string(),
                    kind: Option::None,
                    specific: Option::None }
            ))
        );
    }


    #[test]
    pub fn test_specific() {
        assert_eq!(
            specific("mysql.org:mysql:innodb:7.0.1"),
            Ok((
                "",
                Specific {
                    vendor: "mysql.org".to_string(),
                    product: "mysql".to_string(),
                    variant: "innodb".to_string(),
                    version: "7.0.1".to_string()
                }
            ))
        );
    }

    #[test]
    pub fn test_version() {
        assert_eq!(
            version("1.24.3-beta|on and on"),
            Ok(("|on and on", "1.24.3-beta".to_string()))
        );

        assert_eq!(
            version("1.2.3~dogar and kazon"),
            Ok(("~dogar and kazon", "1.2.3".to_string()))
        );
    }

    #[test]
    pub fn test_version_major_minor_patch() {
        assert_eq!(
            version_major_minor_patch("55.2.3-beta"),
            Ok(("-beta", "55.2.3".to_string()))
        );

        assert_eq!(
            version_major_minor_patch("1.2.3"),
            Ok(("", "1.2.3".to_string()))
        );

       // assert!( version_major_minor_patch("01.2.3").is_err() )
    }

    #[test]
    pub fn test_domain() {
        assert_eq!(domain("mysql.org"), Ok(("", "mysql.org".to_string())));

        assert_eq!(domain("hello.com"), Ok(("", "hello.com".to_string())));

        assert_eq!(
            domain("abc.hello.com"),
            Ok(("", "abc.hello.com".to_string()))
        );

        assert_eq!(
            domain("abc.hello.com:the-zozo:"),
            Ok((":the-zozo:", "abc.hello.com".to_string()))
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SkewerCase {
    string: String,
}

impl SkewerCase {
    pub fn new(string: &str) -> Result<Self, Error> {
        if string.is_empty() {
            return Err("cannot be empty".into());
        }

        for c in string.chars() {
            if !((c.is_lowercase() && c.is_ascii_alphabetic()) || c.is_numeric() || c == '-') {
                return Err(format!("must be lowercase, use only alphanumeric characters & dashes RECEIVED: '{}'", string).into());
            }
        }
        Ok(SkewerCase {
            string: string.to_string(),
        })
    }
}

impl ToString for SkewerCase {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}

impl FromStr for SkewerCase {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(SkewerCase::new(s)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum ResourceAddressPart {
    Wildcard,
    SkewerCase(SkewerCase),
    Domain(DomainCase),
    Base64Encoded(Base64Encoded),
    Path(Path),
    Version(Version),
    Email(String),
    Url(String),
    UrlPathPattern(String),
}

impl ResourceAddressPart {
    pub fn to_kind(self) -> ResourceAddressPartKind {
        match self {
            ResourceAddressPart::Wildcard => ResourceAddressPartKind::Wildcard,
            ResourceAddressPart::SkewerCase(_) => ResourceAddressPartKind::SkewerCase,
            ResourceAddressPart::Domain(_) => ResourceAddressPartKind::Domain,
            ResourceAddressPart::Base64Encoded(_) => ResourceAddressPartKind::Base64Encoded,
            ResourceAddressPart::Path(_) => ResourceAddressPartKind::Path,
            ResourceAddressPart::Version(_) => ResourceAddressPartKind::Version,
            ResourceAddressPart::Email(_) => ResourceAddressPartKind::Email,
            ResourceAddressPart::Url(_) => ResourceAddressPartKind::Url,
            ResourceAddressPart::UrlPathPattern(_) => ResourceAddressPartKind::UrlPathPattern
        }
    }
}

impl ToString for ResourceAddressPart {
    fn to_string(&self) -> String {
        match self {
            ResourceAddressPart::Wildcard => "*".to_string(),
            ResourceAddressPart::SkewerCase(skewer) => skewer.to_string(),
            ResourceAddressPart::Base64Encoded(base64) => base64.encoded.clone(),
            ResourceAddressPart::Path(path) => path.to_string(),
            ResourceAddressPart::Version(version) => version.to_string(),
            ResourceAddressPart::Email(email) => email.to_string(),
            ResourceAddressPart::Url(url) => url.to_string(),
            ResourceAddressPart::UrlPathPattern(path) => path.to_string(),
            ResourceAddressPart::Domain(domain) => domain.to_string(),
        }
    }
}

impl ResourceAddressPart {
    pub fn is_wildcard(&self) -> bool {
        match self {
            ResourceAddressPart::Wildcard => true,
            _ => false,
        }
    }
}


pub struct ParentAddressPatternRecognizer {
    patterns: HashMap<AddressPattern,ResourceType>,
}

impl Default for ParentAddressPatternRecognizer {
    fn default() -> Self {
        let mut pat = HashMap::new();
        pat.insert( AddressPattern::from(vec![ResourceAddressPartKind::SkewerCase,ResourceAddressPartKind::SkewerCase]), ResourceType::SubSpace );
        pat.insert( AddressPattern::from(vec![ResourceAddressPartKind::SkewerCase,ResourceAddressPartKind::SkewerCase,ResourceAddressPartKind::SkewerCase]), ResourceType::App );

        Self {
            patterns:pat
        }
    }
}

impl ParentAddressPatternRecognizer {
    pub fn try_from( &self, pattern: &AddressPattern ) -> Result<ResourceType,Error>{
        self.patterns.get(pattern ).cloned().ok_or(Error{error:"Could not find a match for ParentAddressPatternRecognizer".to_string()})
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
