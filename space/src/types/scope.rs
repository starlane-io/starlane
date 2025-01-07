use core::str::FromStr;
use std::fmt::Display;
use std::ops::Deref;
use futures::TryFutureExt;
use nom::branch::alt;
use nom::combinator::{all_consuming, into};
use nom::error::{ErrorKind, FromExternalError};
use serde_derive::{Deserialize, Serialize};
use strum_macros::{EnumDiscriminants, EnumString};
use validator::ValidateRequired;
use crate::err::ParseErrs;
use crate::loc::Version;
use crate::parse::{skewer, var_case, version, NomErr, Res, SkewerCase, VarCase};
use crate::parse::util::{new_span, result, Span};

use once_cell::sync::Lazy;
use strum::ParseError;
use starlane_space::types::parse::TypeParser;
use starlane_space::types::private::Generic;
use crate::types::specific::Specific;

pub static ROOT_SCOPE: Lazy<Scope> = Lazy::new(|| Scope(Some(Keyword::Root), vec![]));


/// Some Domain Prefixes are reserved builtins like `root` & `starlane`
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq, Hash, strum_macros::Display,EnumString,Serialize,Deserialize)]
#[strum(serialize_all = "lowercase")]
pub enum Keyword {
    /// Represents a `Type` that `Starlane` uses internally
    Starlane,
    /// a special prefix indicating that scoped item is the [Keyword::Root] definition.
    /// The registry must have one and only one `root` definition for every `Type` and
    /// all of starlane's builtin root Type definitions can only be defined by
    /// starlane.  There will be a way of renaming or re-scoping `Type` definitions
    /// with the registry in order to version proof the case of future unforeseen
    /// collisions
    Root
}



impl TypeParser for Keyword {
    fn inner<I>(input:I) -> Res<I,Self> where I: Span{
        let (next,var) = var_case(input.clone())?;
        match Self::from_str(var.as_str()) {
            Ok(keyword) => Ok((next,keyword)),
            Err(err) => Err(nom::Err::Error(NomErr::from_external_error(input,ErrorKind::Fail,err)))
        }
    }
}

/// a segment providing `scope` [Specific] [Meta] in the case where
/// multiple definitions of the same base type and/or to group like definitions
/// together.
///
/// Example for a [super::class::Class::File]
///
#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display,Serialize,Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SegmentDiscriminant))]
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
    Segment(VarCase),
}


impl From<Version> for Segment {
    fn from(version: Version) -> Self {
        Self::Version(version)
    }
}

impl From<VarCase> for Segment {
    fn from(case: VarCase) -> Self {
        Self::Segment(case)
    }
}

impl TypeParser for Segment  {
    fn inner<I>(input:I) -> Res<I,Self> where I: Span{
        alt((into(version),into(var_case)))(input)
    }
}

impl FromStr for Segment {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = new_span(s);
        result(all_consuming(parse::segment)(input))
    }
}

#[derive(Clone,Eq,PartialEq,Hash,Debug,Serialize,Deserialize)]
pub struct Scope(Option<Keyword>, Vec<Segment>);

impl Scope {
    pub fn new(prefix: Option<Keyword>, segments: Vec<Segment> ) -> Self  {
        Self(prefix,segments)
    }

    pub fn root() -> Self {
        ROOT_SCOPE.clone()
    }

    /*
    pub fn with<G>(self, generic: G) -> Scoped<G> {
        Scoped::new(self, generic)
    }

     */

}

impl TypeParser for Scope {
    fn inner<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        parse::scope(input)
    }
}



impl Default for Scope {
    fn default() -> Self {
        Self(None, vec![])
    }
}

impl FromStr for Scope {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(parse::scope)(new_span(s)))
    }
}


impl Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut segs = self.post_segments().iter().map(|segment| segment.to_string()).collect::<Vec<_>>();

        match self.prefix() {
            None => {}
            Some(prefix) => {
                /// so we can use [Vec::push] ...
                segs.reverse();
                segs.push(prefix.to_string());
                /// because [Keyword] should be first...
                segs.reverse();
            }
        }
        write!(f, "{}", segs.join("::").to_string())
    }
}



impl Scope {

    /// indicating that a prefix is reserved such as [starlane], [root], [base]
    pub fn is_reserved_prefix(&self) -> bool {
        self.prefix().is_some()
    }

    pub fn prefix(&self) -> &Option<Keyword>  {
        &self.0
    }

    pub fn post_segments(&self) -> &Vec<Segment> {
        &self.1
    }

}







pub mod parse {
    use std::str::FromStr;
    use ascii::AsciiChar::{a, i};
    use futures::{FutureExt, TryFutureExt};
    use nom::combinator::{opt, peek};
    use nom::multi::separated_list0;
    use nom::sequence::{pair, terminated, tuple};
    use nom_supreme::tag::complete::tag;
    use crate::err;
    use crate::parse::{skewer_case, var_case, version, NomErr, Res};
    use crate::parse::util::{new_span, preceded, result, Span};
    use crate::types::scope::{Keyword, Scope, Segment};
    use nom::{IResult, Parser};
    use nom::branch::alt;
    use nom_supreme::ParserExt;
    use nom_supreme::tag::TagError;
    use crate::types::parse::TypeParser;

    pub(crate) fn parse(s: impl AsRef<str> ) -> Result<Scope,err::ParseErrs> {
        let span = new_span(s.as_ref());
        result(scope(span))
    }
    /// will return an empty [Scope]  -> `DomainScope(None,Vec:default())` if nothing is found
    pub fn scope<I: Span>(input: I) -> Res<I, Scope> {
        let keyword = <Keyword as TypeParser> ::inner;
        pair(opt(pair(keyword,opt(tag("::")))),segments)(input).map(|(next,(preamble,segments))|{
            let keyword = preamble.map_or_else(||None,|(keyword,_) | Some(keyword));

            (next,Scope::new(keyword,segments))
        })
    }



    pub fn segments<I: Span>(input: I) -> Res<I, Vec<Segment>> {
        separated_list0(tag("::"), segment)(input)
    }


    pub(super) fn segment<I: Span>(input: I) -> Res<I, Segment> {
        fn semver<I: Span>(input: I) -> Res<I, Segment> {
            version(input).map(|(input,version)|(input, Segment::Version(version)))
        }

        fn segment<I: Span>(input: I) -> Res<I, Segment> {
            var_case(input).map(|(input,var)|(input, Segment::Segment(var)))
        }

        alt((segment,semver))(input)
    }



}

#[cfg(test)]
pub mod test {
    use std::str::FromStr;
    use itertools::Itertools;
    use starlane_space::types::scope::Keyword;
    use crate::loc::Version;
    use crate::parse::util::{new_span, result};
    use crate::parse::VarCase;
    use crate::types::parse::TypeParser;
    use crate::types::scope::parse::parse;
    use crate::types::scope::{Scope, Segment, SegmentDiscriminant};

    #[test]
    fn test_keywords() {
        let keyword = result(Keyword::inner(new_span("root"))).unwrap();
        assert_eq!(keyword, Keyword::Root);
        let scope = Scope(Some(Keyword::Root), vec![]);
        assert!(scope.is_reserved_prefix())
    }

    #[test]
    fn test_segment() {
        let var = "some_var_case";
        let version = "1.3.5";
        let segment = result(Segment::inner( new_span(var))).unwrap();
        assert_eq!(Segment::Segment(VarCase::from_str(var).unwrap()), segment);

        let segment = result(Segment::inner( new_span(version))).unwrap();
        assert_eq!(Segment::Version(Version::from_str(version).unwrap()), segment);
    }

    #[test]
    fn test_post_segments( )  {

            let segments = vec!["silly","database","postgres"];
            let input = segments.iter().join("::");
            let mut segments  = segments.into_iter().map(|segment|VarCase::from_str(segment).unwrap()).map(|var|Segment::Segment(var)).collect::<Vec<Segment>>();

            assert_eq!( "silly::database::postgres", input.as_str() );


            let parsed = result(super::parse::segments(new_span(input.as_str()))).unwrap();

            assert_eq!(parsed.len(),3);
        assert_eq!(parsed,segments)

    }
    #[test]
    fn test_scope( )  {
        {
            let input = "starlane";
            let scope = result(Scope::inner(new_span(input))).unwrap();
            assert!(scope.is_reserved_prefix());
            assert_eq!(scope, Scope(Some(Keyword::Starlane), vec![]));
        }

        {
            let input = "my";
            let var = VarCase::from_str(input).unwrap();;
            let scope = result(Scope::inner(new_span(input))).unwrap();
            assert!(!scope.is_reserved_prefix());
            assert_eq!(scope, Scope(None, vec![Segment::Segment(var)]));
        }

        {
            let segments = vec!["root","database","postgres"];
            let input = segments.iter().join("::");
            let mut segments  = segments.into_iter().map(|segment|VarCase::from_str(segment).unwrap()).map(|var|Segment::Segment(var)).collect::<Vec<Segment>>();

            assert_eq!( "root::database::postgres", input.as_str() );

            let scope = result(Scope::inner(new_span(input.as_str()))).unwrap();
            assert!(scope.is_reserved_prefix());
            segments.remove(0);
            assert_eq!(segments.len(),2);
            println!("segments: {}", segments.iter().join("::"));

            assert_eq!(scope, Scope(Some(Keyword::Root), segments));
        }


    }
}