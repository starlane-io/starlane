mod chars;
mod err;
mod primitive;
mod scaffold;
mod token;

use crate::parse2::token::{Token, TokenKind};
use anyhow::__private::kind::AdhocKind;
use ariadne::{Label, Report, ReportKind, Source};
use ascii::AsciiStr;
use itertools::Itertools;
use nom::error::{ErrorKind, FromExternalError, ParseError};
use nom::{Compare, Finish, IResult, InputLength, InputTake, Offset, Parser};
use nom_locate::LocatedSpan;
use nom_supreme::context::ContextError;
use nom_supreme::error::{BaseErrorKind, GenericErrorTree};
use nom_supreme::final_parser::ExtractContext;
use nom_supreme::parser_ext::ParserExt;
use nom_supreme::tag::TagError;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Range};
use strum_macros::{Display, EnumString};
use thiserror::Error;

type Input<'a> = LocatedSpan<&'a str, ParseOpRef<'a>>;

/*
/// a wrapper for [LocatedSpan] with some convenience methods like [Self::range]
#[derive( Debug, Clone )]
pub struct Input<'a> {
    span: Span<'a>,
}

 */

pub fn range<'a>(input: &'a Input<'a>) -> Range<usize> {
    let offset = Input::location_offset(input);
    let len = Input::fragment(input).len();
    offset..(offset + len)
}

/// `op` is a helpful name of this parse operation i.e. `BindConf`,`PackConf` ...
pub fn parse_operation(op: impl ToString, data: &str) -> ParseOp {
    ParseOp::new(op.to_string(), data)
}

pub type ErrTree<'a> =
    GenericErrorTree<Input<'a>, &'static str, Ctx, Box<dyn Error + Send + Sync + 'static>>;

fn to_err(input: Input, message: impl ToString) -> nom::Err<ErrTree> {
    nom::Err::Failure(ErrTree::from_external_error(
        input,
        ErrorKind::Alpha,
        message.to_string(),
    ))
}

pub type Res<'a, O> = IResult<Input<'a>, O, ErrTree<'a>>;
pub type TokenRes<'a> = Res<'a, Token<'a>>;

pub type ParseErrsTree<'a> = ParseErrsDef<&'a str, ErrTree<'a>>;
pub type ParseErrs<'a> = ParseErrsDef<&'a str, Vec<UnitErrDef<Input<'a>, ErrKind>>>;
pub type ParseErrsOwned = ParseErrsDef<String, Vec<UnitErrDef<Range<usize>, ErrKind>>>;

#[derive(Debug, Clone)]
pub struct ParseErrsDef<D, E> {
    data: D,
    errors: E,
}

impl<D, E> ParseErrsDef<D, E>
where
    E: Default,
{
    pub fn new(data: D) -> Self {
        Self {
            data,
            errors: Default::default(),
        }
    }
}

impl<D, E> Display for ParseErrsDef<D, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl<'a, D, K> From<UnitErrDef<D, K>> for ErrRange
where
    D: Debug + Clone + Into<Range<usize>>,
    K: Into<ErrKind> + Debug + Clone,
{
    fn from(err: UnitErrDef<D, K>) -> Self {
        Self {
            range: err.span.into(),
            kind: err.kind.into(),
        }
    }
}

impl<'a> From<ParseErrs<'a>> for ParseErrsOwned {
    fn from(errs: ParseErrs<'a>) -> Self {
        let data = errs.data.into();
        let errors = errs
            .errors
            .into_iter()
            .map(|e| UnitErrDef {
                span: range(&e.span),
                kind: e.kind,
            })
            .collect_vec();
        Self { data, errors }
    }
}

pub type TokenErrKindRef<'a> = TokenErrKindDef<Input<'a>>;
pub type TokenErrKind = TokenErrKindDef<ErrRange>;

#[derive(Error, Debug, Clone)]
pub enum TokenErrKindDef<S>
where
    S: Debug + Clone,
{
    #[error("Illegal cast. '{from}' cannot be cast into: '{to}'")]
    IllegalCast { from: TokenKind, to: TokenKind },
    #[error("PhantomData to make generics work on TokenErrsDef<S> Enum. nothing to see here... ")]
    _Phantom(PhantomData<S>),
}

pub type UnitErrRef<'a> = UnitErrDef<Input<'a>, TokenErrKindRef<'a>>;

#[derive(Debug, Clone)]
pub struct UnitErrDef<I, K> {
    span: I,
    kind: K,
}

#[derive(Debug, Clone)]
pub struct ErrRangeDef<K>
where
    K: Debug + Clone,
{
    pub range: Range<usize>,
    pub kind: K,
}

impl<K> ErrRangeDef<K>
where
    K: Debug + Clone,
{
    pub fn new(range: Range<usize>, kind: K) -> Self {
        Self { range, kind }
    }
}

pub type ErrRange = ErrRangeDef<ErrKind>;

pub type TokenErr<'a> = UnitErrDef<Input<'a>, TokenErrKindRef<'a>>;

impl<'a> Deref for ParseErrsTree<'a> {
    type Target = ErrTree<'a>;

    fn deref(&self) -> &Self::Target {
        &self.errors
    }
}

impl<'a> DerefMut for ParseErrsTree<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.errors
    }
}

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
    #[strum(to_string = "upper case alphabetic character")]
    UpperCaseChar,
    #[strum(to_string = "lower case alphabetic character")]
    LowerCaseChar,
    #[strum(to_string = "Camel Case")]
    CamelCase,
    #[strum(to_string = "Skewer Case")]
    SkewerCase,
    #[strum(to_string = "Snake Case")]
    SnakeCase,
    #[strum(to_string = "Type")]
    Type,
    #[strum(to_string = "Class")]
    Class,
    #[strum(to_string = "Data")]
    Data,
}

impl<'a, E> ParseError<Input<'a>> for ParseErrsDef<Input<'a>, E>
where
    E: ParseError<Input<'a>>,
{
    fn from_error_kind(data: Input<'a>, kind: ErrorKind) -> Self {
        Self {
            errors: E::from_error_kind(data.clone(), kind),
            data,
        }
    }

    fn append(input: Input<'a>, kind: ErrorKind, mut other: Self) -> Self {
        let errors = E::append(input, kind, other.errors);
        other.errors = errors;
        other
    }

    fn from_char(data: Input<'a>, c: char) -> Self {
        let errors = E::from_char(data.clone(), c);
        Self { errors, data }
    }
}

impl<'a, E> ContextError<Input<'a>, Ctx> for ParseErrsDef<Input<'a>, E>
where
    E: ContextError<Input<'a>, Ctx>,
{
    fn add_context(location: Input<'a>, ctx: Ctx, other: Self) -> Self {
        let errors = E::add_context(location.clone(), ctx, other.errors);
        Self {
            data: location,
            errors,
        }
    }
}

impl<'a, E> TagError<Input<'a>, Ctx> for ParseErrsDef<Input<'a>, E>
where
    E: TagError<Input<'a>, Ctx>,
{
    fn from_tag(data: Input<'a>, tag: Ctx) -> Self {
        let errors = E::from_tag(data.clone(), tag);
        Self { data, errors }
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
    /*
    let mut parser = pair(
        separated_list1(tag(":"), alpha1::<Input, ParseErrs>),
        preceded(tag("^"), alpha1),
    );

    parser
        .parse(i)
        .map(|(next, (segments, extra))| (next, segments))

     */
    todo!()
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

/*
#[test]
fn test() {
    let op = parse_operation("test", "you:are:^awesome");
    let input = op.input();
    let error = all_consuming(segments)(input).finish().unwrap_err();
    log(op.data,error);
}

 */

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
