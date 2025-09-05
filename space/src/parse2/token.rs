use crate::parse::model::{BlockSymbol, NestedSymbols};
use crate::parse::util::{new_span, preceded, recognize, Span};
use crate::parse::{CamelCase, Ctx, Domain, SkewerCase, SnakeCase};
use crate::parse2::ast::err::{AstErr, AstErrKind};
use crate::parse2::chars::ident;
use crate::parse2::document::Unit;
use crate::parse2::err::{ErrTree, ParseErrs2Proto};
use crate::parse2::token::symbol::symbol;
use crate::parse2::token::whitespace::whitespace;
use crate::parse2::{Input, Res};
use nom::branch::alt;
use nom::bytes::complete::{tag, take};
use nom::character::complete::digit1;
use nom::combinator::{into, not};
use nom::error::{ErrorKind, FromExternalError, ParseError};
use nom::multi::{many0, many1, separated_list1};
use nom::{Needed, Offset, Parser, Slice};
use nom_supreme::ParserExt;
use semver::Version;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;
use std::slice::Iter;
use std::str::FromStr;
use std::sync::Arc;
use nom_locate::LocatedSpan;
use nom_supreme::error::GenericErrorTree;
use strum_macros::{Display, EnumDiscriminants, EnumString, EnumTryAs};

pub(crate) fn tokens(input: Input) -> Res<Vec<Token>> {
    pub(crate) fn token<'a>(input: Input<'a>) -> Res<Token<'a>> {
        let (next, (kind, len)) = loc(alt((defined, undefined)))(input.clone())?;
        let token = Token::new(input.slice(..len), kind);
        Ok((next, token))
    }

    /// returns tuple `Ok(Output,Range<usize>)`
    fn loc<'a, O>(
        mut f: impl FnMut(Input<'a>) -> Res<O>,
    ) -> impl FnMut(Input<'a>) -> Res<(O, usize)> {
        move |input| {
            let (next, output) = f(input.clone())?;
            let len = input.len() - next.len();
            Ok((next, (output, len)))
        }
    }

    fn defined(input: Input) -> Res<TokenKind> {
        alt((whitespace, symbol, into(ident), into(version)))(input)
    }
    fn undefined(input: Input) -> Res<TokenKind> {
        recognize(many1(preceded(not(defined), take(1usize))))(input)
            .map(|(next, undef)| (next, TokenKind::Undefined(undef.to_string())))
    }

    fn into_token<'a, O>(
        f: impl FnMut(Input) -> Res<O> + Copy,
    ) -> impl FnMut(Input) -> Res<Token<'a>>
    where
        O: Into<Token<'a>>,
    {
        move |input| into(f)(input)
    }

    many0(token)(input)
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
    let (next, version) = recognize(separated_list1(tag("."), parse_u64))(input.clone())?;
    let version = version.to_string();
    match semver::Version::from_str(version.as_str()) {
        Ok(version) => {
            Ok((next, version))
        }
        Err(err) => {
            let span = input.slice(0..(next.location_offset() - input.location_offset()));
            Err(nom::Err::Failure(ErrTree::from_external_error(span, ErrorKind::Alpha, AstErrKind::VersionFormat)))
        }
    }
}

pub type TokenDef<'a,K> = Unit<'a, K>;

pub type Token<'a> = TokenDef<'a, TokenKind>;
pub type IdentToken<'a> = TokenDef<'a, Ident>;

#[derive(Debug, Clone)]
struct Block<T> {
    kind: BlockSymbol,
    tokens: Vec<T>,
}

impl<T> Block<T> {
    fn new(kind: impl Into<BlockSymbol>, tokens: Vec<T>) -> Self {
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

impl Into<CamelCase> for DocType {
    fn into(self) -> CamelCase {
        CamelCase(self.to_string())
    }
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
    #[strum(to_string = "{0}")]
    Ident(Ident),
    #[strum(to_string = "BlockOpen({0})")]
    Open(NestedSymbols),
    #[strum(to_string = "BlockClose({0})")]
    Close(NestedSymbols),

    //#[strum(to_string="BlockOpen({0})")]
    //Block(Block<Token<'a>>),
    /// `+` symbol
    Plus,
    /// `@` symbol
    At,
    /// `::` symbol
    SuperSeparator,
    /// `:` symbol
    Separator,
    /// `.` symbol (used for properties and child defs)
    Dot,
    /// `=`
    Equals,
    /// semicolon ';' (just as god intended)
    Terminator,
    /// `&` good old ampersand
    Return,
    /// `version` i.e.: Def(`version`=1.1.5) ... tells which parser version to use
    VersionLiteral,
    /// version=`1.2.3`
    Version(Version),
    /// a `space` or a `tab`
    Space,
    /// newline \n
    Newline,
    /// any cluster of whitespace: `space`, `tab` and `newline`
    Whitespace,
    /// anything that is not recognized by the parser
    #[strum(to_string = "Undefined({0})")]
    Undefined(String),
    /// End of File
    EOF,
    /// an erroneous token...
    Err(Range<usize>),
}

impl TokenKind {
    pub fn describe_format(&self) -> &'static str {
        match &self {
            TokenKind::Ident(ident) => ident.description(),
            TokenKind::Open(symbol) => match symbol {
                NestedSymbols::Curly => "block open '{'",
                NestedSymbols::Parens => "block open '('",
                NestedSymbols::Square => "block open '['",
                NestedSymbols::Angle => "block open '<'",
            },
            TokenKind::Close(symbol) => match symbol {
                NestedSymbols::Curly => "block close '}'",
                NestedSymbols::Parens => "block close')'",
                NestedSymbols::Square => "block close']'",
                NestedSymbols::Angle => "block close '>'",
            },
            TokenKind::Plus => "'+' symbol",
            TokenKind::At => "'@' symbol",
            TokenKind::Separator => "':' symbol",
            TokenKind::SuperSeparator => "'::' scope symbol",
            TokenKind::Dot => "'.' dot symbol",
            TokenKind::Equals => "'=' equals symbol",
            TokenKind::Terminator => "';' terminator symbol (semicolon)",
            TokenKind::Return => "'&' return symbol",
            TokenKind::VersionLiteral => "'version=' literal",
            TokenKind::Version(_) => IdentKind::Version.description(),
            TokenKind::Space => "' ' space (whitespace)",
            TokenKind::Whitespace => "'\\n' newline (whitespace)",
            TokenKind::Undefined(_) => "undefined",
            TokenKind::EOF => "End of File",
            TokenKind::Err(_) => "Err",
            TokenKind::Newline => "\n"
        }
    }

    pub fn whitespace(&self) -> &'static WhiteSpace {
        match self {
            TokenKind::Space => &WhiteSpace::Space,
            TokenKind::Whitespace => &WhiteSpace::Newline,
            _ => &WhiteSpace::None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum WhiteSpace {
    None,
    Space,
    Newline,
    Either,
}

impl TokenKind {
    pub fn is_whitespace(&self, whitespace: &'static WhiteSpace) -> bool {
        match whitespace {
            WhiteSpace::None => false,
            WhiteSpace::Space => self.is_space(),
            WhiteSpace::Newline => self.is_newline(),
            WhiteSpace::Either => self.is_space() || self.is_newline(),
        }
    }

    pub fn is_space(&self) -> bool {
        *self == Self::Space
    }

    pub fn is_newline(&self) -> bool {
        *self == Self::Whitespace
    }
}

impl<'a> From<Version> for TokenKind {
    fn from(version: Version) -> Self {
        Self::Version(version)
    }
}

impl TokenKind {
    fn open(block: NestedSymbols) -> Self {
        Self::Open(block)
    }

    fn close(block: NestedSymbols) -> Self {
        Self::Close(block)
    }
}
#[derive(Clone, Debug, EnumDiscriminants, Display, Eq, PartialEq)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(IdentKind))]
#[strum_discriminants(derive(Hash))]
pub(crate) enum Ident {
    #[strum(to_string = "{0}")]
    Camel(CamelCase),
    #[strum(to_string = "{0}")]
    Skewer(SkewerCase),
    #[strum(to_string = "{0}")]
    Snake(SnakeCase),
    #[strum(to_string = "{0}")]
    Domain(Domain),
    #[strum(to_string = "{0}")]
    Version(Version),
    //#[strum(to_string = "Undefined({0})")]
    // [Ident::Undefined] represents a semi plausible ident ... maybe camel case with underscores & dashes
    // Undefined(String),
}

impl Ident {
    pub fn description(&self) -> &'static str {
        let kind: IdentKind = self.clone().into();
        kind.description()
    }
}

impl IdentKind {
    pub fn description(&self) -> &'static str {
        match self {
            IdentKind::Camel => "'CamelCase' mixed case alphanumeric characters (must start with a capitol letter)",
            IdentKind::Skewer => "'skewer-case' lowercase alphanumeric plus dash '-' (must start with lowercase letter)",
            IdentKind::Snake =>  "'snake-case' lowercase alphanumeric plus underscore '_' (must start with lowercase letter)",
            IdentKind::Domain => "'domain-case.com' lowercase alphanumeric plus dash and dot '.' (must start with lowercase letter) AND consecutive dots are not allowed: 'domain..com' == no!'",
            IdentKind::Version => "'major.minor.patch-prerelease.1+build-metadata' standard semver  i.e.: '1.3.5'  Also supports prerelease and build metadata:  '0.1.13-alpha.3+deprecated'"
        }
    }
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
        use crate::parse::model::NestedSymbols;
        use crate::parse2::token::TokenKind;
        use crate::parse2::{Input, Res};
        use nom::branch::alt;
        use nom::Parser;
        use nom_supreme::tag::complete::tag;
        use nom_supreme::ParserExt;

        pub fn angle(input: Input) -> Res<NestedSymbols> {
            tag("<")(input).map(|(next, _)| (next, NestedSymbols::Angle))
        }

        pub fn square(input: Input) -> Res<NestedSymbols> {
            tag("[")(input).map(|(next, _)| (next, NestedSymbols::Square))
        }

        pub fn parenthesis(input: Input) -> Res<NestedSymbols> {
            tag("(")(input).map(|(next, _)| (next, NestedSymbols::Parens))
        }

        pub fn curly(input: Input) -> Res<NestedSymbols> {
            tag("{")(input).map(|(next, _)| (next, NestedSymbols::Curly))
        }

        pub fn open(input: Input) -> Res<NestedSymbols> {
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
        use crate::parse::model::NestedSymbols;
        use crate::parse2::token::TokenKind;
        use crate::parse2::{Input, Res};
        use nom::branch::alt;
        use nom::Parser;
        use nom_supreme::tag::complete::tag;

        pub fn angle(input: Input) -> Res<NestedSymbols> {
            tag(">")(input).map(|(next, _)| (next, NestedSymbols::Angle))
        }

        pub fn square(input: Input) -> Res<NestedSymbols> {
            tag("]")(input).map(|(next, _)| (next, NestedSymbols::Square))
        }

        pub fn parenthesis(input: Input) -> Res<NestedSymbols> {
            tag(")")(input.clone()).map(|(next, _)| (next, NestedSymbols::Parens))
        }

        pub fn curly(input: Input) -> Res<NestedSymbols> {
            tag("}")(input).map(|(next, _)| (next, NestedSymbols::Curly))
        }

        pub fn close(input: Input) -> Res<NestedSymbols> {
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
        value(TokenKind::Whitespace, tag("\n"))(input)
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
        value(TokenKind::Separator, tag(":"))(input)
    }

    fn scope(input: Input) -> Res<TokenKind> {
        value(TokenKind::SuperSeparator, tag("::"))(input)
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

pub fn result<'a, R>(result: Result<(LocatedSpan<&'a str, &'a Arc<std::string::String>>, R), nom::Err<ErrTree<'a>>>) -> Result<R, ParseErrs2Proto<'a>> {
    match result {
        Ok((_, e)) => Ok(e),
        Err(nom::Err::Error(err)) => Err(ParseErrs2Proto::from(err)),
        Err(nom::Err::Failure(err)) => Err(ParseErrs2Proto::from(err)),
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

/*
#[cfg(test)]
pub mod tests {
    use crate::parse2::parse;
    use crate::parse2::token::symbol::symbol;
    use crate::parse2::token::util::diagnose;
    use crate::parse2::token::{result, TokenKind};
    use insta::_macro_support::assert_snapshot;
    use insta::assert_snapshot;
    use nom::combinator::all_consuming;
    use std::sync::Arc;

    #[test]
    pub fn symbols() {
        let data: Arc<String> = "=".to_string().into();
        let result = parse(&data);
        let token = result(all_consuming(symbol)(op.input())).unwrap();
        assert_eq!(token, TokenKind::Equals);
        assert_snapshot!(token);
    }

    #[test]
    pub fn test_undefined() {
        let op = parse("undefined", "^%%skewer");
        let (input, kind) = diagnose(undefined)(op.input()).unwrap();
        assert_snapshot!(input);
        assert_snapshot!(kind);
    }

    #[test]
    pub fn tokenz() {
        let op = parse(
            "tokenz",
            r#"
Release(version=1.3.7){
  + <SomeClass>;
}
        "#,
        );

        let tokens = result(tokenize(op.input())).unwrap();
        assert_snapshot!(format!("{:?}", tokens));

        assert_eq!(op.stack.len(), 0)
    }


    #[test]
    pub fn more_tokens() {
        let op = parse(
            "more_tokens",
            r#"
Package(version=1.3.7){
  + <SomeClass> {

  }
  + 1.0.3<Slice> {

  }

}





        "#,
        );

        let tokens = result(tokenize(op.input())).unwrap();
        assert_snapshot!(format!("{:?}", tokens));

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

 */

#[derive(Clone, Debug)]
pub struct Tokens<'a> {
    pub tokens: Vec<Token<'a>>,
}

impl<'a> Tokens<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self { tokens }
    }

    pub fn iter(self) -> TokenIter<'a> {
        TokenIter::new( self.tokens)
    }
}

#[derive(Clone, Debug)]
pub struct TokenIter<'a> {
    tokens: &'a[Token<'a>]
}

impl<'a> Tokens<'a> {

    fn empty(&self) -> Input<'a> {
        let string = self.data.as_str().slice((self.data.len() - 1)..self.data.len());
        let empty = Input::new_extra(string, self.data);
        empty
    }


    /// return the next token that is not whitespace: [TokenKind::Space] || [TokenKind::Whitespace]
    pub fn skip_ws(&'a self) -> Option<(Self,&'a Token<'a>)> {
        let mut iter = self.tokens.iter();
        let mut index = 0u16;
        while let Some(token) = iter.next() {
            index+=1;
            if !token.kind.is_whitespace(&WhiteSpace::Either) {
                return Some((next,token));
            }
        }
        /// out of tokens
        None
    }

    pub fn expect(
        &'a self,
        id: &'static str,
        expect: &TokenKind,
    ) -> Result<Option<(Self,&'a Token<'a>)>, AstErr<'a>> {
        if let Some((next,token)) = self.skip_ws() {
            if token.kind == *expect {
                Ok(Some((next,token)))
            } else {
                Err(
                    AstErr::new(token.span, AstErrKind::ExpectedKind {
                        id,
                        kind: expect.clone(),
                        found: token.kind.clone(),
                    }
                    ))
            }
        } else {
            Err(AstErr::new(self.empty(), AstErrKind::UnexpectedEof(expect.clone())))?
        }
    }

    pub fn space(&'a self) -> Result<Option<(Self,&'a Token<'a>)>, AstErr<'a>> {
        self.whitespace_kind(&WhiteSpace::Space)
    }

    pub fn newline(&'a self) -> Result<Option<(Self,&'a Token<'a>)>, AstErr<'a>> {
        self.whitespace_kind(&WhiteSpace::Newline)
    }

    pub fn whitespace(&'a self) -> Result<Option<(Self,&'a Token<'a>)>, AstErr<'a>> {
        self.whitespace_kind(&WhiteSpace::Either)
    }
    fn whitespace_kind(
        &'a self,
        whitespace: &'static WhiteSpace,
    ) -> Result<Option<(Self,&'a Token<'a>)>, AstErr<'a>> {
        let mut next = self.clone();
        if let Some(token) = next.next() {
            if token.kind.is_whitespace(whitespace) {
                Ok(Some((next,token)))
            } else {
                Err(AstErr::new(
                    token.span,
                    AstErrKind::Whitespace(token.kind.clone()),
                ))
            }
        } else {
            Err(AstErr::new( self.empty(), AstErrKind::UnexpectedEof(TokenKind::Whitespace)))
        }
    }
}

impl<'a> Iterator for TokenIter<'a> {
    type Item = &'a Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
