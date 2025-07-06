use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::{preceded, recognize, Span};
use crate::parse::{CamelCase, Domain, SkewerCase, SnakeCase};
use crate::parse2::chars::ident;
use crate::parse2::token::symbol::symbol;
use crate::parse2::token::whitespace::whitespace;
use crate::parse2::{range, to_err, ErrTree, Input, Res};
use nom::branch::alt;
use nom::bytes::complete::{tag, take};
use nom::character::complete::digit1;
use nom::combinator::{all_consuming, eof, into, not};
use nom::error::ParseError;
use nom::multi::{many0, many1, separated_list1};
use nom::sequence::terminated;
use nom::{Needed, Offset, Parser, Slice};
use nom_supreme::ParserExt;
use semver::Version;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;
use std::str::FromStr;
use strum_macros::{Display, EnumDiscriminants, EnumString, EnumTryAs};

fn token<'a>(input: Input<'a>) -> Res<Token<'a>> {
    let (next, (kind,len)) = loc(alt((defined, undefined)))(input.clone())?;
    let token = Token::new(input.slice(..len), kind);
    Ok((next, token))
}

/// returns tuple `Ok(Output,Range<usize>)` 
fn loc<'a,O>(mut f:impl FnMut(Input<'a>) -> Res<O> ) -> impl FnMut(Input<'a>) -> Res<(O,usize)> {
    move |input| {
       let (next, output) = f(input.clone())?;
       let len = input.len() - next.len();
       Ok((next, (output,len)))
    }
}

fn defined(input: Input) -> Res<TokenKind> {
    alt((whitespace, symbol, into(ident),into(version)))(input)
}
fn undefined(input: Input) -> Res<TokenKind> {
    recognize(many1(preceded(not(defined), take(1usize))))(input)
        .map(|(next, undef)| (next, TokenKind::Undefined(undef.to_string())))
}

fn into_token<'a, O>(f: impl FnMut(Input) -> Res<O> + Copy) -> impl FnMut(Input) -> Res<Token<'a>>
where
    O: Into<Token<'a>>,
{
    move |input| into(f)(input)
}

fn tokens(input: Input) -> Res<Vec<Token>> {
    many0(token)(input)
}

fn tokenize(input: Input) -> Res<Vec<Token>> {
    all_consuming(terminated(tokens, eof))(input)
}

/*
fn version_decl(input: Input) -> Res<TokenKind> {
    preceded(tag("version="), into(version))(input)
}

 */

fn parse_u64(input: Input) -> Res<u64> {
    let (next, digits) = digit1(input)?;
    let digits = digits.to_string();
    let digits: u64 = digits.parse().map_or_else(|_| u64::MAX, |r| r);
    Ok((next, digits))
}

fn version(input: Input) -> Res<Version> {
    let (next,version) = recognize(separated_list1(tag("."), parse_u64))(input.clone())?;
    let version = version.to_string();
    let version = semver::Version::from_str(version.as_str()).map_err(|err|to_err(input,err))?; 
    Ok((next, version))
}

#[derive(Clone, Debug)]
pub struct TokenDef<'a, K> {
    span: Input<'a>,
    kind: K,
}

pub type Token<'a> = TokenDef<'a, TokenKind>;
pub type IdentToken<'a> = TokenDef<'a, Ident>;

impl<'a, T> TokenDef<'a, T> {
    fn new(span: Input<'a>, kind: T) -> Self {
        Self { span, kind }
    }

    fn with<T2>(self, kind: T2) -> TokenDef<'a, T2> {
        TokenDef {
            span: self.span,
            kind,
        }
    }
}

#[derive(Debug, Clone)]
struct Block<T> {
    kind: BlockKind,
    tokens: Vec<T>,
}

impl<T> Block<T> {
    fn new(kind: impl Into<BlockKind>, tokens: Vec<T>) -> Self {
        let kind = kind.into();
        Self { kind, tokens }
    }
}

impl<T> Display for Block<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Block({}){{ {} tokens }}", self.kind, self.tokens.len())
    }
}

#[derive(Clone, Debug, Display, EnumString)]
pub enum DocType {
    Package,
}

#[derive(Clone, Debug)]
struct Header {
    kind: DocType,
    version: Version,
}

impl Display for Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(version={})", self.kind, self.version)
    }
}

impl Header {
    pub fn new(kind: DocType, version: Version) -> Self {
        Self { kind, version }
    }
}

#[derive(Clone, Debug, EnumDiscriminants, Display, EnumTryAs, Eq, PartialEq)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TokenKindDisc))]
#[strum_discriminants(derive(Hash, Display))]
pub enum TokenKind {
    /*    #[strum(to_string="Header({0})")]
       Header(Header),

    */
    #[strum(to_string = "Ident({0})")]
    Ident(Ident),
    #[strum(to_string = "BlockOpen({0})")]
    Open(NestedBlockKind),
    #[strum(to_string = "BlockClose({0})")]
    Close(NestedBlockKind),

    //#[strum(to_string="BlockOpen({0})")]
    //Block(Block<Token<'a>>),
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
    /// `=`
    Equals,
    /// semicolon ';' (just as god intended)
    Terminator,
    /// `&` good old ampersand
    Return,
    /// `version=` i.e.: Def(`version=`1.1.5) ... tells which parser version to use
    VersionPrelude,
    /// version=`1.2.3`
    Version(Version),
    /// a `space` or a `tab`
    Space,
    /// any cluster of whitespace: `space`, `tab` and `newline`
    Newline,
    /// anything that is not recognized by the parser
    #[strum(to_string = "Undefined({0})")]
    Undefined(String),
    /// End of File
    EOF,
    /// an erroneous token...
    Err(Range<usize>),
}

impl<'a> From<Version> for TokenKind {
    fn from(version: Version) -> Self {
        Self::Version(version)
    }
}

impl TokenKind {
    fn open(block: NestedBlockKind) -> Self {
        Self::Open(block)
    }

    fn close(block: NestedBlockKind) -> Self {
        Self::Close(block)
    }
}
#[derive(Clone, Debug, EnumDiscriminants, Display, Eq, PartialEq)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(IdentKind))]
#[strum_discriminants(derive(Hash))]
pub(crate) enum Ident {
    #[strum(to_string = "Camel({0})")]
    Camel(CamelCase),
    #[strum(to_string = "Skewer({0})")]
    Skewer(SkewerCase),
    #[strum(to_string = "Snake({0})")]
    Snake(SnakeCase),
    #[strum(to_string = "Domain({0})")]
    Domain(Domain),
    #[strum(to_string = "Version({0})")]
    Version(Version),
    //#[strum(to_string = "Undefined({0})")]
    // [Ident::Undefined] represents a semi plausible ident ... maybe camel case with underscores & dashes
    // Undefined(String),
}

impl From<Ident> for TokenKind {
    fn from(ident: Ident) -> Self {
        TokenKind::Ident(ident)
    }
}

/*
impl<'a> From<Input<'a>> for Ident {
    fn from(value: Input<'a>) -> Self {
        Self::Undefined(value.to_string())
    }
}

 */

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

/*
#[derive(Clone, Builder)]
pub struct DocTokenized {
    pub kind: Token,
    pub version: Token,
    pub defs: HashMap<Ident, Vec<Token>>,
}


 */

pub(super) mod block {
    use nom::error::FromExternalError;

    /*
    pub fn block_with<O>(tokens: impl FnMut(Input) -> Res<Vec<O>>+Copy) -> impl FnMut(Input) -> Res<Vec<O>> {
        |input| alt((angle(tokens), square(tokens), parenthesis(tokens), curly(tokens)))(input)
    }



    pub fn block(input: Input) -> Res<TokenKind> {
        alt((angle(tokens),square(tokens),parenthesis(tokens),curly(tokens)))(input)
    }

    pub fn angle<O>(tokens: impl FnMut(Input) -> Res<O>) -> impl Fn(Input) -> Res<Block<O>> {
        |input| delimited(open::angle, tokens, close::angle)(input).map(|(next,tokens)|(next,Block::new(NestedBlockKind::Angle,tokens).into()))
    }

        pub fn square<O>(tokens: impl FnMut(Input) -> Res<O>) -> impl Fn(Input) -> Res<Block<O>> {
        |input|delimited(open::square, tokens, close::square)(input).map(|(next,tokens)|(next,Block::new(NestedBlockKind::Square,tokens).into()))
    }

    pub fn parenthesis<O>(tokens: impl FnMut(Input) -> Res<O>) -> impl Fn(Input) -> Res<Block<O>> {
        |input| delimited(open::parenthesis, tokens, close::parenthesis)(input).map(|(next,tokens)|(next,Block::new(NestedBlockKind::Parens,tokens).into()))
    }

    pub fn curly<O>(tokens: impl FnMut(Input) -> Res<O>) -> impl Fn(Input) -> Res<Block<O>> {
        |input|delimited(open::curly, tokens, close::curly)(input).map(|(next,tokens)|(next,Block::new(NestedBlockKind::Curly,tokens).into()))
    }


     */
    pub(super) mod open {
        use crate::parse::model::NestedBlockKind;
        use crate::parse2::token::TokenKind;
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

        pub fn token(input: Input) -> Res<TokenKind> {
            open(input).map(|(next, kind)| {
                let kind = TokenKind::Open(kind);
                (next, kind)
            })
        }
    }

    pub(super) mod close {
        use crate::parse::model::NestedBlockKind;
        use crate::parse2::token::TokenKind;
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
            tag(")")(input.clone()).map(|(next, _)| (next, NestedBlockKind::Parens))
        }

        pub fn curly(input: Input) -> Res<NestedBlockKind> {
            tag("}")(input).map(|(next, _)| (next, NestedBlockKind::Curly))
        }

        pub fn close(input: Input) -> Res<NestedBlockKind> {
            alt((angle, square, parenthesis, curly))(input)
        }
        pub fn token(input: Input) -> Res<TokenKind> {
            close(input).map(|(next, kind)| {
                let kind = TokenKind::Close(kind);
                (next, kind)
            })
        }
    }
}

pub(super) mod whitespace {
    use crate::parse2::token::TokenKind;
    use crate::parse2::{Input, Res};
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use nom::character::complete::space1;
    use nom::combinator::value;
    use nom::multi::many0;

    pub fn space(input: Input) -> Res<TokenKind> {
        value(TokenKind::Space, space1)(input)
    }

    pub fn newline(input: Input) -> Res<TokenKind> {
        value(TokenKind::Newline, tag("\n"))(input)
    }

    pub fn whitespace(input: Input) -> Res<TokenKind> {
        alt((space, newline))(input)
    }

    pub fn multi0(input: Input) -> Res<Vec<TokenKind>> {
        many0(alt((space, newline)))(input)
    }

    pub fn multi1(input: Input) -> Res<Vec<TokenKind>> {
        many0(alt((space, newline)))(input)
    }
}

pub(super) mod literal {}

pub(super) mod symbol {
    use crate::parse2::token::TokenKind;
    use crate::parse2::{Input, Res};
    use nom::branch::alt;
    use nom::bytes::complete::tag;
    use nom::combinator::value;

    fn at(input: Input) -> Res<TokenKind> {
        value(TokenKind::At, tag("@"))(input)
    }

    fn plus(input: Input) -> Res<TokenKind> {
        value(TokenKind::Plus, tag("+"))(input)
    }

    fn segment_sep(input: Input) -> Res<TokenKind> {
        value(TokenKind::SegmentSep, tag(":"))(input)
    }

    fn scope(input: Input) -> Res<TokenKind> {
        value(TokenKind::Scope, tag("::"))(input)
    }

    fn dot(input: Input) -> Res<TokenKind> {
        value(TokenKind::Dot, tag("."))(input)
    }

    pub(super) fn equals(input: Input) -> Res<TokenKind> {
        value(TokenKind::Equals, tag("="))(input)
    }

    fn terminator(input: Input) -> Res<TokenKind> {
        value(TokenKind::Terminator, tag(";"))(input)
    }

    fn r#return(input: Input) -> Res<TokenKind> {
        value(TokenKind::Return, tag("&"))(input)
    }

    pub(super) fn symbol(input: Input) -> Res<TokenKind> {
        use super::block::{close, open};
        alt((
            scope,
            equals,
            segment_sep,
            dot,
            plus,
            at,
            terminator,
            r#return,
            open::token,
            close::token,
        ))(input)
    }
}

fn kind<O>(mut f: impl FnMut(Input) -> Res<O>) -> impl FnMut(Input) -> Res<TokenKind>
where
    O: Into<TokenKind>,
{
    move |input| {
        let (next, output) = f(input)?;
        let kind = output.into();
        Ok((next, kind))
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
        Err(nom::Err::Incomplete(needed)) => match needed {
            Needed::Unknown => panic!("Needed::Unknown"),
            Needed::Size(size) => panic!("Needed::Size(size={})", size),
        },
    }
}

pub mod util {
    use crate::parse2::{Input, Res};
    use nom::Parser;
    use std::fmt::Debug;

    pub fn diagnose<O>(mut f: impl FnMut(Input) -> Res<O>) -> impl FnMut(Input) -> Res<O>
    where
        O: Debug + ToString,
    {
        move |input| {
            f(input)
                .map(|(next, output)| {
                    println!("{:?}", output);
                    (next, output)
                })
                .map_err(|err| {
                    println!("Err: {:?}", err);
                    err
                })
        }
    }

    /*
    pub fn not<O, F>(mut check: impl FnMut(Input) -> Res<O>) -> impl FnMut(Input) -> Res<O>
    where
        F: FnMut(Input) -> Res<O>,
    {
        move |input| {
            let i = input.clone();
            match check.parse(input) {
                Ok(_) => Err(nom::Err::Error(ErrTree::from_error_kind(i, ErrorKind::Not))),
                Err(_) => Ok((i, ())),
            }
        }
    }

     */
}

#[cfg(test)]
pub mod tests {
    use crate::parse2::parse_operation;
    use crate::parse2::token::symbol::symbol;
    use crate::parse2::token::{result, tokenize, undefined, Token, TokenKind};
    use nom::combinator::all_consuming;
    use crate::parse2::token::util::diagnose;

    #[test]
    pub fn symbols() {
        let op = parse_operation("equals", "=");
        let token = result(all_consuming(symbol)(op.input())).unwrap();
        assert_eq!(token, TokenKind::Equals);
    }

    #[test]
    pub fn test_undefined() {
        let op = parse_operation("undefined", "^%%skewer");
        match diagnose(undefined)(op.input()) {
            Ok(_) => {}
            Err(err) => {
                panic!();
            }
        }
    }

    #[test]
    pub fn tokenz() {
        let op = parse_operation(
            "tokenz",
            r#"
Release(version=1.3.7){
  + <SomeClass>;
}       
        "#,
        );

        let tokens = result(tokenize(op.input())).unwrap();
        diag(&tokens);

        assert_eq!(op.stack.len(), 0)
    }

    fn diag(tokens: &Vec<Token>) {
        println!("*****************");
        for token in tokens {
            println!("token: {}", token.kind);
        }
        println!("*****************");
    }
}
