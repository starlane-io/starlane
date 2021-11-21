use crate::pattern::specific::{ProductPattern, VariantPattern, VendorPattern, VersionPattern};
use crate::{CamelCase, Error, ResourceType};
use semver::VersionReq;
use std::ops::Deref;
use std::str::FromStr;

#[derive(Eq, PartialEq)]
pub enum SegmentPattern {
    Any,       // *
    Recursive, // **
    Exact(ExactSegment),
}

pub type KeySegment = String;
pub type AddressSegment = String;

#[derive(Eq, PartialEq)]
pub enum ExactSegment {
    Key(KeySegment),
    Address(AddressSegment),
}

#[derive(Eq, PartialEq)]
pub struct Hop {
    pub segment: SegmentPattern,
    pub tks: TksPattern,
}

#[derive(Eq, PartialEq)]
pub enum Pattern<P> {
    Any,
    Exact(P),
}

impl Into<Pattern<String>> for Pattern<&str> {
    fn into(self) -> Pattern<String> {
        match self {
            Pattern::Any => Pattern::Any,
            Pattern::Exact(f) => Pattern::Exact(f.to_string()),
        }
    }
}

impl <P> ToString for Pattern<P> where P:ToString {
    fn to_string(&self) -> String {
        match self{
            Pattern::Any => {"*".to_string()}
            Pattern::Exact(exact) => {exact.to_string()}
        }
    }
}

pub type ResourceTypePattern = Pattern<CamelCase>;
pub type KindPattern = Pattern<CamelCase>;
pub mod specific {
    use crate::pattern::Pattern;
    use crate::{DomainCase, Error, SkewerCase};
    use semver::VersionReq;
    use std::ops::Deref;
    use std::str::FromStr;

    pub struct Version {
        pub req: VersionReq,
    }

    impl Deref for Version {
        type Target = VersionReq;

        fn deref(&self) -> &Self::Target {
            &self.req
        }
    }

    impl FromStr for Version {
        type Err = Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(Version {
                req: VersionReq::from_str(s)?,
            })
        }
    }

    pub type VendorPattern = Pattern<DomainCase>;
    pub type ProductPattern = Pattern<SkewerCase>;
    pub type VariantPattern = Pattern<SkewerCase>;
    pub type VersionPattern = Pattern<Version>;
}

#[derive(Eq, PartialEq)]
pub struct SpecificPattern {
    pub vendor: VendorPattern,
    pub product: ProductPattern,
    pub variant: VariantPattern,
    pub version: VersionReq,
}

impl ToString for SpecificPattern{
    fn to_string(&self) -> String {
        format!("{}:{}:{}:({})", self.vendor.to_string(), self.product.to_string(), self.variant.to_string(), self.version.to_string() )
    }
}

#[derive(Eq, PartialEq)]
pub struct TksPattern {
    pub resource_type: ResourceTypePattern,
    pub kind: KindPattern,
    pub specific: Pattern<SpecificPattern>,
}


impl TksPattern {
    pub fn new(resource_type: ResourceTypePattern, kind: KindPattern, specific: Pattern<SpecificPattern> ) -> Self {
        Self {
            resource_type,
            kind,
            specific
        }
    }
}

impl TksPattern {
    pub fn any() -> Self {
        Self {
            resource_type: ResourceTypePattern::Any,
            kind: KindPattern::Any,
            specific: Pattern::Any,
        }
    }
}

#[derive(Eq, PartialEq)]
pub struct ResourcePattern {
    pub hops: Vec<Hop>,
}

pub mod parse {
    use crate::parse::any_resource_path_segment;
    use crate::pattern::specific::VersionPattern;
    use crate::pattern::{
        AddressSegment, ExactSegment, Hop, KindPattern, Pattern, ResourceTypePattern,
        SegmentPattern, SpecificPattern, TksPattern,
    };
    use crate::{camel, domain, skewer, version_req, Error, Res, ResourceType};
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use nom::character::complete::{alpha1, digit1};
    use nom::combinator::{opt, recognize};
    use nom::error::VerboseError;
    use nom::sequence::{delimited, tuple};
    use nom::IResult;
    use nom::Parser;
    use nom_supreme::{parse_from_str, ParserExt};
    use semver::VersionReq;

    fn any_segment(input: &str) -> Res<&str, SegmentPattern> {
        tag("*")(input).map(|(next, _)| (next, SegmentPattern::Any))
    }

    fn recursive_segment(input: &str) -> Res<&str, SegmentPattern> {
        tag("**")(input).map(|(next, _)| (next, SegmentPattern::Recursive))
    }

    fn exact_segment(input: &str) -> Res<&str, SegmentPattern> {
        any_resource_path_segment(input).map(|(next, segment)| {
            (
                next,
                SegmentPattern::Exact(ExactSegment::Address(segment.to_string())),
            )
        })
    }

    fn segment(input: &str) -> Res<&str, SegmentPattern> {
        alt((recursive_segment, any_segment, exact_segment))(input)
    }

    fn pattern<P>(
        parse: fn(input: &str) -> Res<&str, P>,
    ) -> impl Fn(&str) -> Res<&str, Pattern<P>> {
        move |input: &str| match tag::<&str, &str, VerboseError<&str>>("*")(input) {
            Ok((next, _)) => Ok((next, Pattern::Any)),
            Err(_) => {
                let (next, p) = parse(input)?;
                let pattern = Pattern::Exact(p);
                Ok((next, pattern))
            }
        }
    }

    fn version(input: &str) -> Res<&str, VersionReq> {
        parse_from_str(version_req).parse(input)
    }

    fn specific(input: &str) -> Res<&str, SpecificPattern> {
        tuple((
            pattern(domain),
            tag(":"),
            pattern(skewer),
            tag(":"),
            pattern(skewer),
            tag(":"),
            delimited(tag("("),version, tag(")"))
        ))(input)
        .map(|(next, (vendor, _, product, _, variant, _, version))| {
            let specific = SpecificPattern {
                vendor,
                product,
                variant,
                version
            };
            (next, specific)
        })
    }

    fn kind(input: &str) -> Res<&str, KindPattern> {
        pattern(camel)(input).map(|(next, kind)| (next, kind))
    }

    fn resource_type(input: &str) -> Res<&str, ResourceTypePattern> {
        pattern(camel)(input).map(|(next, resource_type)| (next, resource_type))
    }

    fn tks(input: &str) -> Res<&str, TksPattern> {
        delimited(
            tag("<"),
            tuple((
                resource_type,
                opt(delimited(
                    tag("<"),
                    tuple((kind, opt(delimited(tag("<"), pattern(specific), tag(">"))))),
                    tag(">"),
                )),
            )),
            tag(">"),
        )(input)
        .map(|(next, (resource_type, kind_and_specific))| {
            let (kind, specific) = match kind_and_specific {
                None => (Pattern::Any, Pattern::Any),
                Some((kind, specific)) => (
                    kind,
                    match specific {
                        None => Pattern::Any,
                        Some(specific) => specific,
                    },
                ),
            };

            let tks = TksPattern {
                resource_type,
                kind,
                specific,
            };

            (next, tks)
        })
    }

    fn hop(input: &str) -> Res<&str, Hop> {
        tuple((segment, opt(tks)))(input).map(|(next, (segment, tks))| {
            let tks = match tks {
                None => TksPattern::any(),
                Some(tks) => tks,
            };
            (next, Hop { segment, tks })
        })
    }

    #[cfg(test)]
    pub mod test {
        use std::str::FromStr;
        use nom::combinator::all_consuming;
        use semver::VersionReq;
        use crate::pattern::parse::{segment, specific, tks, version};
        use crate::pattern::{AddressSegment, ExactSegment, Pattern, SegmentPattern, SpecificPattern, TksPattern};
        use crate::{CamelCase, DomainCase, Error, SkewerCase};

        #[test]
        pub fn test_segs() -> Result<(), Error> {
            assert!(segment("*")? == ("", SegmentPattern::Any));
            assert!(segment("**")? == ("", SegmentPattern::Recursive));
            assert!(segment("hello")? == ("", SegmentPattern::Exact(ExactSegment::Address("hello".to_string()) )));
            Ok(())
        }

        #[test]
        pub fn test_specific() -> Result<(),Error> {
           let (_,x) = specific( "mysql.org:mysql:innodb:(7.0.1)'")?;
println!("specific: '{}'",x.to_string()) ;
            let (_,x) = specific( "mysql.org:mysql:innodb:(>=7.0.1, <8.0.0)" )?;
println!("specific: '{}'",x.to_string()) ;

            Ok(())
        }

        #[test]
        pub fn test_tks() -> Result<(), Error> {
            let tks_pattern = TksPattern {
                resource_type: Pattern::Exact(CamelCase::new("App")),
                kind: Pattern::Any,
                specific: Pattern::Any
            };

            assert!( tks( "<App>")? == ("",tks_pattern) );

            let tks_pattern = TksPattern {
                resource_type: Pattern::Exact(CamelCase::new("Database")),
                kind: Pattern::Exact(CamelCase::new("Relational")),
                specific: Pattern::Any
            };

            assert!( tks( "<Database<Relational>>")? == ("",tks_pattern) );

            let tks_pattern = TksPattern {
                resource_type: Pattern::Exact(CamelCase::new("Database")),
                kind: Pattern::Exact(CamelCase::new("Relational")),
                specific: Pattern::Exact( SpecificPattern {
                    vendor: Pattern::Exact(DomainCase::new("mysql.org")),
                    product: Pattern::Exact(SkewerCase::new("mysql")),
                    variant: Pattern::Exact(SkewerCase::new("innodb")),
                    version: VersionReq::from_str("^7.0.1")?
                })
            };

            assert!( tks( "<Database<Relational<mysql.org:mysql:innodb:^7.0.1>>>")? == ("",tks_pattern) );

            Ok(())
        }
    }
}

// space.org:app  // exact match of app
// space.org:app:*  // all children of 'app'

// space.org:app<App> // exact address with Type requirement
// space.org:app:db<Database<Relative>> // exact address with Type & Kind requirement .. will match to ANY specific
// space.org:app:db<Database<Relative<mysql.org:mysql:innodb:+7.0.1>>> // with specific version at 7.0.1 or up...
// space.org:app:*<*<*<mysql.org:*:*:*>>> // Any specific with mysql.org as domain

// space.org:app:*<Mechtron> // all children of 'app' that are Mechtrons
// space.org:app:** // recursive children of 'app'
// space.org:app:**<Mechtron> // recursive children of 'app' that are mechtrons
// space.org:app:**<Mechtron>:*<FileSystem>:** // all files under any mechtron filesystems

// match everything under tenant of each user
// space.org:users:*:tenant:**
//
// match everything under tenant of each user
// space.org:**<User>:tenant:**
//

// support for registry:
// space.org:app:*+blah  // all children of 'app' with a 'blah' label
// space.org:app:*+key=value // all children of 'app' with a 'key' label set to 'value'
// match everything under tenant of each user that does NOT have an admin label
// space.org:**<User>!admin:tenant:**
// space.org:[app]:**<User>:tenant:**

// Call pattern
// space.org:app:**<User>:tenant:**^Msg!*
// space.org:app:**<User>:tenant:**^Http
// space.org:app:**<User>:tenant:**^Rc

/////////////////////
// allow switch agent to pattern... and grant permissions 'crwx'
// -> { -| $admins:** +c*wx |-> $app:**<Mechtron>*; }
// allow agent pattern and permissions for sending anything to the admin/** port call
// -> { -| $admins:** +c*wx |-> $app:**<Mechtron>^Msg!admin/**; }
