use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use core::ops::Deref;
use core::range::{Range, RangeFrom, RangeTo};
use nom::{AsBytes, AsChar, Compare, CompareResult, FindSubstring, IResult, InputIter, InputLength, InputTake, InputTakeAtPosition, Needed, Offset, Parser, Slice};
use nom::error::{ErrorKind, ParseError};
use nom_supreme::context::ContextError;
use nom_supreme::error::GenericErrorTree;
use nom_supreme::parser_ext::Context;
use nom_supreme::ParserExt;
use crate::space::parse::ctx::{InputCtx, ToInputCtx};
use crate::space::parse::util::{CharIterator, MyChars, SliceStr};
use crate::RustErr;
use crate::space::parse::nomplus::err::ParseErr;

pub type LocatedSpan<'a> = nom_locate::LocatedSpan<&'a str,()>;


pub type ErrTree<I: Input> = GenericErrorTree<I, Tag, InputCtx , ParseErr>;
pub type Res<I: Input,Output> = IResult<I, Output, ErrTree<I>>;


pub trait MyParser<'a,I:Input,O> : ParserExt<I,O,ErrTree<I>> where I:Input{
        fn ctx<C>( self, ctx: C) -> Context<Self,InputCtx> where C: ToInputCtx+Copy{
            self.context(ctx.to()())
        }
}



impl<'a,I, O, P> MyParser<'a, I, O> for P where P: Parser<I, O, ErrTree<'a,I>>, I: Input {

}


pub trait Input:
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


}

impl<'a> Input for Span<LocatedSpan<'a>> {
    fn location_offset(&self) -> usize {
        self.input.location_offset()
    }

    fn get_column(&self) -> usize {
        self.input.get_column()
    }

    fn location_line(&self) -> u32 {
        self.input.location_line()
    }

    fn extra(&self) -> () {
        ()
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




#[derive(Debug, Clone)]
pub struct Span<I>
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

impl<I> Span<I>
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

impl<I> Deref for Span<I>
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

impl<I> AsBytes for Span<I>
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

impl<I> Slice<Range<usize>> for Span<I>
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

impl<I> Slice<RangeFrom<usize>> for Span<I>
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

impl<I> Slice<RangeTo<usize>> for Span<I>
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

impl<'a> Compare<&'static str> for Span<LocatedSpan<'a>> {
    fn compare(&self, t: &str) -> CompareResult {
        self.input.compare(t)
    }

    fn compare_no_case(&self, t: &str) -> CompareResult {
        self.input.compare_no_case(t)
    }
}

impl<I> InputLength for Span<I>
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

impl<I> Offset for Span<I>
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

impl<I> InputIter for Span<I>
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

impl<I> InputTake for Span<I>
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
        Span::new(self.input.take(count))
    }

    fn take_split(&self, count: usize) -> (Self, Self) {
        let (left, right) = self.input.take_split(count);
        (Span::new(left), Span::new(right))
    }
}

impl<I> ToString for Span<I>
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

impl<'a> FindSubstring<&str> for Span<LocatedSpan<'a>> {
    fn find_substring(&self, substr: &str) -> Option<usize> {
        self.input.find_substring(substr)
    }
}

impl<I> InputTakeAtPosition for Span<I>
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




#[derive(Clone)]
pub enum Tag {
    RouteSegSep,
    SegSep,
    VarPrefix,
    CurlyOpen,
    CurlyClose,
    AngleOpen,
    AngleClose,
    SquareOpen,
    SquareClose,
    ParenOpen,
    ParenClose,
    Pipe,
    DoubleQuote,
    SingleQuote,
    Slash,
    At,
    Bang,
    Question,
    Wildcard,
    BackTic,
    Pound,
    Plus,
    Minus,
    Concat,
    VarOpen,
    VarClose,
    FileRoot,
}



impl Into<SliceStr> for Tag {
    fn into(self) -> SliceStr {
        SliceStr::new(self.as_str().to_string())
    }
}




impl Tag {
    fn as_str(&self) -> &'static str {
        match self {
            Tag::RouteSegSep => "::",
            Tag::SegSep => ":",
            Tag::VarPrefix => "$",
            Tag::CurlyOpen => "{",
            Tag::CurlyClose => "}",
            Tag::AngleOpen => "<",
            Tag::AngleClose => ">",
            Tag::SquareOpen => "[",
            Tag::SquareClose => "]",
            Tag::ParenOpen => "(",
            Tag::ParenClose=> ")",
            Tag::Pipe => "|",
            Tag::DoubleQuote => "\"",
            Tag::SingleQuote => "'",
            Tag::Slash => "/",
            Tag::At => "@",
            Tag::Bang => "!",
            Tag::Question => "?",
            Tag::Wildcard => "*",
            Tag::BackTic => "`",
            Tag::Pound => "#",
            Tag::Plus => "+",
            Tag::Minus => "-",
            Tag::Concat => "+",
            Tag::VarOpen => "${",
            Tag::VarClose => "}",
            Tag::FileRoot => ":/"
        }
    }
}


struct Scoped {
   pub open: &'static str,
   pub close: &'static str
}



pub fn tag<'a, I>( tag: Tag ) -> impl Clone + Fn(I) -> Res<I, I> where I: Input{
   let tag : SliceStr= tag.into();
   nom_supreme::tag::complete::tag(tag)
}


pub mod err {
    use alloc::boxed::Box;
    use alloc::format;
    use alloc::string::{String, ToString};
    use core::range::Range;
    use nom_supreme::error::GenericErrorTree;
    use thiserror_no_std::Error;
    use crate::space::parse::ctx::{InputCtx, ToInputCtx};
    use crate::space::parse::nomplus::{Input, Tag};

    pub struct ErrCtxStack {

    }

    #[derive(Error)]
    pub struct ParseErr  {
        ctx: InputCtx,
        message: String,
        range: Range<usize>,
    }

    impl ParseErr {
        pub fn new<Ctx,M>( ctx: Ctx, message: M, range: Range<usize> ) -> Self where Ctx: ToInputCtx, M: AsRef<str>
        {
            let message = message.as_ref().to_string();
            let ctx = ctx.to()();
            Self {
                ctx,
                message,
                range,
            }
        }
    }

    pub trait ErrMsg: 'static+Into<&'static str>
    {
        fn msg( &self, span: &str) -> &str;
    }


}