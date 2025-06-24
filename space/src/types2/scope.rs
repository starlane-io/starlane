use crate::err::ParseErrs0;
use crate::loc::VersionSegLoc;
use crate::parse::util::{new_span, result, Span};
use crate::parse::{Res, SkewerCase};
use core::str::FromStr;
use futures::TryFutureExt;
use nom::branch::alt;
use nom::combinator::{all_consuming, into};
use serde_derive::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use strum_macros::{EnumDiscriminants, EnumString};
use validator::ValidateRequired;

use crate::types::scope::parse::scope;
use crate::types::specific::SpecificLoc;
use once_cell::sync::Lazy;
use crate::types::archetype::Archetype;

pub static ROOT_SCOPE: Lazy<Scope> = Lazy::new(|| Scope(Some(ScopeKeyword::Root), vec![]));

/// Some Domain Prefixes are reserved builtins like `root` & `starlane`
#[non_exhaustive]
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, strum_macros::Display, EnumString, Serialize, Deserialize,
)]
#[strum(serialize_all = "lowercase")]
#[strum(ascii_case_insensitive)]
pub enum ScopeKeyword {
    /// Represents a `Type` that `Starlane` uses internally
    #[strum(ascii_case_insensitive)]
    Starlane,
    /// a special prefix indicating that scoped item is the [ScopeKeyword::Root] definition.
    /// The registry must have one and only one `root` definition for every `Type` and
    /// all of starlane's builtin root Type definitions can only be defined by
    /// starlane.  There will be a way of renaming or re-scoping `Type` definitions
    /// with the registry in order to version proof the case of future unforeseen
    /// collisions
    #[strum(ascii_case_insensitive)]
    Root,
}

/// a segment providing `scope` [Specific] [Meta] in the case where
/// multiple definitions of the same base type and/or to group like definitions
/// together.
///
/// Example for a [super::class::Class::File]
///
#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    EnumDiscriminants,
    strum_macros::Display,
    Serialize,
    Deserialize,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SegmentKind))]
#[strum_discriminants(derive(Hash, strum_macros::EnumString))]
#[non_exhaustive]
#[strum(serialize_all = "lowercase")]
pub enum Segment {
    #[strum(to_string = "{0}")]
    Version(VersionSegLoc),
    #[strum(to_string = "{0}")]
    Segment(SkewerCase),
}

impl From<VersionSegLoc> for Segment {
    fn from(version: VersionSegLoc) -> Self {
        Self::Version(version)
    }
}

impl From<SkewerCase> for Segment {
    fn from(skewer: SkewerCase) -> Self {
        Self::Segment(skewer)
    }
}

impl Archetype for Segment {
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        alt((into(SkewerCase::parser), into(VersionSegLoc::parser)))(input)
    }
}

impl FromStr for Segment {
    type Err = ParseErrs0;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = new_span(s);
        result(all_consuming(parse::postfix_segment)(input))
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
pub struct Scope(Option<ScopeKeyword>, Vec<Segment>);

impl Scope {
    pub fn from_segments(mut segments: Vec<Segment>) -> Self {
        let pre = if let Some(Segment::Segment(skewer)) = segments.first() {
            if let Ok(key) = ScopeKeyword::from_str(skewer) {
                segments.remove(0);
                Some(key)
            } else {
                None
            }
        } else {
            None
        };

        Scope(pre, segments)
    }
}

impl Archetype for Scope {
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        scope(input)
    }
}

impl Scope {
    /// used for testing
    pub(crate) fn new(prefix: Option<ScopeKeyword>, segments: Vec<Segment>) -> Self {
        Self(prefix, segments)
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

impl Default for Scope {
    fn default() -> Self {
        Self(None, vec![])
    }
}

impl FromStr for Scope {
    type Err = ParseErrs0;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(parse::scope)(new_span(s)))
    }
}

impl Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut segs = self
            .post_segments()
            .iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>();

        match self.prefix() {
            None => {}
            Some(prefix) => {
                /// so we can use [Vec::push] ...
                segs.reverse();
                segs.push(prefix.to_string());
                /// because [ScopeKeyword] should be first...
                segs.reverse();
            }
        }
        write!(f, "{}", segs.join("::").to_string())
    }
}

impl Scope {
    /// indicating that a prefix is reserved such as [starlane], [root], [base]
    pub fn reserved(&self) -> bool {
        self.prefix().is_some()
    }

    pub fn prefix(&self) -> &Option<ScopeKeyword> {
        &self.0
    }

    pub fn post_segments(&self) -> &Vec<Segment> {
        &self.1
    }
}

#[cfg(test)]
pub mod test {
    use crate::types::scope::parse::parse;
    use crate::types::scope::ScopeKeyword;
    use std::str::FromStr;
    use crate::parse::util::{new_span, result};
    use crate::types2::scope::parse::scope;
    
    
    #[test]
    fn text_x() {

        assert_eq!(ScopeKeyword::from_str("root").unwrap(), ScopeKeyword::Root);
        let domain = result(scope(new_span("hello"))).unwrap();
        assert_eq!(domain.to_string().as_str(), "hello");
        assert_eq!(domain.reserved(), false);
        assert_eq!(domain.prefix().is_none(), true);
    }

/*

        let scope = parse("root").unwrap();
        println!("{:?}", scope);
        assert!(scope.prefix().is_some());
        println!();

        assert_eq!(scope.0.clone().unwrap(), ScopeKeyword::Root);
        assert_eq!(scope.to_string().as_str(), "root");
        assert_eq!(scope.reserved(), true);
        assert_eq!(scope.prefix().is_some(), true);

        let scope = parse("starlane::child").unwrap();
        assert_eq!(scope.to_string().as_str(), "starlane::child");
        assert_eq!(scope.reserved(), true);
        assert_eq!(scope.prefix().is_some(), true);

        let scope = parse("root::some::1.3.7").unwrap();
        assert_eq!(scope.to_string().as_str(), "root::some::1.3.7");
        assert_eq!(scope.reserved(), true);
        assert_eq!(scope.prefix().is_some(), true);

        // let domain = DomainScope(Some(Prefix::Starlane),vec!["one","two","truee"].into());
        // println!("domain: '{}'", domain.to_string());
        //assert!(false)
    }
        */
}

pub mod parse {
    use crate::err;
    use crate::parse::util::{new_span, result, Span};
    use crate::parse::{context, skewer_case, version, NomErr, Res, SkewerCase};
    use crate::types::scope::{Scope, ScopeKeyword, Segment};
    use nom::branch::alt;
    use nom::combinator::{opt, peek};
    use nom::multi::separated_list0;
    use nom::sequence::{terminated, tuple};
    use nom::Parser;
    use nom_supreme::tag::complete::tag;
    use nom_supreme::tag::TagError;
    use nom_supreme::ParserExt;
    use std::str::FromStr;

    pub(crate) fn parse(s: impl AsRef<str>) -> Result<Scope, err::ParseErrs0> {
        let span = new_span(s.as_ref());
        result(scope(span))
    }
    /// will return an empty [Scope]  -> `DomainScope(None,Vec:default())` if nothing is found
    pub fn scope<I: Span>(input: I) -> Res<I, Scope> {
        context(
            "scope parsing",
            terminated(separated_list0(tag("::"), postfix_segment), tag("::"))
        )(input)
            .map(|(next, segments)| (next, Scope::from_segments(segments))) 
    }

    fn prefix<I: Span>(input: I) -> Res<I, ScopeKeyword> {
        let (next, skewer) = skewer_case(input.clone())?;
        match ScopeKeyword::from_str(skewer.as_str()) {
            Ok(prefix) => Ok((next, prefix)),
            Err(_) => Err(nom::Err::Error(NomErr::from_tag(input, "prefix"))),
        }
    }

    pub(super) fn postfix_segment<I: Span>(input: I) -> Res<I, Segment> {
        fn semver<I: Span>(input: I) -> Res<I, Segment> {
            version(input).map(|(input, version)| (input, Segment::Version(version)))
        }

        fn segment<I: Span>(input: I) -> Res<I, Segment> {
            skewer_case(input).map(|(input, skewer)| (input, Segment::Segment(skewer)))
        }
        alt((segment, semver))(input)
    }

    #[test]
    fn test_scope() {
        assert_eq!(
            scope(new_span("root::")).unwrap().1,
            Scope(Some(ScopeKeyword::Root), vec![])
        );
        assert_eq!(
            scope(new_span("my::")).unwrap().1,
            Scope(
                None,
                vec![Segment::Segment(SkewerCase::from_str("my").unwrap())]
            )
        );
        assert_eq!(
            scope(new_span("my::Root")).unwrap().1,
            Scope(
                None,
                vec![Segment::Segment(SkewerCase::from_str("my").unwrap())]
            )
        );
        assert_eq!(
            scope(new_span("my::more::Root")).unwrap().1,
            Scope(
                None,
                vec![
                    Segment::Segment(SkewerCase::from_str("my").unwrap()),
                    Segment::Segment(SkewerCase::from_str("more").unwrap())
                ]
            )
        );
        assert_eq!(
            scope(new_span("root::more::Root")).unwrap().1,
            Scope(
                Some(ScopeKeyword::Root),
                vec![Segment::Segment(SkewerCase::from_str("more").unwrap())]
            )
        );
    }
}

/*
#[derive(Clone)]
pub(crate) struct Scoped<G> {
    scope: Scope,
    reference: G
}

impl <G> Scoped<G> where G: Generic
{
    pub fn plus_specific(self, specific: Specific) -> GenericExact<G> {
        G::plus(self.reference, self.scope, specific )
    }
}

impl <G> Scoped<G> {
    pub fn new(scope: Scope, item: G) -> Self {
        Self{
            scope,
            reference: item,
        }
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }
}

impl <G> Deref for Scoped<G> {
    type Target = G;

    fn deref(&self) -> &Self::Target {
        &self.reference
    }
}

 */
