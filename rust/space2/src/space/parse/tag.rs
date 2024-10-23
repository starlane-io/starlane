use nom::InputLength;
use alloc::string::ToString;
use starlane_primitive_macros::Autobox;
use crate::space::parse::case::CharTag;
use crate::space::parse::nomplus::{Input, Res};
use crate::space::parse::token::point::PointTag;
use crate::space::parse::util::SliceStr;
use crate::space::parse::err::ParseErrs;

#[derive(Clone,Eq,PartialEq,Debug)]
pub enum Tag {
    HyperSegmentSep,
    PointSegSep,
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
    Point(PointTag),
    Char(CharTag)
}

impl InputLength for Tag {
    fn input_len(&self) -> usize {
        self.as_str().input_len()
    }
}

impl Into<SliceStr> for Tag {
    fn into(self) -> SliceStr {
        SliceStr::new(self.as_str().to_string())
    }
}

impl Tag {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tag::HyperSegmentSep => "::",
            Tag::PointSegSep => ":",
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
            Tag::FileRoot => "/",
            Tag::Point(pnt) => pnt.as_str(),
            Tag::Char(c) => c.as_str()
        }
    }
}

impl crate::lib::std::convert::AsRef<[u8]> for Tag {
    fn as_ref(&self) -> &[u8] {
        self.as_str().as_bytes()
    }
}

pub fn tag<'a, I, T>(tag: T ) -> impl Clone + FnMut(I) -> Res<I, I> where I: Input, T: Into<Tag>{
   let tag = tag.into();
    move |input:I| {
        nom_supreme::tag::complete::tag(tag.as_str())(input)
    }

}



