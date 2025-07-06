use crate::parse::util::{preceded, Span};
use ariadne::{Label, Report, ReportKind, Source};
use cliclack::input;
use nom::character::complete::alpha1;
use nom::combinator::all_consuming;
use nom::error::{
    convert_error, ErrorKind, FromExternalError, ParseError, VerboseError, VerboseErrorKind,
};
use nom::multi::separated_list1;
use nom::sequence::pair;
use nom::{Compare, Finish, IResult, InputLength, InputTake, Offset, Parser};
use nom_locate::LocatedSpan;
use nom_supreme::context::ContextError;
use nom_supreme::parser_ext::ParserExt;
use std::fmt::{Debug, Formatter};
use std::ops::Range;
use nom::bytes::complete::tag;
use strum_macros::{Display, EnumString};
//use nom_supreme::tag::complete::tag;
use nom_supreme::context;
use nom_supreme::final_parser::ExtractContext;
use nom_supreme::tag::TagError;
use thiserror::Error;
use starlane_macros::{push_ctx_for_input};

type Input<'a> = LocatedSpan<&'a str, ParseOpRef<'a>>;

/// `op` is a helpful name of this parse operation i.e. `BindConf`,`PackConf` ...
pub fn new(op: impl ToString, data: &str) -> ParseOp {
    ParseOp::new(op.to_string(), data)
}

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
    fn slice(&self, range: Range<usize>) -> Self {
        let stack = &self.stack[range];
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

struct ParseErrs<'a> {
    pub errors: Vec<(Input<'a>, ErrKind)>,
}

impl Debug for ParseErrs<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (input, error) in &self.errors {
            write!(f, "{} in {}", error, input)?;
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Error)]
pub enum ErrKind {
    #[error("unexpected: nom::ErrorKind lacks 'Display'")]
    Nom(ErrorKind),
    #[error("not expecting: '{0}'")]
    Char(char),
    #[error("{0}")]
    Context(Ctx),
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, EnumString, Error)]
pub enum Ctx {
    #[error("Yuk")]
    Yuk,
    #[error("parsing expected '{ctx}' ")]
    Expected {
        ctx: &'static str,
        expected: &'static str,
        found: &'static str,
    },
}
impl<'a> ParseError<Input<'a>> for ParseErrs<'a> {
    fn from_error_kind(input: Input<'a>, kind: ErrorKind) -> Self {
        Self {
            errors: vec![(input, ErrKind::Nom(kind))],
        }
    }

    fn append(input: Input<'a>, kind: ErrorKind, mut other: Self) -> Self {
        other.errors.push((input, ErrKind::Nom(kind)));
        other
    }

    fn from_char(input: Input<'a>, c: char) -> Self {
        Self {
            errors: vec![(input, ErrKind::Char(c))],
        }
    }
}
impl<'a> ContextError<Input<'a>, Ctx> for ParseErrs<'a> {
    fn add_context(input: Input<'a>, err: Ctx, mut other: Self) -> Self {
        other.errors.push((input, ErrKind::Context(err)));
        other
    }
}

impl<'a> TagError<Input<'a>, Ctx> for ParseErrs<'a> {
    fn from_tag(input: Input<'a>, tag: Ctx) -> Self {
        todo!()
    }
}

pub fn expect<O>(
    f: impl FnMut(Input) -> Res<O>+Copy,
    ctx: &'static str,
    expected: &'static str,
    found: &'static str,
) -> impl FnMut(Input) -> Res<O>+Copy {
    move |input| {
        f.context(Ctx::Expected {
            ctx,
            expected,
            found,
        })
        .parse(input)
    }
}




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

pub fn x_seg(i: Input) -> Res<Vec<Input>> {
    expect(segments, "ctx", "expected", "found" )(i).finish()
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
    let op = new("test", "you:are:^awesome");
    let input = op.input();
    let error = all_consuming(segments)(input).finish().unwrap_err();
    log(error);
}

fn log(err: ParseErrs) {
    for (input, err) in err.errors {
        Report::build(ReportKind::Error, 0..input.extra.data().len())
            .with_message(err.to_string())
            .with_label(
                Label::new(input.location_offset()..input.len())
                    .with_message("This is of type Nat"),
            )
            .finish()
            .print(Source::from(input.extra.data()))
            .unwrap();
    }
}
