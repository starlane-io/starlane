use crate::parse2::ast::ast;
use crate::parse2::err::{ParseErrs2, ParseErrs2Def, ParseErrs2Proto};
use crate::parse2::token::{result, tokens, Tokens};
use ariadne::{Label, Report, ReportKind, Source};
use err::ErrTree;
use itertools::Itertools;
use nom::combinator::{all_consuming, eof};
use nom::error::{FromExternalError, ParseError};
use nom::sequence::terminated;
use nom::{Compare, Finish, IResult, InputLength, InputTake, Offset, Parser};
use nom_locate::LocatedSpan;
use nom_supreme::context::ContextError;
use nom_supreme::error::BaseErrorKind;
use nom_supreme::final_parser::ExtractContext;
use nom_supreme::parser_ext::ParserExt;
use nom_supreme::tag::TagError;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::ops::{Deref, DerefMut, Range};
use std::sync::Arc;
use crate::parse2::document::{Document, DocumentDef, DocumentProto};

mod chars;
mod err;
mod primitive;
mod scaffold;
mod token;
mod ast;
pub mod document;

/*
#[derive(Debug)]
pub struct Op {
    pub data: Arc<String>
}

impl Op {
    
    pub fn new(data: Arc<String>) -> Op {
        Self { data }
    }
    

    
    pub fn parse(&self) -> Result<Doc,ParseErrs2> {
        let input = Input::new_extra(self.data.as_str(),self);
        let (_,tokens) = self.tokenize(input)?;
        ast(tokens)
    }

}
 
 
 */


pub fn empty_span() -> Input<'static> {
    let stat = "";
    let source = Arc::new(stat.to_string());
    Input::new_extra(stat, & source)
}

pub fn parse<'a>(source: &'a Arc<String>) -> Result<DocumentDef<'a>, ParseErrs2Def<'a>>{
    let input = Input::new_extra(source.as_str(), & source);
    let tokens = tokenize(& source, input)?;
    let doc = ast(tokens)?;
    Ok(doc)
}

fn tokenize<'a>(source: &'a Arc<String>,input: Input<'a>) -> Result<Tokens<'a>, ParseErrs2Def<'a>> {
    let (_,tokens)= result(tokens(input))?;
    let tokens = Tokens::new( source, tokens );
    Ok(tokens)
}

type Input<'a> = LocatedSpan<&'a str, &'a Arc<String>>;



pub type Res<'a, O> = IResult<Input<'a>, O, ErrTree<'a>>;


pub fn range<'a>(input: &'a Input<'a>) -> Range<usize> {
    let offset = Input::location_offset(input);
    let len = Input::fragment(input).len();
    offset..(offset + len)
}


pub enum ParseResultDef<'a,S> {
    Ok(DocumentDef<'a,S>),
    Err(ParseErrs2Def<'a,S>)
}
pub type ParseResultProto<'a> = ParseResultDef<'a,()>;
impl <'a> ParseResultProto<'a>{
   pub fn promote( self, source: Arc<String>) -> Result<Document<'a>,ParseErrs2<'a>> {
       match self {
           ParseResultProto::Ok(ok) => Ok(ok.promote(source)),
           ParseResultProto::Err(err) => Err(err.promote(source))
       }
   }
}


fn log(data: impl AsRef<str>, err: ErrTree) {
    match &err {
        ErrTree::Base { location, ref kind } => {
            let range = range(&location);

            let mut builder = Report::build(ReportKind::Error, range.clone());
            match kind {
                BaseErrorKind::Expected(expect) => {
                    let report = builder
                        .with_message(format!("Expected: '{}' found: {}", expect, location))
                        .with_label(Label::new(range).with_message(format!("{}", err)))
                        .finish();
                    report.print(Source::from(data.as_ref())).ok();
                }
                BaseErrorKind::Kind(kind) => {
                    panic!()
                }
                BaseErrorKind::External(external) => {
                    panic!()
                }
            }
        }
        ErrTree::Stack { base, contexts } => {
            panic!("\n\nERR !STACK\n\n");
        }
        ErrTree::Alt(_) => {
            panic!("\n\nERR !ALT\n\n");
            panic!();
        }
    }
}
