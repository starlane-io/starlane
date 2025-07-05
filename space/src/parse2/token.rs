use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::{preceded, Span};
use crate::parse::{rec_version, CamelCase, Domain, SkewerCase, SnakeCase};
use crate::parse2::chars::ident;
use crate::parse2::token::block::block;
use crate::parse2::token::punctuation::punctuation;
use crate::parse2::{to_err, ErrTree, Input, Res};
use derive_builder::Builder;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::multispace0;
use nom::combinator::into;
use nom::error::{ErrorKind, ParseError};
use nom::multi::many0;
use nom::sequence::tuple;
use nom::{Offset, Parser, Slice};
use nom_supreme::ParserExt;
use semver::Version;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::Range;
use std::str::FromStr;
use ascii::AsciiChar::P;
use strum_macros::{Display, EnumDiscriminants, EnumString, EnumTryAs, FromRepr};

fn token(input: Input) -> Res<Token> {
    use whitespace::whitespace;
    alt((tok(block),tok(whitespace),tok(punctuation), tok(ident)))(input)
}

fn tokens(input: Input) -> Res<Vec<Token>> {
    many0(token)(input)
}


fn tokenize(input: Input) -> Res<Vec<Token>> {
  
    
    todo!()

}


fn header(input: Input) -> Res<Header> {
    
    
    let (next,(ident,_,block))= tuple((input_token(ident),multispace0,input_token(block::parenthesis(version_decl))))(input.clone())?;
    let ident_str = ident.to_string();
    let kind = DocType::from_str(ident_str.as_str()).map_err(|e| to_err(input,e)) ?;
    
    let header = Header::new(kind,version);

    (next,tokens)})

}

fn version_decl(input: Input) -> Res<InputToken> {
    preceded(tag("version="),input_token(into(version)))(input)
}

fn version(input: Input) -> Res<Version> {
    let (next, version) = rec_version(input.clone())?;
    let version = version.to_string();
    let str_input = version.as_str();
    let rtn = semver::Version::parse(str_input);
    match rtn {
        Ok(version) => Ok((next,version)),
        Err(err) => Err(nom::Err::Error(ErrTree::from_error_kind(input, ErrorKind::Fail)))
    }

}

#[derive(Clone, Debug)]
pub struct TokLocDef<S,T>{
    span: S,
    kind: T,
}

pub type TokLoc<T> = TokLocDef<Range<usize>,TokenKindDef>;

pub type Token = TokLoc<TokenKindDef>;
pub type InputToken<'a> = TokLocDef<Input<'a>, TokenKindDef>;
pub type IdentToken = TokLoc<Ident>;

impl <S,T> TokLocDef<S,T> {
    fn new(span: S, kind: T) -> TokLocDef<S,T> {
        Self {
            span,
            kind
        }
    }

    fn with<T2>(self, kind: T2) -> TokLocDef<S,T2> {
        let span = self.span;
        Self {
            span,
            kind
        }
    }
}

impl <T> TokLoc<T> {
    pub fn with_span<'a,S2>(self, input: Input ) -> TokLocDef<S2, T> {
        let span = Input::new_extra(input.extra.data,input.extra.clone()).slice(&self.span);
        let kind = self.kind;
        TokLocDef {
            span,
            kind
        }
    }
}


#[derive(Debug, Clone )]
struct Block<T> {
    kind: BlockKind,
    tokens: Vec<T>
}



impl Into<TokenKindDef> for Block<Token> {
    fn into(self) -> TokenKindDef {
        TokenKindDef::Block(self)
    }
}

impl <T> Block<T> {
    fn new( kind: impl Into<BlockKind>, tokens: Vec<T> ) -> Self {
        let kind = kind.into();
        Self {
            kind,
            tokens,
        }
    }
}

impl <T> Display for Block<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Block({}){{ {} tokens }}", self.kind, self.tokens.len())
    }
}

#[derive(Clone, Debug, Display,EnumString)]
pub enum DocType {
    Package
}

#[derive(Clone, Debug )]
struct Header {
    kind: DocType,
    version: Version
}

impl Header {
    pub fn new(kind: DocType, version: Version) -> Self {
        Self {
            kind,
            version,
        }
    }
}

#[derive(Clone, Debug, EnumDiscriminants, Display,EnumTryAs)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TokenKind))]
#[strum_discriminants(derive(Hash,Display))]
enum TokenKindDef {
    #[strum(to_string="Header({0})")]
    Header(Header),
    #[strum(to_string="Ident({0})")]
    Ident(Ident),
    #[strum(to_string="BlockOpen({0})")]
    Open(NestedBlockKind),
    #[strum(to_string="BlockClose({0})")]
    Close(NestedBlockKind),

    #[strum(to_string="BlockOpen({0})")]
    Block(Block<Token>),
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
    /// version=`1.2.3`
    Version(Version),
    /// a `space` or a `tab`
    Space,
    /// any cluster of whitespace: `space`, `tab` and `newline`
    Newline,
    /// an erroneous token...
    Err(Range<usize>),
}

impl TryInto<IdentToken> for InputToken {
    type Error = nom::Err(ErrTree);

    fn try_into(self) -> Result<IdentToken, Self::Error> {
        match &self.kind {
            TokenKindDef::Ident(ident) => Ok(self.with(ident.clone())),
            what => to_err(self.span,format!("Illegal cast. '{}' cannot be casted into: 'Ident'", self.kind ))
        }
    }
} 

impl From<Version> for TokenKindDef {
    fn from(version: Version) -> Self {
        Self::Version(version)
    }
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
    use crate::parse::model::NestedBlockKind;
    use crate::parse2::token::{tokens, Block, TokenKindDef};
    use crate::parse2::{ErrTree, Input, Res};
    use nom::branch::alt;
    use nom::error::FromExternalError;
    use nom::sequence::delimited;
    use nom_supreme::error::Expectation;

    pub fn block_with<O>(tokens: impl FnMut(Input) -> Res<Vec<O>>+Copy) -> impl FnMut(Input) -> Res<Vec<O>> {
        |input| alt((angle(tokens), square(tokens), parenthesis(tokens), curly(tokens)))(input)
    }


    pub fn block(input: Input) -> Res<TokenKindDef> {
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
        use crate::parse2::{to_err, ErrTree, Input, Res};
        use nom::branch::alt;
        use nom::{Parser, Slice};
        use nom_supreme::error::{BaseErrorKind, Expectation};
        use nom_supreme::tag::complete::tag;

        pub fn angle(input: Input) -> Res<NestedBlockKind> {
            tag(">")(input).map(|(next, _)| (next, NestedBlockKind::Angle))
        }

        pub fn square(input: Input) -> Res<NestedBlockKind> {
            tag("]")(input).map(|(next, _)| (next, NestedBlockKind::Square))
        }

        pub fn parenthesis(input: Input) -> Res<NestedBlockKind> {
            tag(")")(input.clone()).map(|(next, _)| (next, NestedBlockKind::Parens)).map_err(|err| {
                let kind = BaseErrorKind::Expected(Expectation::Char(')');
                let slice = input.slice(..1);
                ErrTree::from_error_kind()
                to_err(slice,)
                todo!();
            })
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
        let token = Token { span, kind: kind };
        Ok((next, token))
    }
}

fn input_token<O>(mut f: impl FnMut(Input) -> Res<O>+Copy) -> impl FnMut(Input) -> Res<InputToken>
where
    O: Into<TokenKindDef>,
{
    move |input| {
        let (next, output) = kind(f)(input.clone())?;
        let kind = output.into();
        let token = InputToken::new( input, kind);
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