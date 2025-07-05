use crate::parse::model::NestedBlockKind;
use crate::parse::util::Span;
use crate::parse::{CamelCase, Domain, SkewerCase, SnakeCase};
use crate::parse2::chars::ident;
use crate::parse2::token::block::block;
use crate::parse2::token::punctuation::punctuation;
use crate::parse2::{ErrTree, Input, ParseErrsTree, Res};
use derive_builder::Builder;
use nom::branch::alt;
use nom::error::ParseError;
use nom::multi::many1;
use nom::{Offset, Parser, Slice};
use nom_supreme::ParserExt;
use semver::Version;
use std::collections::HashMap;
use std::ops::Range;
use std::str::FromStr;
use strum_macros::{Display, EnumDiscriminants};


fn token(input: Input) -> Res<Token> {
    use whitespace::whitespace;
    alt((tok(whitespace),tok(punctuation), tok(ident), tok(block)))(input)
}

fn tokenize(input: Input) -> Res<Vec<Token>> {
    many1(token)(input).map(|(next,tokens)|{
        println!("**********************");
        println!("tokens.len(): {}", tokens.len());
        for token in &tokens {
            println!("{}", token.token);
        }
        println!("**********************");
        
        (next,tokens)})
}


#[derive(Clone, Debug)]
pub struct Token {
    span: Range<usize>,
    token: TokenKindDef,
}

#[derive(Clone, Debug, EnumDiscriminants, Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TokenKind))]
#[strum_discriminants(derive(Hash,Display))]
enum TokenKindDef {
    #[strum(to_string="Ident({0})")]
    Ident(Ident),
    #[strum(to_string="BlockOpen({0})")]
    Open(NestedBlockKind),
    #[strum(to_string="BlockClose({0})")]
    Close(NestedBlockKind),
    /// `+` symbol
    Plus,
    /// `@` symbol
    At,
    /// `:` symbol
    SegmentSep,
    /// `::` symbol
    Scope,
    /// `+::` add variant
    Variant,
    /// `.` symbol (used for properties and child defs)
    Dot,
    /// semicolon ';' (just as god intended)
    Terminator,
    /// `&` good old ampersand
    Return,
    /// `version=` i.e.: Def(`version=`1.1.5) ... tells which parser version to use
    VersionPrelude,
    /// a `space` or a `tab`
    Space,
    /// any cluster of whitespace: `space`, `tab` and `newline`
    Newline,
    /// an erroneous token...
    Err(Range<usize>),
}

impl TokenKindDef {
    fn open(block: NestedBlockKind) -> Self {
        Self::Open(block)
    }

    fn close(block: NestedBlockKind) -> Self {
        Self::Close(block)
    }
}
#[derive(Clone, Debug, EnumDiscriminants, Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(IdentKind))]
#[strum_discriminants(derive(Hash))]
pub(crate) enum Ident {
    #[strum(to_string="Camel({0})")]
    Camel(CamelCase),
    #[strum(to_string="Skewer({0})")]
    Skewer(SkewerCase),
    #[strum(to_string="Snake({0})")]
    Snake(SnakeCase),
    #[strum(to_string="Domain({0})")]
    Domain(Domain),
    #[strum(to_string="Version({0})")]
    Version(Version),
    #[strum(to_string="Undefined({0})")]
    /// [Ident::Undefined] represents a semi plausible ident ... maybe camel case with underscores & dashes
    Undefined(String),
}

impl From<Ident> for TokenKindDef {
    fn from(ident: Ident) -> Self {
        TokenKindDef::Ident(ident)
    }
}

impl<'a> From<Input<'a>> for Ident {
    fn from(value: Input<'a>) -> Self {
        Self::Undefined(value.to_string())
    }
}

impl From<CamelCase> for Ident {
    fn from(value: CamelCase) -> Self {
        Ident::Camel(value)
    }
}
impl From<SkewerCase> for Ident {
    fn from(value: SkewerCase) -> Self {
        Ident::Skewer(value)
    }
}
impl From<SnakeCase> for Ident {
    fn from(value: SnakeCase) -> Self {
        Ident::Snake(value)
    }
}
impl From<Domain> for Ident {
    fn from(value: Domain) -> Self {
        Ident::Domain(value)
    }
}
impl From<Version> for Ident {
    fn from(value: Version) -> Self {
        Ident::Version(value)
    }
}

#[derive(Clone, Builder)]
pub struct DocTokenized {
    pub kind: Token,
    pub version: Token,
    pub defs: HashMap<Ident, Vec<Token>>,
}

#[derive(Clone, Builder)]
pub struct DocLayer {
    pub kind: Token,
    pub defs: HashMap<Ident, Vec<Token>>,
}

pub(super) mod block {
    use crate::parse2::token::TokenKindDef;
    use crate::parse2::{Input, Res};
    use nom::branch::alt;

    pub fn block(input: Input) -> Res<TokenKindDef> {
        use close::token as close;
        use open::token as open;
        alt((open, close))(input)
    }

    pub(super) mod open {
        use crate::parse::model::NestedBlockKind;
        use crate::parse2::token::TokenKindDef;
        use crate::parse2::{Input, Res};
        use nom::branch::alt;
        use nom::Parser;
        use nom_supreme::tag::complete::tag;
        use nom_supreme::ParserExt;

        pub fn angle(input: Input) -> Res<NestedBlockKind> {
            tag("<")(input).map(|(next, _)| (next, NestedBlockKind::Angle))
        }

        pub fn square(input: Input) -> Res<NestedBlockKind> {
            tag("[")(input).map(|(next, _)| (next, NestedBlockKind::Square))
        }

        pub fn parenthesis(input: Input) -> Res<NestedBlockKind> {
            tag("(")(input).map(|(next, _)| (next, NestedBlockKind::Parens))
        }

        pub fn curly(input: Input) -> Res<NestedBlockKind> {
            tag("{")(input).map(|(next, _)| (next, NestedBlockKind::Curly))
        }

        pub fn open(input: Input) -> Res<NestedBlockKind> {
            alt((angle, square, parenthesis, curly))(input)
        }

        pub fn token(input: Input) -> Res<TokenKindDef> {
            open(input).map(|(next, kind)| {
                let kind = TokenKindDef::Open(kind);
                (next, kind)
            })
        }
    }

    pub(super) mod close {
        use crate::parse::model::NestedBlockKind;
        use crate::parse2::token::TokenKindDef;
        use crate::parse2::{Input, Res};
        use nom::branch::alt;
        use nom::Parser;
        use nom_supreme::tag::complete::tag;

        pub fn angle(input: Input) -> Res<NestedBlockKind> {
            tag(">")(input).map(|(next, _)| (next, NestedBlockKind::Angle))
        }

        pub fn square(input: Input) -> Res<NestedBlockKind> {
            tag("]")(input).map(|(next, _)| (next, NestedBlockKind::Square))
        }

        pub fn parenthesis(input: Input) -> Res<NestedBlockKind> {
            tag(")")(input).map(|(next, _)| (next, NestedBlockKind::Parens))
        }

        pub fn curly(input: Input) -> Res<NestedBlockKind> {
            tag("}")(input).map(|(next, _)| (next, NestedBlockKind::Curly))
        }

        pub fn close(input: Input) -> Res<NestedBlockKind> {
            alt((angle, square, parenthesis, curly))(input)
        }
        pub fn token(input: Input) -> Res<TokenKindDef> {
            close(input).map(|(next, kind)| {
                let kind = TokenKindDef::Close(kind);
                (next, kind)
            })
        }
    }
}

pub(super) mod whitespace {
    use crate::parse2::token::TokenKindDef;
    use crate::parse2::{Input, Res};
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use nom::character::complete::space1;
    use nom::combinator::value;
    use nom::multi::many0;

    pub fn space(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::Space, space1)(input)
    }

    pub fn newline(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::Newline, tag("\n"))(input)
    }
    
    pub fn whitespace(input: Input) -> Res<TokenKindDef> {
        alt((space,newline))(input)
    }

    pub fn multi0(input: Input) -> Res<Vec<TokenKindDef>> {
        many0(alt((space, newline)))(input)
    }

    pub fn multi1(input: Input) -> Res<Vec<TokenKindDef>> {
        many0(alt((space, newline)))(input)
    }
}

pub(super) mod punctuation {
    use crate::parse2::token::TokenKindDef;
    use crate::parse2::{Input, Res};
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use nom::combinator::value;

    fn at(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::At, tag("@"))(input)
    }

    fn plus(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::Plus, tag("+"))(input)
    }

    fn segment_sep(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::SegmentSep, tag(":"))(input)
    }

    fn scope(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::Scope, tag("::"))(input)
    }

    fn dot(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::Dot, tag("."))(input)
    }

    fn terminator(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::Terminator, tag(";"))(input)
    }

    fn r#return(input: Input) -> Res<TokenKindDef> {
        value(TokenKindDef::Return, tag("&"))(input)
    }

    pub(super) fn punctuation(input: Input) -> Res<TokenKindDef> {
        alt((scope, segment_sep, dot, plus, at, terminator, r#return))(input)
    }
}

fn kind<O>(mut f: impl FnMut(Input) -> Res<O>) -> impl FnMut(Input) -> Res<TokenKindDef>
where
    O: Into<TokenKindDef>,
{
    move |input| {
        let (next, output) = f(input)?;
        let kind = output.into();
        Ok((next, kind))
    }
}

fn tok<O>(mut f: impl FnMut(Input) -> Res<O>+Copy) -> impl FnMut(Input) -> Res<Token>
where
    O: Into<TokenKindDef>,
{
    move |input| {
        let (next, output) = kind(f)(input.clone())?;
        let len = (next.location_offset() - input.location_offset());
        let span = input.location_offset()..(input.location_offset() + len);
        let kind = output.into();
        let token = Token { span, token: kind };
        Ok((next, token))
    }
}

pub mod err {
    use crate::parse2::Input;
    use strum_macros::Display;
    use thiserror::Error;

    #[derive(Clone, Display, Debug, Error)]
    pub enum TokenErr<'a> {
        Expect {
            expected: &'static str,
            found: Input<'a>,
        },
    }

    impl<'a> TokenErr<'a> {
        pub fn expected(expected: &'static str, found: Input<'a>) -> Self {
            Self::Expect { expected, found }
        }
    }
}

pub fn result<R>(result: Res<R>) -> Result<R, ErrTree> {
    match result {
        Ok((_, e)) => Ok(e),
        Err(nom::Err::Error(err)) => Result::Err(err),
        Err(nom::Err::Failure(err)) => Result::Err(err),
        Err(nom::Err::Incomplete(_)) => panic!("nom::Err::Incomplete not implemented "),
    }
}

#[cfg(test)]
pub mod tests {
    use crate::parse2::token::{result, tokenize};
    use crate::parse2::{log, parse_operation};

    #[test]
    pub fn tokenz() {
        let op= parse_operation("tokenz", 
r#"
Release(version=1.3.7) {
  + <SomeClass>;
}       
        "#);
        
       match result(tokenize(op.input())) {
           Ok(_) => {}
           Err(errs) => {
               log(op.data, errs);
           }
       }

        assert_eq!(op.stack.len(), 0)
    }
    
}