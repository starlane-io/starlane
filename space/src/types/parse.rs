use crate::parse::util::Span;
use crate::parse::{camel_case, lex_block, CamelCase, NomErr, Res};
use crate::types::class::Class;
use crate::types::private::{Generic, Parsers};
use crate::types::{Abstract, Schema};
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
use crate::parse::model::BlockKind;



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



/*
fn into<I,O>((input,kind):(I,impl Into<O>)) -> (I,O) {
    (input.into(),kind.into())
}

 */


/*
pub fn r#abstract<I, P>(input: I) -> Res<I, P::Output>
where
    P: Parsers,
    I: Span,
{

    let (next, (disc, variant)) = pair(P::discriminant, opt(P::segment))(input.clone())?;
    let output = match variant {
        None => P::Output::try_from(disc).map_err(|err|NomErr::from_external_error(input,ErrorKind::Fail,err))?,
        Some(variant) => {
            P::variant(disc,variant).map_err(|err|NomErr::from_external_error(input,ErrorKind::Fail,err))?
        }
    };


    Ok((next, output))
}

 */




