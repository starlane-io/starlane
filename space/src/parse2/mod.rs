mod chars;
mod primitive;
mod scaffold;
mod token;
mod err;

use std::error::Error;
use crate::parse::util::{preceded, Span};
use ariadne::{Label, Report, ReportKind, Source};
use nom::character::complete::alpha1;
use nom::combinator::all_consuming;
use nom::error::{
    ErrorKind, FromExternalError, ParseError,
};
use nom::multi::separated_list1;
use nom::sequence::pair;
use nom::{Compare, Finish, IResult, InputLength, InputTake, Offset, Parser};
use nom_locate::LocatedSpan;
use nom_supreme::context::ContextError;
use nom_supreme::parser_ext::ParserExt;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Range;
use nom::bytes::complete::tag;
use strum_macros::{Display, EnumString};
use nom_supreme::final_parser::ExtractContext;
use nom_supreme::tag::TagError;
use thiserror::Error;
use nom_supreme::error::{ErrorTree, BaseErrorKind, Expectation, GenericErrorTree};
type Input<'a> = LocatedSpan<&'a str, ParseOpRef<'a>>;

pub fn range(input: &Input) -> Range<usize> {
    input.location_offset()..input.fragment().len() 
}


/// `op` is a helpful name of this parse operation i.e. `BindConf`,`PackConf` ...
pub fn parse_operation(op: impl ToString, data: &str) -> ParseOp {
    ParseOp::new(op.to_string(), data)
}


pub type ParseErrs<'a> = GenericErrorTree<Input<'a>, &'static str, Ctx, Box<dyn Error + Send + Sync + 'static>>;
pub type Res<'a, O> = IResult<Input<'a>, O, ParseErrs<'a>>;

pub trait Operation {}
pub struct ParseOperationDef<'a, N, S> {
    name: N,
    stack: S,
    data: &'a str,
}

impl<'a, N, S> ParseOperationDef<'a, N, S> {
    fn from(name: N, stack: S, data: &'a str) -> Self {
        Self { name, stack, data }
    }
    pub fn data(&self) -> &'a str {
        self.data
    }
}

impl<'a> Debug for ParseOpRef<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ParseOp: '{}' data.len(): {}  ",
            self.name,
            self.data.len()
        )?;
        Ok(())
    }
}

pub type ParseOp<'a> = ParseOperationDef<'a, String, Vec<Ctx>>;

impl<'a> ParseOp<'a> {
    fn new(name: String, data: &'a str) -> Self {
        Self::from(name, Default::default(), data)
    }
    fn ctx_range(&self, range: Range<usize>) -> &[Ctx] {
        let blah = self.stack.as_slice();
        &blah[range]
    }

    fn push(&mut self, ctx: Ctx) {
        self.stack.push(ctx);
    }

    fn to_ref(&'a self) -> ParseOpRef<'a> {
        let name: &'a str = self.name.as_str();
        let stack: &'a [Ctx] = self.stack.as_slice();
        ParseOpRef::from(name, stack, self.data)
    }

    fn input(&'a self) -> Input<'a> {
        Input::new_extra(self.data, self.to_ref())
    }
}

pub type ParseOpRef<'a> = ParseOperationDef<'a, &'a str, &'a [Ctx]>;
impl<'a> ParseOpRef<'a> {
    /// returning [Self] from [Range] instead of [Result<Self,()>]
    /// goes against `rust's` principles, however, since this `unsafe`
    /// this code is only used by this parsing mod and I think it will
    /// become robust over time.  Since the [ParseOpRef] is integral
    /// in managing parse errors it's a little hard to do proper error
    /// [Result] on the error system!
    fn slice(& self, range: Range<usize>) -> Self {
        let stack = & self.stack[range];
        Self {
            name: self.name,
            stack,
            data: self.data,
        }
    }
}

impl<'a> Clone for ParseOpRef<'a> {
    fn clone(&self) -> Self {
        Self::from(self.name, self.stack, self.data)
    }
}

struct ParseErrsFinal {
    pub string: String,
    pub errors: Vec<(Range<usize>, ErrKind)>,
}

/*
struct ParseErrs {
    pub errors: Vec<(Range<usize>, ErrKind)>,
}

impl Default for ParseErrs {
    fn default() -> Self {
        Self { errors: Default::default() }
    }
}

impl ParseErrs {
    pub fn panic(m: impl ToString) -> Self {
        Self { errors: vec![(0..1,ErrKind::Panic(m.to_string()))] }
    }
}

impl Debug for ParseErrs {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (input, error) in &self.errors {
            write!(f, "{} in {}..{}", error, input.start, input.end)?;
        }
        Ok(())
    }
}

 */

#[derive(Debug, Eq, PartialEq, Hash, Clone, Error)]
pub enum ErrKind {
    #[error("Tag({0})")]
    Tag(&'static str),
    #[error("ErrKind")]
    Nom(ErrorKind),
    #[error("not expecting: '{0}'")]
    Char(char),
    #[error("{0}")]
    Context(Ctx),
    #[error("{0}")]
    Panic(String),
}

pub struct ErrorKindWrap(ErrKind);

impl Display for ErrorKindWrap {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, EnumString, Display)]
pub enum Ctx {
    Token,
    #[strum(to_string="upper case alphabetic character")]
    UpperCaseChar,
    #[strum(to_string="lower case alphabetic character")]
    LowerCaseChar,   
    #[strum(to_string="Camel Case")]
    CamelCase,
    #[strum(to_string="Skewer Case")]
    SkewerCase,
    #[strum(to_string="Snake Case")]
    SnakeCase,
    #[strum(to_string="Type")]
    Type,
    #[strum(to_string="Class")]
    Class,
    #[strum(to_string="Data")]
    Data 
}

/*
impl<'a> ParseError<Input<'a>> for ParseErrs {
    fn from_error_kind(input: Input<'a>, kind: ErrorKind) -> Self {
        Self {
            errors: vec![(range(input), ErrKind::Nom(kind))],
        }
    }

    fn append(input: Input<'a>, kind: ErrorKind, mut other: Self) -> Self {
        other.errors.push((range(input), ErrKind::Nom(kind)));
        other
    }

    fn from_char(input: Input<'a>, c: char) -> Self {
        Self {
            errors: vec![(range(input), ErrKind::Char(c))],
        }
    }
}

 */
/*
impl<'a> ContextError<Input<'a>, Ctx> for ParseErrs {
    fn add_context(input: Input<'a>, ctx: Ctx, mut other: Self) -> Self {
        other.errors.push((range(input), ErrKind::Context(ctx)));
        other
    }
}

impl <'a> TagError<Input<'a>, &'static str> for ParseErrs {
    fn from_tag(input: Input, tag: &'static str) -> Self {
        let mut errs = ParseErrs::default();
        errs.errors.push((range(input), ErrKind::Tag(tag)));
        errs
    }
}

 */

/*
fn segments(ix : Input) -> Res < Vec < Input > >
{
    let mut f = move | ix2| 
        {
            let mut parser =
                pair(separated_list1(tag(":"), alpha1 :: < Input, ParseErrs >),
                     preceded(tag("^").context(Ctx :: Yuk), alpha1));
            
            parser.parse(ix2).map(| (next, (segments, extra)) | (next, segments))
        };
    
    expect(f , "segment", "x", "y") (ix)
}

 */


pub fn segments(i: Input) -> Res<Vec<Input>> {
    let mut parser = pair(
        separated_list1(tag(":"), alpha1::<Input, ParseErrs>),
        preceded(tag("^"), alpha1),
    );

    parser
        .parse(i)
        .map(|(next, (segments, extra))| (next, segments))
}

pub fn camel(input: Input) -> Res<String> {
    chars::camel(input).map(|(next,rtn)|(next, rtn.to_string()))
}


/*
#[push_ctx_for_input]

pub fn segments(i: Input) -> Res<Vec<Input>> {
    let mut parser = pair(
        separated_list1(tag(":"), alpha1::<Input, ParseErrs>),
        preceded(tag("^").context(Ctx::Yuk), alpha1),
    );

    parser
        .parse(i)
        .map(|(next, (segments, extra))| (next, segments))
}

 */



#[test]
fn test() {
    let op = parse_operation("test", "you:are:^awesome");
    let input = op.input();
    let error = all_consuming(segments)(input).finish().unwrap_err();
    log(op.data,error);
}

fn log(data: impl AsRef<str>, err: ParseErrs) {
    match &err {
        ParseErrs::Base { location, ref kind } => {
            
            let range = range(&location);

            let mut builder = Report::build(ReportKind::Error, range.clone());
            match kind {
                BaseErrorKind::Expected(expect) => {
                    let report = builder.with_message(format!("Expected: '{}' found: {}", expect, location)).with_label(
                        Label::new(range)
                            .with_message(format!("{}",err)),
                    ).finish();
                    report.print(Source::from(data.as_ref())).ok();
                }
                BaseErrorKind::Kind(kind) => {}
                BaseErrorKind::External(external) => {}
            }
        }
        ParseErrs::Stack { base, contexts } => {
            panic!();
        }
        ParseErrs::Alt(_) => {

            panic!();
        }
    }
    todo!()
    /*
    for (range, err) in err.errors {
        Report::build(ReportKind::Error, range.clone())
            .with_message(err.to_string())
            .with_label(
                Label::new(range)
                    .with_message(format!("{}",err)),
            )
            .finish()
            .print(Source::from(data.as_ref()))
            .unwrap();
    }
    
     */
}
