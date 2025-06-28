use std::str::FromStr;
use itertools::Itertools;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use strum_macros::Display;
use crate::parse::CamelCase;

#[derive(Parser)]
#[grammar_inline = "
upper = { 'A'..'Z' }
lower = { 'a'..'z' }
alphabetic = { upper | lower  }
alphanumeric= { alphabetic | ASCII_DIGIT }

camel_case = { upper{1} ~ alphanumeric+ }
skewer_case= { lower{1} ~ lower+ }



my = { SOI ~ camel_case~ EOI }"]
struct MyParser;

#[derive(Display)]
pub enum AstNode {
   #[strum(to_string = "{0}")]
   Type(CamelCase)
}

fn from(pair: Pair<Rule>) -> AstNode {
    match pair.as_rule() {
        Rule::camel_case => AstNode::Type(CamelCase::from_str(pair.as_str()).unwrap()),
        _ => panic!()
    }
}

#[test]
fn camel_case() {
    let pairs = MyParser::parse(Rule::camel_case, "HelloMyFriend16X ora").unwrap();
    let ast = from(pairs.into_iter().next().unwrap());
    println!("AST: {}", ast);
    
}
