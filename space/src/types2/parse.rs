use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::Span;
use crate::parse::{lex_block, CamelCase, Res};
use crate::types::archetype::Archetype;
use crate::types::class::Class;
use crate::types::data::DataType;
use crate::types::{Type, TypeDisc};
use futures::FutureExt;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{into, opt, value};
use nom::sequence::delimited;
use nom::Parser;
use nom_supreme::ParserExt;
use starlane_space::parse::from_camel;
use std::fmt::Display;
use std::str::FromStr;

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

fn kind<K: Archetype, I: Span>(input: I) -> Res<I, K>
where
    K: Archetype + From<CamelCase>,
{
    from_camel(input)
}

pub fn type_kind<I>(input: I) -> Res<I, Type>
where
    I: Span,
{
    alt((into(data), into(class)))(input)
    //alt((map(schema_kind,TypeKind::from),map(class_kind,TypeKind::from) ))
}

/*
fn into<I,O>((input,kind):(I,impl Into<O>)) -> (I,O) {
    (input.into(),kind.into())
}

 */
pub fn identify_abstract_disc<I>(input: I) -> Res<I, TypeDisc>
where
    I: Span,
{
    alt((
        value(
            TypeDisc::Class,
            lex_block(BlockKind::Nested(NestedBlockKind::Angle)),
        ),
        value(
            TypeDisc::Data,
            lex_block(BlockKind::Nested(NestedBlockKind::Square)),
        ),
    ))(input)
}

pub fn unwrap_abstract<I>(input: I) -> Res<I, Type>
where
    I: Span,
{
    let (next, r#abstract) = identify_abstract_disc(input.clone())?;
    match r#abstract {
        TypeDisc::Class => into(Class::delimited_parser(Class::parser))(input),
        TypeDisc::Data => into(DataType::delimited_parser(DataType::parser))(input),
    }
}

pub fn class<I: Span>(input: I) -> Res<I, Class> {
    from_camel(input)
}

pub fn data<I: Span>(input: I) -> Res<I, DataType> {
    from_camel(input)
}

pub mod delim {
    use crate::parse::util::{new_span, result, Span};
    use crate::parse::{from_camel, CamelCase, Res};
    use crate::types::class::Class;
    use crate::types::data::DataType;
    use crate::types::parse::Delimited;
    use crate::types::parse::{class, data};
    use nom::sequence::delimited;
    use nom_supreme::tag::complete::tag;
    use std::str::FromStr;

    pub fn delim<I, F, O>(f: F) -> impl FnMut(I) -> Res<I, O>
    where
        I: Span,
        F: FnMut(I) -> Res<I, O> + Copy,
        O: Delimited,
    {
        fn tags<I>(
            (open, close): (&'static str, &'static str),
        ) -> (impl Fn(I) -> Res<I, I>, impl Fn(I) -> Res<I, I>)
        where
            I: Span,
        {
            (tag(open), tag(close))
        }

        let (open, close) = tags(O::delimiters());
        delimited(open, f, close)
    }

    #[test]
    pub fn test_from_camel() {
        #[derive(Eq, PartialEq, Debug)]
        struct Blah(CamelCase);

        impl From<CamelCase> for Blah {
            fn from(camel: CamelCase) -> Self {
                Blah(camel)
            }
        }

        let s = "MyCamelCase";
        let i = new_span(s);
        let blah: Blah = result(from_camel(i)).unwrap();
        assert_eq!(blah.0.as_str(), s);
    }

    #[test]
    pub fn test_class() {
        let s = "<Database>";
        let i = new_span(s);
        let database = result(delim(class)(i)).unwrap();
        assert_eq!(database, Class::Database);
    }

    #[test]
    pub fn test_schema() {
        let s = "[Text]";
        let i = new_span(s);
        let text = result(delim(data)(i)).unwrap();
        assert_eq!(text, DataType::Text);
    }

    #[test]
    pub fn class_from_camel() {
        let camel = CamelCase::from_str("Database").unwrap();
        let class = Class::from(camel);

        assert_eq!(class, Class::Database);
    }

    #[test]
    pub fn test_class_ext() {
        /// test [Class:_Ext]
        let camel = CamelCase::from_str("Zophis").unwrap();
        let class = Class::from(camel.clone());

        assert_eq!(class, Class::_Ext(camel));
    }

    #[test]
    pub fn test_from_variant() {
        let camel = CamelCase::from_str("Database").unwrap();
        let class = Class::from(camel);
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
#[cfg(test)]
pub mod test {
    #[test]
    pub fn delimit() {}
}

pub trait Delimited: Archetype + Sized {
    fn delimiters() -> (&'static str, &'static str);

    fn delimited_parser<I, O>(f: impl FnMut(I) -> Res<I, O>) -> impl FnMut(I) -> Res<I, O>
    where
        I: Span,
    {
        delimited(tag(Self::delimiters().0), f, tag(Self::delimiters().1))
    }
}
