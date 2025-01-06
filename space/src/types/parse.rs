use crate::parse::util::Span;
use crate::parse::{camel_case, CamelCase, NomErr, Res};
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

pub fn block_delimiters<I>(
    (open, close): (&'static str, &'static str),
) -> (impl Fn(I) -> Res<I, I>, impl Fn(I) -> Res<I, I>)
where
    I: Span,
{
    (tag(open), tag(close))
}

pub fn angle_block_delimiters<I>() -> (impl Fn(I) -> Res<I, I>, impl Fn(I) -> Res<I, I>)
where
    I: Span,
{
    block_delimiters(("<", ">"))
}

pub fn square_block_delimiters<I>() -> (impl Fn(I) -> Res<I, I>, impl Fn(I) -> Res<I, I>)
where
    I: Span,
{
    block_delimiters(("[", "]"))
}

pub fn angle_block<I, F, O>(f: F) -> impl FnMut(I) -> Res<I, O>
where
    F: FnMut(I) -> Res<I, O>+Copy,
    I: Span,
{
    let (outer, inner) = angle_block_delimiters();
    delimited(outer, f, inner)
}

pub fn square_block<I, F, O>(f: F) -> impl FnMut(I) -> Res<I, O>
where
    F: FnMut(I) -> Res<I, O>+Copy,
    I: Span,
{
    let (outer, inner) = square_block_delimiters();
    delimited(outer, f, inner)
}



pub trait ParsePrimitive {
    type Output;
    fn parse<I>(input: I) -> Res<Self::Output, NomErr<I>> where I: Span;
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

pub fn type_kind<I>(input: I) -> Res<I, Abstract>
where
    I: Span,
{
    alt((into(schema), into(class)))(input)
    //alt((map(schema_kind,TypeKind::from),map(class_kind,TypeKind::from) ))
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

pub fn class<I: Span>(input: I) -> Res<I, Class> {
    let (next, (base, variant)) = pair(camel_case, opt(angle_block(camel_case)))(input.clone())?;
    let class = match variant {
        None => From::from(base),
        Some(variant) => {
            Class::from_variant(base, variant).map_err(|e| nom::Err::Failure(e.to_nom(input)))?
        }
    };

    Ok((next, class))
}

pub fn schema<I: Span>(input: I) -> Res<I, Schema> {
    from_camel(input)
}

pub mod delim {
    use crate::parse::util::Span;
    use crate::parse::Res;
    use crate::types::private::Generic;
    use nom::sequence::delimited;
    use nom_supreme::tag::complete::tag;
    pub fn angles<I, F, O>(f: F) -> impl FnMut(I) -> Res<I, O>
    where
        I: Span,
        F: FnMut(I) -> Res<I, O> + Copy,
    {
        delimited(tag("<"), f, tag(">"))
    }




    /*
        #[test]
        pub fn test_delim() {
            use nom::combinator::all_consuming;
            use crate::parse::util::{new_span, result};
            use crate::types::class::Class;
            use crate::types::parse::class;
            use crate::types::parse::delim::delim;


            //let i = new_span("<Database>");
            let i = new_span("Database");
            let c = result(class(i)).unwrap();
            assert_eq!(Class::Database,c)
            //let c = result(delim(class)(i)).unwrap();
    //        assert_eq!(Class::Database,c)
        }

         */
}
