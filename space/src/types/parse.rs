use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::Span;
use crate::parse::{lex_block, CamelCase, Res};
use crate::types::class::Class;
use crate::types::private::{Delimited, Parsable};
use crate::types::{Abstract, AbstractDisc, Data};
use futures::FutureExt;
use nom::branch::alt;
use nom::combinator::{into, opt, value};
use nom::sequence::delimited;
use nom::Parser;
use nom_supreme::tag::complete::tag;
use nom_supreme::ParserExt;
use starlane_space::parse::from_camel;
use std::str::FromStr;

pub mod case {

}


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

fn kind<K: Parsable, I: Span>(input: I) -> Res<I, K>
where
    K: Parsable + From<CamelCase>,
{
    from_camel(input)
}

pub fn type_kind<I>(input: I) -> Res<I, Abstract>
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
pub fn identify_abstract_disc<I>(input:I) -> Res<I,AbstractDisc> where I: Span {
  alt((value( AbstractDisc::Class, lex_block( BlockKind::Nested(NestedBlockKind::Angle))),value( AbstractDisc::Data, lex_block( BlockKind::Nested(NestedBlockKind::Square)))))(input)
}



pub fn unwrap_abstract<I>(input: I) -> Res<I, Abstract> where I: Span {
    let (next,r#abstract) = identify_abstract_disc(input.clone())?;
    match r#abstract {
        AbstractDisc::Class => into(Class::delimited_parser(Class::parser))(input),
        AbstractDisc::Data => into(Data::delimited_parser(Data::parser))(input)
    }
}


pub fn class<I: Span>(input: I) -> Res<I, Class> {
    from_camel(input)
}

pub fn data<I: Span>(input: I) -> Res<I, Data> {
    from_camel(input)
}


pub mod delim {
    use crate::parse::util::{new_span, result, Span};
    use crate::parse::{from_camel, CamelCase, Res};
    use crate::types::class::Class;
    use crate::types::parse::{class, data};
    use crate::types::Data;
    use nom::sequence::delimited;
    use nom_supreme::tag::complete::tag;
    use starlane_space::types::private::Delimited;
    use std::str::FromStr;

    pub fn delim<I, F, O>(f: F) -> impl FnMut(I) -> Res<I, O>
    where
        I: Span,
        F: FnMut(I) -> Res<I, O> + Copy,
        O: Delimited
    {
        fn tags<I>(
            (open, close): (&'static str, &'static str),
        ) -> (impl Fn(I) -> Res<I, I>, impl Fn(I) -> Res<I, I>) where I: Span{
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
        let blah: Blah  = result(from_camel(i)).unwrap();
        assert_eq!(blah.0.as_str(), s);
    }

    #[test]
    pub fn test_class() {
        let s = "<Database>";
        let i = new_span(s);
        let database= result(delim(class)(i)).unwrap();
        assert_eq!(database, Class::Database);
    }

    #[test]
    pub fn test_schema() {
        let s = "[Text]";
        let i = new_span(s);
        let text = result(delim(data)(i)).unwrap();
        assert_eq!(text, Data::Text);
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
