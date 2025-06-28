pub mod prim;
use chumsky::Parser;
use chumsky::prelude::end;
use crate::chumsky::prim::CamelCaseCap;

pub enum Exp {
    Type(CamelCaseCap),
}

fn parser<'src>() -> impl Parser<'src, &'src str, ()> {
    end() // --(5)
}

#[test]
fn test_parser() {
    // Our parser expects empty strings, so this should parse successfully
    assert_eq!(parser().parse("").into_result(), Ok(()));

    // Anything other than an empty string should produce an error
    assert!(parser().parse("123").has_errors());
}