use std::fmt::Display;
use crate::parse::util::Span;
use crate::parse::{camel_case, lex_block, CamelCase, NomErr, Res};
use crate::types::class::Class;
use crate::types::private::{Generic, Parsers};
use crate::types::{Type, Schema};
use futures::FutureExt;
use nom::branch::alt;
use nom::combinator::{into, opt};
use nom::sequence::{delimited, pair};
use nom::Parser;
use nom_supreme::tag::complete::tag;
use nom_supreme::ParserExt;
use starlane_space::parse::from_camel;
use std::str::FromStr;
use nom::error::FromExternalError;
use once_cell::sync::Lazy;
use crate::parse::model::{BlockKind, NestedBlockKind};

pub static NESTED_BLOCKS_DEFAULT: Lazy<Option<NestedBlockKind>> =
    Lazy::new(|| None);

pub trait TypeParser: Display+Sized {
    /// an outer parser will unwrap the root nested block and pass to [TypeParser::inner]
    /// i.e.`<Database>` become `Database`  For implementation that don't have the
    /// concept of an outer block [TypeParser::outer] simply proxies to [TypeParser::inner]
    fn outer<I>(input: I) -> Res<I,Self> where I: Span {
        Self::inner(input)
    }
    fn inner<I>(input: I) -> Res<I,Self> where I: Span;


    fn block() -> &'static Option<NestedBlockKind> {
        &*NESTED_BLOCKS_DEFAULT
    }
}


pub mod case {}

/// scan `opt(f) -> Option<D>`  then [Option::unwrap_or_default]  to generate a [D::default] value
///
pub fn opt_def<I, F, D>(f: F) -> impl Fn(I) -> Res<I, D>
where
    I: Span,
    F: FnMut(I) -> Res<I, D> + Copy,
    D: Default,
{
    move |input| opt(f)(input).map(|(next, opt)| (next, opt.unwrap_or_default()))
}

fn kind<K: Generic, I: Span>(input: I) -> Res<I, K>
where
    K: Generic + From<CamelCase>,
{
    from_camel(input)
}








