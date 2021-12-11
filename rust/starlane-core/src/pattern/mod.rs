use semver::VersionReq;
use std::ops::Deref;
use std::str::FromStr;
use crate::pattern::specific::{VendorPattern, ProductPattern, VariantPattern};
use crate::mesh::serde::id::{Specific, ResourceType};
use crate::resource::Kind;
use crate::parse::CamelCase;

#[derive(Eq, PartialEq)]
pub struct AddressTksPattern {
    pub hops: Vec<Hop>,
}

impl AddressTksPattern {
    pub fn consume(&self) -> Option<AddressTksPattern> {
        if self.hops.len() <= 1 {
            Option::None
        } else {
            let mut hops = self.hops.clone();
            hops.remove(0);
            Option::Some(AddressTksPattern { hops });
        }
    }

    pub fn is_final(&self) -> bool {
        self.hops.len() == 1
    }

    pub fn matches(&self, address_tks_path: &AddressTksPath) -> bool {
        if address_tks_path.segments.len() < self.hops.len() {
            return false;
        }

        if address_tks_path.segments.is_empty() || self.hops.is_empty() {
            return false;
        }

        let hop = self.hops.first().expect("hop");
        let seg = address_tks_path.segments.first().expect("segment");

        if address_tks_path.is_final() && self.is_final() {
            // this is the final hop & segment if they match, everything matches!
            hop.matches(seg)
        } else if address_tks_path.is_final() {
            // we still have hops that haven't been matched and we are all out of path
            false
        }
        // special logic is applied to recursives **
        else if hop.segment.is_recursive() && self.hops.len() >= 2 {
            // a Recursive is similar to an Any in that it will match anything, however,
            // let's see if the NEXT hop will match the segment
            let next_hop = self.hops.get(1).expect("next<Hop>");
            if next_hop.matches(seg) {
                // since the next hop after the recursive matches, we consume the recursive and continue hopping
                // this allows us to make matches like:
                // space.org:**:users ~ space.org:many:silly:dirs:users
                self.consume()
                    .expect("AddressTksPattern")
                    .matches(&address_tks_path.consume().expect("AddressTksPath"))
            } else {
                // the NEXT hop does not match, therefore we do NOT consume() the current hop
                self.matches(&address_tks_path.consume().expect("AddressTksPath"))
            }
        } else if hop.matches(seg) {
            // in a normal match situation, we consume the hop and move to the next one
            self.consume()
                .expect("AddressTksPattern")
                .matches(&address_tks_path.consume().expect("AddressTksPath"))
        } else {
            false
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum SegmentPattern {
    Any,       // *
    Recursive, // **
    Exact(ExactSegment),
}

impl SegmentPattern {
    pub fn matches(&self, segment: &String) -> bool {
        match self {
            SegmentPattern::Any => true,
            SegmentPattern::Recursive => true,
            SegmentPattern::Exact(exact) => match exact {
                ExactSegment::Address(pattern) => *pattern == *segment,
            },
        }
        false
    }

    pub fn is_recursive(&self) -> bool {
        match self {
            SegmentPattern::Any => false,
            SegmentPattern::Recursive => true,
            SegmentPattern::Exact(_) => false,
        }
    }
}

pub type KeySegment = String;
pub type AddressSegment = String;

#[derive(Clone, Eq, PartialEq)]
pub enum ExactSegment {
    Address(AddressSegment),
}

impl ExactSegment {
    pub fn matches(&self, segment: &AddressSegment) -> bool {
        match self {
            ExactSegment::Address(s) => *s == *segment,
        }
        false
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Hop {
    pub segment: SegmentPattern,
    pub tks: TksPattern,
}

impl Hop {
    pub fn matches(&self, address_tks_segment: &AddressTksSegment) -> bool {
        self.segment.matches(&address_tks_segment.address_segment)
    }
}

#[derive(Eq, PartialEq)]
pub enum Pattern<P> {
    Any,
    Exact(P),
}

impl<P> Pattern<P> {
    pub fn matches(&self, t: &P) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(p) => *p == t,
        }
    }
    pub fn matches_opt(&self, other: Option<&P>) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(exact) => {
                if let Option::Some(other) = other {
                    *exact == *other
                } else {
                    false
                }
            }
        }
    }
}

impl Into<Pattern<String>> for Pattern<&str> {
    fn into(self) -> Pattern<String> {
        match self {
            Pattern::Any => Pattern::Any,
            Pattern::Exact(f) => Pattern::Exact(f.to_string()),
        }
    }
}

impl<P> ToString for Pattern<P>
where
    P: ToString,
{
    fn to_string(&self) -> String {
        match self {
            Pattern::Any => "*".to_string(),
            Pattern::Exact(exact) => exact.to_string(),
        }
    }
}

pub type ResourceTypePattern = Pattern<CamelCase>;
pub type KindPattern = Pattern<CamelCase>;

pub mod specific {
    use semver::VersionReq;
    use std::ops::Deref;
    use std::str::FromStr;
    use crate::pattern::Pattern;
    use crate::error::Error;
    use crate::parse::SkewerCase;

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

impl ToString for SpecificPattern {
    fn to_string(&self) -> String {
        format!(
            "{}:{}:{}:({})",
            self.vendor.to_string(),
            self.product.to_string(),
            self.variant.to_string(),
            self.version.to_string()
        )
    }
}

#[derive(Eq, PartialEq)]
pub struct TksPattern {
    pub resource_type: ResourceTypePattern,
    pub kind: KindPattern,
    pub specific: Pattern<SpecificPattern>,
}

impl TksPattern {
    pub fn new(
        resource_type: ResourceTypePattern,
        kind: KindPattern,
        specific: Pattern<SpecificPattern>,
    ) -> Self {
        Self {
            resource_type,
            kind,
            specific,
        }
    }

    pub fn matches(&self, tks: &Tks) -> bool {
        self.resource_type.matches(&tks.resource_type)
            && self.kind.matches_opt(tks.kind.as_ref())
            && self.specific.matches_opt(tks.kind.specific())
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
pub struct AddressTksPath {
    pub segments: Vec<AddressTksSegment>,
}

impl AddressTksPath {
    pub fn consume(&self) -> Option<AddressTksPath> {
        if self.segments.len() <= 1 {
            Option::None
        }
        let mut segments = self.segments.clone();
        segments.remove(0);
        Option::Some(AddressTksPath { segments })
    }

    pub fn is_final(&self) -> bool {
        self.segments.len() == 1
    }
}

#[derive(Eq, PartialEq)]
pub struct AddressTksSegment {
    pub address_segment: AddressSegment,
    pub tks: Tks,
}

#[derive(Eq, PartialEq)]
pub struct Tks {
    pub resource_type: ResourceType,
    pub kind: Option<Kind>,
}

impl Tks {
    pub fn specific(&self) -> Option<Specific> {
        match &self.kind {
            Some(kind) => kind.specific(),
            None => None,
        }
    }
}

pub mod parse {

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
    use crate::pattern::{SegmentPattern, ExactSegment, Pattern, SpecificPattern, KindPattern, ResourceTypePattern, TksPattern, Hop};
    use mesh_portal_parse::parse::{Res, skewer};

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
            delimited(tag("("), version, tag(")")),
        ))(input)
        .map(|(next, (vendor, _, product, _, variant, _, version))| {
            let specific = SpecificPattern {
                vendor,
                product,
                variant,
                version,
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

        use nom::combinator::all_consuming;
        use semver::VersionReq;
        use std::str::FromStr;
        use crate::error::Error;
        use crate::pattern::{SegmentPattern, ExactSegment, TksPattern, Pattern, SpecificPattern};
        use crate::pattern::parse::{segment, specific, tks, hop};

        #[test]
        pub fn test_segs() -> Result<(), Error> {
            assert!(segment("*")? == ("", SegmentPattern::Any));
            assert!(segment("**")? == ("", SegmentPattern::Recursive));
            assert!(
                segment("hello")?
                    == (
                        "",
                        SegmentPattern::Exact(ExactSegment::Address("hello".to_string()))
                    )
            );
            Ok(())
        }

        #[test]
        pub fn test_specific() -> Result<(), Error> {
            let (_, x) = specific("mysql.org:mysql:innodb:(7.0.1)'")?;
            println!("specific: '{}'", x.to_string());
            let (_, x) = specific("mysql.org:mysql:innodb:(>=7.0.1, <8.0.0)")?;
            println!("specific: '{}'", x.to_string());
            let (_, x) = specific("mysql.org:*:innodb:(>=7.0.1, <8.0.0)")?;
            println!("specific: '{}'", x.to_string());

            Ok(())
        }

        #[test]
        pub fn test_tks() -> Result<(), Error> {
            let tks_pattern = TksPattern {
                resource_type: Pattern::Exact(CamelCase::new("App")),
                kind: Pattern::Any,
                specific: Pattern::Any,
            };

            assert!(tks("<App>")? == ("", tks_pattern));

            let tks_pattern = TksPattern {
                resource_type: Pattern::Exact(CamelCase::new("Database")),
                kind: Pattern::Exact(CamelCase::new("Relational")),
                specific: Pattern::Any,
            };

            assert!(tks("<Database<Relational>>")? == ("", tks_pattern));

            let tks_pattern = TksPattern {
                resource_type: Pattern::Exact(CamelCase::new("Database")),
                kind: Pattern::Exact(CamelCase::new("Relational")),
                specific: Pattern::Exact(SpecificPattern {
                    vendor: Pattern::Exact(DomainCase::new("mysql.org")),
                    product: Pattern::Exact(SkewerCase::new("mysql")),
                    variant: Pattern::Exact(SkewerCase::new("innodb")),
                    version: VersionReq::from_str("^7.0.1")?,
                }),
            };

            assert!(
                tks("<Database<Relational<mysql.org:mysql:innodb:(^7.0.1)>>>")?
                    == ("", tks_pattern)
            );

            Ok(())
        }

        #[test]
        pub fn test_hop() -> Result<(), Error> {
            hop("*<Database<Relational<mysql.org:mysql:innodb:(^7.0.1)>>>")?;
            hop("**<Database<Relational<mysql.org:mysql:innodb:(^7.0.1)>>>")?;
            hop("space.org:<Database<Relational<mysql.org:mysql:innodb:(^7.0.1)>>>")?;
            hop("space.org:something<Database<Relational<mysql.org:mysql:innodb:(^7.0.1)>>>")?;
            hop("space.org:no-type")?;
            hop("space.org:no-type:**")?;
            hop("space.org:app:users:*:tenant:**")?;
            hop("space.org:app:users:*:tenant:**<Mechtron>")?;
            hop("space.org:something:**<*<*<mysql.org:mysql:innodb:(^7.0.1)>>>")?;
            hop("space.org:something<*>")?;

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
// -> { -| $admins:** +CrWX |-> $app:**<Mechtron>^Msg!admin/**; }

// -> { +( sa .:users:*||.:old-users:*; )+( grant .:my-files:** +CRWX; )-> $app:**<Mechtron>^Msg/admin/**; }
// -> { +( sa .:(users|old-users):*; )+( grant .:my-files:** +CRWX; )-> $app:**<Mechtron>^Msg/admin/**; }
// -> { +( sa .:(users|old-users):*; )+( grant .:my-files:** +CRUDLX; )-> $app:**<Mechtron>^Http/admins/*; }

// Http<Post>:/some/path/(.*) +( set req.path="/new/path/$1" )-[ Map{ body<Bin~json> } ]+( session )-[ Map{ headers<Meta>, body<Bin~json>, session<Text> } ]-> {*} => &;

// Msg<Action>:/work/it -> { +( sa .:users:*||.:old-users:*; )+( grant .:my-files:** +CRWX; )-> $app:**<Mechtron>^Msg/admin/**; } =[ Text ]=> &;

// <App> 'taint'
// block -| $app:..:** +crwx |-| !$app:..:** +---- |
