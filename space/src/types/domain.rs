use crate::loc::Version;
use crate::parse::{CamelCase, SkewerCase};
use core::str::FromStr;
use strum_macros::{EnumDiscriminants, EnumString};


/// Some Domain Prefixes are reserved builtins like `root` & `starlane`
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Hash, strum_macros::Display,EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Prefix {
    /// Represents a `Type` that `Starlane` uses internally
    Starlane,
    /// a special prefix indicating that scoped item is the [Prefix::Root] definition.
    /// The registry must have one and only one `root` definition for every `Type` and
    /// all of starlane's builtin root Type definitions can only be defined by
    /// starlane.  There will be a way of renaming or re-scoping `Type` definitions
    /// with the registry in order to version proof the case of future unforeseen
    /// collisions
    Root
}

/// a segment providing `scope` [Specific] [Meta] in the case where
/// multiple definitions of the same base type and/or to group like definitions
/// together.
///
///
/// Example for a [super::class::Class::File]
///
///
#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SegmentKind))]
#[strum_discriminants(derive(
    Clone,
    Debug,
    Hash,
    strum_macros::EnumString,
))]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum Segment {
    #[strum(to_string = "{0}")]
    Ver(Version),
    #[strum(to_string = "{0}")]
    Seg(SkewerCase),
}



#[derive(Clone,Eq,PartialEq,Hash,Debug)]
pub struct DomainScope(Option<Prefix>,Vec<Segment>);


impl Default for DomainScope {
    fn default() -> Self {
        Self(None,Vec::new())
    }
}

impl DomainScope {

    /// indicating that a prefix is reserved such as [starlane], [root], [base]
    pub fn reserved(&self) -> bool {
        self.prefix().is_some()
    }

    pub fn prefix(&self) -> &Option<Prefix>  {
        &self.0
    }

    pub fn post_segments(&self) -> &Vec<Segment> {
        &self.1
    }

}



impl FromStr for Segment {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        fn ext(s: &str) -> Result<Segment, eyre::Error> {
            Ok(Segment::Seg(SkewerCase::from_str(s)?.into()))
        }

        match SegmentKind::from_str(s) {
            /// this Ok match is actually an Error!
            Ok(SegmentKind::Seg) => ext(s),
            Ok(kind) => ext(kind.into()),
            /// a non match in the builtins means an [Segment::_Ext]
            Err(_) => ext(s),
        }
    }
}
#[cfg(test)]
pub mod test {
    use crate::types::domain::parse::parse;
    use crate::types::domain::{DomainScope, Prefix};

    #[test]
    fn text_x( )  {
        assert!(false);
        let domain = parse("hello").unwrap();
        assert_eq!(domain.to_string().as_str(), "hello");
        assert_eq!(domain.reserved(), false);
        assert_eq!(domain.prefix().is_none(), true);

        let domain = parse("root").unwrap();
        assert_eq!(domain.to_string().as_str(), "root");
        assert_eq!(domain.reserved(), true);
        assert_eq!(domain.prefix().is_none(), true);


        let domain = parse("starlane::child").unwrap();
        assert_eq!(domain.to_string().as_str(), "starlane::child");
        assert_eq!(domain.reserved(), true);
        assert_eq!(domain.prefix().is_some(), true);

        let domain = parse("root::some::1.3.7").unwrap();
        assert_eq!(domain.to_string().as_str(), "root::some::1.3.7");
        assert_eq!(domain.reserved(), true);
        assert_eq!(domain.prefix().is_some(), true);

        let domain = DomainScope(Some(Prefix::Starlane),vec!["one","two","truee"].into());
        println!("domain: '{}'", domain.to_string());
        assert!(false)
    }
}

pub mod parse {
    use crate::err;
    use crate::parse::util::{new_span, result, Span};
    use crate::parse::{skewer, skewer_chars, version, Res};
    use crate::types::domain::{DomainScope, Prefix, Segment};
    use nom::branch::alt;
    use nom::combinator::opt;
    use nom::multi::separated_list0;
    use nom::sequence::{terminated, tuple};
    use nom::Parser;
    use nom_supreme::tag::complete::tag;
    use nom_supreme::ParserExt;
    use std::str::FromStr;

    pub(crate) fn parse(s: impl AsRef<str> ) -> Result<DomainScope,err::ParseErrs> {
        let span = new_span(s.as_ref());
        result(domain(span))
    }
    /// will return an empty [DomainScope]  -> `DomainScope(None,Vec:default())` if nothing is found
    pub fn domain<I: Span>(input: I) -> Res<I, DomainScope> {
        tuple((opt(prefix), separated_list0(tag("::"), postfix_segment)))(input).map(|(input,(prefix,segments))|{
            (input, DomainScope(prefix,segments))
        })
    }

    fn prefix <I: Span>(input: I) -> Res<I, Prefix> {
        terminated(skewer_chars.parse_from_str(),tag("::"))(input)
    }

    fn postfix_segment<I: Span>(input: I) -> Res<I, Segment> {
        fn semver<I: Span>(input: I) -> Res<I, Segment> {
            version(input).map(|(input,version)|(input, Segment::Ver(version)))
        }

        fn segment<I: Span>(input: I) -> Res<I, Segment> {
            skewer(input).map(|(input,skewer)|(input, Segment::Seg(skewer)))
        }
        alt((segment,semver))
    }



}

