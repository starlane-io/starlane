use crate::err::{ParseErrs, PrintErr};
use crate::loc::Variable;
use crate::parse::{ErrCtx, NomErr, VarCase};
use core::fmt::Display;
use nom::character::complete::multispace0;
use nom::error::{ErrorKind, ParseError};
use nom::sequence::delimited;
use nom::{AsBytes, AsChar, Compare, CompareResult, FindSubstring, IResult, InputIter, InputLength, InputTake, InputTakeAtPosition, Needed, Offset, Parser, Slice};
use nom_locate::LocatedSpan;
use nom_supreme::error::{GenericErrorTree, StackContext};
use nom_supreme::final_parser::ExtractContext;
use nom_supreme::ParserExt;
use serde_derive::{Deserialize, Serialize};
use std::ops::{Deref, Range, RangeFrom, RangeTo};
use std::sync::Arc;
use nom::branch::alt;
use nom::combinator::into;
use thiserror::__private::AsDisplay;
use crate::parse;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}

pub trait Span:
    Clone
    + ToString
    + AsBytes
    + Slice<Range<usize>>
    + Slice<RangeTo<usize>>
    + Slice<RangeFrom<usize>>
    + InputLength
    + Offset
    + InputTake
    + InputIter<Item = char>
    + InputTakeAtPosition<Item = char>
    + Compare<&'static str>
    + FindSubstring<&'static str>
    + core::fmt::Debug
where
    Self: Sized,
    <Self as InputTakeAtPosition>::Item: AsChar,
{
    fn location_offset(&self) -> usize;

    fn location_line(&self) -> u32;

    fn get_column(&self) -> usize;

    fn extra(&self) -> Arc<String>;

    fn len(&self) -> usize;

    fn range(&self) -> Range<usize>;

    fn trace(&self) -> Trace {
        Trace {
            range: self.range(),
            extra: self.extra(),
        }
    }
}

impl<'a> Span for Wrap<LocatedSpan<&'a str, Arc<String>>> {
    fn location_offset(&self) -> usize {
        self.input.location_offset()
    }

    fn get_column(&self) -> usize {
        self.input.get_column()
    }

    fn location_line(&self) -> u32 {
        self.input.location_line()
    }

    fn extra(&self) -> Arc<String> {
        self.input.extra.clone()
    }

    fn len(&self) -> usize {
        self.input.len()
    }

    fn range(&self) -> Range<usize> {
        Range {
            start: self.location_offset(),
            end: self.location_offset() + self.len(),
        }
    }
}

// TraceWrap
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Tw<W> {
    pub trace: Trace,
    pub w: W,
}

impl<W> Tw<W> {
    pub fn new<I: Span>(span: I, w: W) -> Self {
        Self {
            trace: span.trace(),
            w,
        }
    }

    pub fn unwrap(self) -> W {
        self.w
    }
}

impl<W> ToString for Tw<W>
where
    W: ToString,
{
    fn to_string(&self) -> String {
        self.w.to_string()
    }
}

impl<W> Deref for Tw<W> {
    type Target = W;

    fn deref(&self) -> &Self::Target {
        &self.w
    }
}

impl Into<Variable> for Tw<VarCase> {
    fn into(self) -> Variable {
        Variable {
            name: self.w,
            trace: self.trace,
        }
    }
}

pub fn tw<I, F, O, C, E>(mut f: F) -> impl FnMut(I) -> Res<I, Tw<O>, C, E>
where
    I: Span,
    F: FnMut(I) -> Res<I, O, C, E>,
{
    move |input: I| {
        let (next, output) = f(input.clone())?;

        let span = input.slice(0..next.len());
        let tw = Tw::new(span, output);

        Ok((next, tw))
    }
}

//pub type OwnedSpan<'a> = LocatedSpan<&'a str, SpanExtra>;
pub type SpanExtra = Arc<String>;

mod span_serde {
    use crate::parse::util::SpanExtra;
    use serde::{Deserialize, Deserializer, Serializer};
    use std::sync::Arc;

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<SpanExtra, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Arc::new(String::deserialize(deserializer)?))
    }

    pub(super) fn serialize<S>(span: &SpanExtra, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(span.as_str())
    }
}

pub fn new_span<'a>(s: &'a str) -> Wrap<LocatedSpan<&'a str, Arc<String>>> {
    let extra = Arc::new(s.to_string());
    let span = LocatedSpan::new_extra(s, extra);
    Wrap::new(span)
}

pub fn span_with_extra(s: &str, extra: Arc<String>) -> Wrap<LocatedSpan<&str, Arc<String>>> {
    Wrap::new(LocatedSpan::new_extra(s, extra))
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Trace {
    pub range: Range<usize>,

    #[serde(
        serialize_with = "span_serde::serialize",
        deserialize_with = "span_serde::deserialize"
    )]
    pub extra: SpanExtra,
}

impl Trace {
    pub fn new(range: Range<usize>, extra: SpanExtra) -> Self {
        Self { range, extra }
    }

    pub fn at_offset(offset: usize, extra: SpanExtra) -> Self {
        Self {
            range: offset..offset,
            extra,
        }
    }

    pub fn scan<F, I: Span, O, C, E>(f: F, input: I) -> Self
    where
        F: FnMut(I) -> Res<I, O, C, E> + Copy,
        E: std::error::Error + Send + Sync + 'static,
    {
        let extra = input.extra();
        let range = input.location_offset()..len(f)(input);
        Self { range, extra }
    }
}

#[derive(Debug, Clone)]
pub struct SliceStr {
    location_offset: usize,
    len: usize,
    string: Arc<String>,
}

impl ToString for SliceStr {
    fn to_string(&self) -> String {
        self.string
            .as_str()
            .slice(self.location_offset..self.location_offset + self.len)
            .to_string()
    }
}

impl SliceStr {
    pub fn new(string: String) -> Self {
        Self::from_arc(Arc::new(string))
    }

    pub fn from_arc(string: Arc<String>) -> Self {
        Self {
            len: string.len(),
            string,
            location_offset: 0,
        }
    }

    pub fn from(string: Arc<String>, location_offset: usize, len: usize) -> Self {
        Self {
            string,
            location_offset,
            len,
        }
    }
}

impl SliceStr {
    pub fn as_str(&self) -> &str {
        &self
            .string
            .as_str()
            .slice(self.location_offset..self.location_offset + self.len)
    }
}

impl Deref for SliceStr {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsBytes for SliceStr {
    fn as_bytes(&self) -> &[u8] {
        self.string
            .as_bytes()
            .slice(self.location_offset..self.location_offset + self.len)
    }
}

impl Slice<Range<usize>> for SliceStr {
    fn slice(&self, range: Range<usize>) -> Self {
        SliceStr {
            location_offset: self.location_offset + range.start,
            len: range.end - range.start,
            string: self.string.clone(),
        }
    }
}

impl Slice<RangeFrom<usize>> for SliceStr {
    fn slice(&self, range: RangeFrom<usize>) -> Self {
        SliceStr {
            location_offset: self.location_offset + range.start,
            len: self.len - range.start,
            string: self.string.clone(),
        }
    }
}

impl Slice<RangeTo<usize>> for SliceStr {
    fn slice(&self, range: RangeTo<usize>) -> Self {
        SliceStr {
            location_offset: self.location_offset,
            len: range.end,
            string: self.string.clone(),
        }
    }
}

impl Compare<&str> for SliceStr {
    fn compare(&self, t: &str) -> CompareResult {
        self.as_str().compare(t)
    }

    fn compare_no_case(&self, t: &str) -> CompareResult {
        self.as_str().compare_no_case(t)
    }
}

impl InputLength for SliceStr {
    fn input_len(&self) -> usize {
        self.len
    }
}

impl Offset for SliceStr {
    fn offset(&self, second: &Self) -> usize {
        self.location_offset
    }
}

pub struct MyCharIterator {}

pub struct MyChars {
    index: usize,
    slice: SliceStr,
}

impl MyChars {
    pub fn new(slice: SliceStr) -> Self {
        Self { index: 0, slice }
    }
}

impl Iterator for MyChars {
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        let mut chars = self.slice.as_str().chars();
        let next = chars.nth(self.index);
        match next {
            None => None,
            Some(next) => {
                self.index = self.index + 1;
                Some(next)
            }
        }
    }
}

pub struct CharIterator {
    index: usize,
    slice: SliceStr,
}

impl CharIterator {
    pub fn new(slice: SliceStr) -> Self {
        Self { index: 0, slice }
    }
}

impl Iterator for CharIterator {
    type Item = (usize, char);

    fn next(&mut self) -> Option<Self::Item> {
        let mut chars = self.slice.as_str().chars();
        let next = chars.nth(self.index);
        match next {
            None => None,
            Some(next) => {
                //let byte_index = self.index * std::mem::size_of::<char>();
                let byte_index = self.index;
                self.index = self.index + 1;
                Some((byte_index, next))
            }
        }
    }
}

impl InputIter for SliceStr {
    type Item = char;
    type Iter = CharIterator;
    type IterElem = MyChars;

    #[inline]
    fn iter_indices(&self) -> Self::Iter {
        CharIterator::new(self.clone())
    }
    #[inline]
    fn iter_elements(&self) -> Self::IterElem {
        MyChars::new(self.clone())
    }
    #[inline]
    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.as_str().position(predicate)
    }

    #[inline]
    fn slice_index(&self, count: usize) -> Result<usize, nom::Needed> {
        self.as_str().slice_index(count)
    }
}

impl InputTakeAtPosition for SliceStr {
    type Item = char;

    fn split_at_position<P, E: ParseError<Self>>(&self, predicate: P) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.split_at_position(predicate) {
            Err(nom::Err::Incomplete(_)) => Ok(self.take_split(self.input_len())),
            res => res,
        }
    }

    fn split_at_position1<P, E: ParseError<Self>>(
        &self,
        predicate: P,
        e: ErrorKind,
    ) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.as_str().position(predicate) {
            Some(0) => Err(nom::Err::Error(E::from_error_kind(self.clone(), e))),
            Some(n) => Ok(self.take_split(n)),
            None => Err(nom::Err::Incomplete(nom::Needed::new(1))),
        }
    }

    fn split_at_position_complete<P, E: ParseError<Self>>(
        &self,
        predicate: P,
    ) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.split_at_position(predicate) {
            Err(nom::Err::Incomplete(_)) => Ok(self.take_split(self.input_len())),
            res => res,
        }
    }

    fn split_at_position1_complete<P, E: ParseError<Self>>(
        &self,
        predicate: P,
        e: ErrorKind,
    ) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.as_str().position(predicate) {
            Some(0) => Err(nom::Err::Error(E::from_error_kind(self.clone(), e))),
            Some(n) => Ok(self.take_split(n)),
            None => {
                if self.as_str().input_len() == 0 {
                    Err(nom::Err::Error(E::from_error_kind(self.clone(), e)))
                } else {
                    Ok(self.take_split(self.input_len()))
                }
            }
        }
    }
}

impl InputTake for SliceStr {
    fn take(&self, count: usize) -> Self {
        self.slice(count..)
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        (self.slice(count..), self.slice(..count))
    }
}

impl FindSubstring<&str> for SliceStr {
    fn find_substring(&self, substr: &str) -> Option<usize> {
        self.as_str().find_substring(substr)
    }
}





#[cfg(test)]
pub mod test {
    use crate::parse::util::SliceStr;
    use nom::Slice;

    #[test]
    pub fn test() {
        let s = SliceStr::new("abc123".to_string());
        assert_eq!(6, s.len());

        let s = s.slice(0..3);
        assert_eq!(3, s.len());
        assert_eq!("abc", s.as_str());

        println!("bytes: {}", s.as_bytes().len());
        println!("chars: {}", s.chars().count());

        let s = SliceStr::new("abc123".to_string());
        assert_eq!("123", s.slice(3..).as_str());
        assert_eq!("abc", s.slice(..3).as_str());
    }
}

#[derive(Debug, Clone)]
pub struct Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    input: I,
}

impl<I> Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    pub fn new(input: I) -> Self {
        Self { input }
    }
}

impl<I> Deref for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.input
    }
}

impl<I> AsBytes for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn as_bytes(&self) -> &[u8] {
        self.input.as_bytes()
    }
}

impl<I> Slice<Range<usize>> for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn slice(&self, range: Range<usize>) -> Self {
        Self::new(self.input.slice(range))
    }
}

impl<I> Slice<RangeFrom<usize>> for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn slice(&self, range: RangeFrom<usize>) -> Self {
        Self::new(self.input.slice(range))
    }
}

impl<I> Slice<RangeTo<usize>> for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn slice(&self, range: RangeTo<usize>) -> Self {
        Self::new(self.input.slice(range))
    }
}

impl<'a> Compare<&'static str> for Wrap<LocatedSpan<&'a str, Arc<String>>> {
    fn compare(&self, t: &str) -> CompareResult {
        self.input.compare(t)
    }

    fn compare_no_case(&self, t: &str) -> CompareResult {
        self.input.compare_no_case(t)
    }
}

impl<I> InputLength for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn input_len(&self) -> usize {
        self.input.input_len()
    }
}

impl<I> Offset for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn offset(&self, second: &Self) -> usize {
        self.input.offset(&second.input)
    }
}

impl<I> InputIter for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    type Item = <I as InputIter>::Item;
    type Iter = <I as InputIter>::Iter;
    type IterElem = <I as InputIter>::IterElem;

    fn iter_indices(&self) -> Self::Iter {
        self.input.iter_indices()
    }

    fn iter_elements(&self) -> Self::IterElem {
        self.input.iter_elements()
    }

    fn position<P>(&self, predicate: P) -> Option<usize>
    where
        P: Fn(Self::Item) -> bool,
    {
        self.input.position(predicate)
    }

    fn slice_index(&self, count: usize) -> Result<usize, Needed> {
        self.input.slice_index(count)
    }
}

impl<I> InputTake for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn take(&self, count: usize) -> Self {
        Wrap::new(self.input.take(count))
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        let (left, right) = self.input.take_split(count);
        (Wrap::new(left), Wrap::new(right))
    }
}

impl<I> ToString for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    fn to_string(&self) -> String {
        self.input.to_string()
    }
}

impl<'a> FindSubstring<&str> for Wrap<LocatedSpan<&'a str, Arc<String>>> {
    fn find_substring(&self, substr: &str) -> Option<usize> {
        self.input.find_substring(substr)
    }
}

impl<I> InputTakeAtPosition for Wrap<I>
where
    I: Clone
        + ToString
        + AsBytes
        + Slice<Range<usize>>
        + Slice<RangeTo<usize>>
        + Slice<RangeFrom<usize>>
        + InputLength
        + Offset
        + InputTake
        + InputIter<Item = char>
        + core::fmt::Debug
        + InputTakeAtPosition<Item = char>,
{
    type Item = <I as InputIter>::Item;

    fn split_at_position<P, E: ParseError<Self>>(&self, predicate: P) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.position(predicate) {
            Some(n) => Ok(self.take_split(n)),
            None => Err(nom::Err::Incomplete(Needed::new(1))),
        }
    }

    fn split_at_position1<P, E: ParseError<Self>>(
        &self,
        predicate: P,
        e: ErrorKind,
    ) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.position(predicate) {
            Some(0) => Err(nom::Err::Error(E::from_error_kind(self.clone(), e))),
            Some(n) => Ok(self.take_split(n)),
            None => Err(nom::Err::Incomplete(Needed::new(1))),
        }
    }

    fn split_at_position_complete<P, E: ParseError<Self>>(
        &self,
        predicate: P,
    ) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.split_at_position(predicate) {
            Err(nom::Err::Incomplete(_)) => Ok(self.take_split(self.input_len())),
            res => res,
        }
    }

    fn split_at_position1_complete<P, E: ParseError<Self>>(
        &self,
        predicate: P,
        e: ErrorKind,
    ) -> IResult<Self, Self, E>
    where
        P: Fn(Self::Item) -> bool,
    {
        match self.split_at_position1(predicate, e) {
            Err(nom::Err::Incomplete(_)) => {
                if self.input_len() == 0 {
                    Err(nom::Err::Error(E::from_error_kind(self.clone(), e)))
                } else {
                    Ok(self.take_split(self.input_len()))
                }
            }
            res => res,
        }
    }
}

type Res<I: Span, O, C, E: std::error::Error + Send + Sync + 'static> =
    IResult<I, O, GenericErrorTree<I, &'static str, C, E>>;

pub fn wrap<I, F, O, C, E>(mut f: F) -> impl FnMut(I) -> Res<I, O, C, E>
where
    I: Span,
    F: FnMut(I) -> Res<I, O, C, E> + Copy,
    E: std::error::Error + Send + Sync + 'static,
{
    move |input: I| f(input)
}

pub fn len<I, F, O, C, E>(f: F) -> impl FnMut(I) -> usize
where
    I: Span,
    F: FnMut(I) -> Res<I, O, C, E> + Copy,
    E: std::error::Error + Send + Sync + 'static,
{
    move |input: I| match recognize(wrap(f))(input) {
        Ok((_, span)) => span.len(),
        Err(_) => 0,
    }
}

pub fn trim<I, F, O, C, E>(f: F) -> impl FnMut(I) -> Res<I, O, C, E>
where
    I: Span,
    F: FnMut(I) -> Res<I, O, C, E> + Copy,
    E: std::error::Error + Send + Sync + 'static,
{
    move |input: I| delimited(multispace0, f, multispace0)(input)
}

pub fn result<I: Span, R>(result: Result<(I, R), nom::Err<NomErr<I>>>) -> Result<R, ParseErrs> {
    match result {
        Ok((_, e)) => Ok(e),
        Err(nom::Err::Error(err)) => Result::Err(err.into()),
        Err(nom::Err::Failure(err)) => Result::Err(err.into()),
        _ => Result::Err(ParseErrs::new(&"Unidentified nom parse error")),
    }
}

pub fn parse_errs<R, E>(result: Result<R, E>) -> Result<R, ParseErrs>
where
    E: Display,
{
    match result {
        Ok(ok) => Ok(ok),
        Err(err) => Err(ParseErrs::new(&(err.to_string()))),
    }
}

pub fn unstack(ctx: &StackContext<ErrCtx>) -> String {
    match ctx {
        StackContext::Kind(k) => k.description().to_string(),
        StackContext::Context(c) => format!("{}", c).to_string(),
    }
}

pub fn recognize<I: Clone + Offset + Slice<RangeTo<usize>>, O, E: ParseError<I>, F>(
    mut parser: F,
) -> impl FnMut(I) -> IResult<I, I, E>
where
    F: ParserExt<I, O, E>,
{
    move |input: I| {
        let i = input.clone();
        match parser.parse(i) {
            Ok((i, _)) => {
                let index = input.offset(&i);
                Ok((i, input.slice(..index)))
            }
            Err(e) => Err(e),
        }
    }
}

pub fn log_parse_err<I, O>(result: crate::parse::Res<I, O>) -> crate::parse::Res<I, O>
where
    I: Span,
{
    if let Result::Err(err) = &result {
        match err {
            nom::Err::Incomplete(_) => {}
            nom::Err::Error(e) => print(e),
            nom::Err::Failure(e) => print(e),
        }
    }
    result
}

pub fn print<I>(err: &NomErr<I>)
where
    I: Span,
{
    match err {
        NomErr::Base { .. } => {
            println!("BASE!");
        }
        NomErr::Stack { base, contexts } => {
            println!("STACK!");
            let mut contexts = contexts.clone();
            contexts.reverse();
            let mut message = String::new();

            if !contexts.is_empty() {
                if let (location, err) = contexts.remove(0) {
                    let mut last = &err;
                    println!(
                        "line {} column: {}",
                        location.location_line(),
                        location.get_column()
                    );
                    let line = unstack(&err);
                    message.push_str(line.as_str());

                    for (span, context) in contexts.iter() {
                        last = context;
                        let line = format!("\n\t\tcaused by: {}", unstack(&context));
                        message.push_str(line.as_str());
                    }
                    ParseErrs::from_loc_span(message.as_str(), last.to_string(), location).print();
                }
            }
        }
        NomErr::Alt(_) => {
            println!("ALT!");
        }
    }
}






pub fn preceded<I, O1, O2, E: ParseError<I>, F, G>(
    mut first: F,
    mut second: G,
) -> impl FnMut(I) -> IResult<I, O2, E>
where
    F: ParserExt<I, O1, E>,
    G: ParserExt<I, O2, E>,
{
    move |input: I| {
        let (input, _) = first.parse(input)?;
        second.parse(input)
    }
}


