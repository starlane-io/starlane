use crate::parse::util::Span;
use crate::parse::{CamelCase, Res};
use crate::types::class::Class;
use crate::types::private::Generic;
use crate::types::{scope::parse::domain, Abstract, Exact, Schema};
use futures::FutureExt;
use nom::branch::alt;
use nom::combinator::{into, opt};
use nom::Parser;
use nom_supreme::ParserExt;
use starlane_space::parse::from_camel;
use std::str::FromStr;

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
    alt((into(schema_kind), into(class_kind)))(input)
    //alt((map(schema_kind,TypeKind::from),map(class_kind,TypeKind::from) ))
}

/*
fn into<I,O>((input,kind):(I,impl Into<O>)) -> (I,O) {
    (input.into(),kind.into())
}

 */

pub fn class_kind<I: Span>(input: I) -> Res<I, Class> {
    from_camel(input)
}

pub fn schema_kind<I: Span>(input: I) -> Res<I, Schema> {
    from_camel(input)
}

pub mod delim {
    use crate::parse::util::Span;
    use crate::parse::Res;
    use crate::types::private::Generic;
    use nom::sequence::delimited;
    use nom_supreme::tag::complete::tag;
    use starlane_space::types::private::Delimited;
    pub fn delim<I, F, O, D>(f: F) -> impl FnMut(I) -> Res<I, O>
    where
        I: Span,
        F: FnMut(I) -> Res<I, O> + Copy,
        D: Delimited,
    {
        fn tags<I>(
            (open, close): (&'static str, &'static str),
        ) -> (impl Fn(I) -> Res<I, I>, impl Fn(I) -> Res<I, I>) where I: Span{
            (tag(open), tag(close))
        }

        let (open, close) = tags(D::type_delimiters());
        delimited(open, f, close)
    }
}
#[cfg(test)]
pub mod test {
    use crate::parse::kind;
    use crate::parse::util::result;
    use crate::util::log;

    #[test]
    pub fn test() {}
}
