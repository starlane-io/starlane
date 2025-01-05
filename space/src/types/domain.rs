use core::str::FromStr;
use std::ops::ControlFlow;
use futures::stream::{TryFilter, TryFold};
use futures::TryFutureExt;
use nom::combinator::all_consuming;
use serde_derive::{Deserialize, Serialize};
use strum::ParseError;
use strum_macros::{EnumDiscriminants, EnumString};
use validator::ValidateRequired;
use starlane_space::parse::VarCase;
use crate::err::{ParseErrs, SpaceErr};
use crate::loc::Version;
use crate::parse::{space_point_segment, CamelCase, SkewerCase};
use crate::parse::util::{new_span, result};

/// Some Domain Prefixes are reserved builtins like `root` & `starlane`
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Hash, strum_macros::Display,EnumString,Serialize,Deserialize)]
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
/// Example for a [super::class::ClassKind::File]
///
#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display,Serialize,Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SegmentKind))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
))]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum Segment {
    #[strum(to_string = "{0}")]
    Version(Version),
    #[strum(to_string = "{0}")]
    Segment(SkewerCase),
}


impl FromStr for Segment {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = new_span(s);
        result(all_consuming(parse::postfix_segment)(input))
    }
}

#[derive(Clone,Eq,PartialEq,Hash,Debug,Serialize,Deserialize)]
pub struct DomainScope(Option<Prefix>,Vec<Segment>);

impl DomainScope {
    pub fn new( prefix: Option<Prefix>, segments: Vec<Segment> ) -> Self  {
        Self(prefix,segments)
    }
}

impl FromStr for DomainScope {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(parse::domain)(new_span(s)))
    }
}


impl ToString for DomainScope {
    fn to_string(&self) -> String {

        let mut segs = self.post_segments().iter().map(|segment| segment.to_string().as_str()).collect::<Vec<_>>();

        match self.prefix() {
            None => {}
            Some(prefix) => {
                /// so we can use [Vec::push] ...
                segs.reverse();
                segs.push(prefix.to_string().as_str());
                /// because [Prefix] should be first...
                segs.reverse();
            }
        }
        segs.join("::").to_string()

    }
}

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





#[cfg(test)]
pub mod test {
    use crate::types::domain::{DomainScope, Prefix};
    use crate::types::domain::parse::parse;

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


       // let domain = DomainScope(Some(Prefix::Starlane),vec!["one","two","truee"].into());
       // println!("domain: '{}'", domain.to_string());
        //assert!(false)
    }
}

pub mod parse {
    use std::str::FromStr;
    use cliclack::input;
    use futures::TryStreamExt;
    use nom::combinator::{opt, peek};
    use nom::multi::separated_list0;
    use nom::sequence::{terminated, tuple};
    use nom_supreme::tag::complete::tag;
    use url::ParseError;
    use starlane_space::parse::SpaceContextError;
    use crate::command::common::PropertyMod;
    use crate::err;
    use crate::parse::{skewer, skewer_case, skewer_chars, version, NomErr, Res, SkewerCase};
    use crate::parse::util::{new_span, result, Span};
    use crate::types::domain::{DomainScope, Prefix, Segment};
    use nom::{Err, Parser};
    use nom::branch::alt;
    use nom_supreme::ParserExt;
    use nom_supreme::tag::TagError;

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
        let (next, skewer) = terminated(skewer_case,tag("::"))(input.clone())?;
        match Prefix::from_str(skewer.as_str()) {
            Ok(prefix) => Ok((next,prefix)),
            Err(_) => Err(nom::Err::Error(NomErr::from_tag(input,"prefix")))
        }
    }

    pub(super) fn postfix_segment<I: Span>(input: I) -> Res<I, Segment> {
        fn semver<I: Span>(input: I) -> Res<I, Segment> {
            version(input).map(|(input,version)|(input, Segment::Version(version)))
        }

        fn segment<I: Span>(input: I) -> Res<I, Segment> {
            skewer_case(input).map(|(input,skewer)|(input, Segment::Segment(skewer)))
        }
        alt((segment,semver))(input)
    }



}

