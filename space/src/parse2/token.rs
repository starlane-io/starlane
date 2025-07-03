use std::collections::HashMap;
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::{recognize, Span};
use crate::parse::{camel_case, rec_version, CamelCase, Domain, NomErr, SkewerCase, SnakeCase};
use crate::parse2::token::err::TokenErr;
use crate::parse2::{camel, Input, ParseErrs, Res};
use nom::bytes::complete::is_a;
use nom_supreme::ParserExt;
use std::str::FromStr;
use derive_builder::Builder;
use nom::branch::alt;
use nom::character::complete::{alphanumeric0, multispace0, multispace1};
use nom::character::streaming::space0;
use nom::error::{ErrorKind, ParseError};
use nom::Offset;
use nom::sequence::{pair, tuple};
use semver::Version;
use strum_macros::{Display, EnumDiscriminants};
use thiserror::Error;
use crate::loc::VersionSegLoc;
use crate::parse2::chars::upper1;
use crate::types::data::{Config, ConfigKind};

#[derive(Clone,Debug)]
pub struct Token<'a> {
    span: Input<'a>,
    token: TokenKindDef<'a>
}

#[derive(Clone, Debug, EnumDiscriminants, Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TokenKind))]
#[strum_discriminants(derive(Hash))]
enum TokenKindDef<'a> {
    Ident(Ident),
    Open(NestedBlockKind),
    Close(NestedBlockKind),
    /// `+` symbol
    Plus,
    /// `@` symbol
    At,
    /// `:` symbol
    Colon,
    /// `::` symbol
    Scope,
    /// `+::` add variant 
    Variant,
    /// `.` symbol (used for properties and child defs)
    Dot,
    /// `version=` i.e.: Def(`version=`1.1.5) ... tells which parser version to use
    VersionPrelude,
    /// an erroneous token...
    Err(TokenErr<'a>)
    
}
#[derive(Clone, Debug, EnumDiscriminants, Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(IdentKind))]
#[strum_discriminants(derive(Hash))]
pub(crate) enum Ident {
    Camel(CamelCase),
    Skewer(SkewerCase),
    Snake(SnakeCase),
    Domain(Domain),
    Version(Version),
    /// represents a semi plausible ident ... maybe camel case with underscores & dashes 
    Err(String),
}

impl From<Input> for Ident {
    fn from(value: Input) -> Self {
        Self::Err(value.to_string())
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
    fn from(value: Version ) -> Self {
        Ident::Version(value)
    }
}


#[derive(Clone,Builder)]
pub struct DocTokenized<'a> {
    pub kind: Token<'a>,
    pub version: Token<'a>,
    pub defs: HashMap<TokenKindDef<'a>, Vec<Token<'a>>>,
}


fn tokenize(input: Input) -> Res<DocTokenized> {
    tuple( (multispace1,ident))
}

pub mod err {
    use crate::parse2::Input;
    use strum_macros::Display;
    use thiserror::Error;

    #[derive(Clone,Display,Debug,Error)]
    pub enum TokenErr<'a>
    {
        Expect{
           expected: &'static str, 
           found: Input<'a> 
        }
    }

    
    impl <'a> TokenErr<'a> {
        pub fn expected(expected: &'static str, found: Input<'a> ) -> Self {
            Self::Expect {
                expected,
                found
            }
        }
    }
}
