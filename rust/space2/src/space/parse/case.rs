use crate::lib::std::string::ToString;
use crate::space::parse::ctx::CaseCtx;
use crate::space::parse::nomplus::{Input, MyParser, Res};
use crate::space::parse::util::recognize;
use ::nom::branch::alt;
use ::nom::character::complete::{alpha1, alphanumeric1};
use ::nom::combinator::peek;
use ::nom::error::ErrorKind;
use ::nom::multi::many0;
use ::nom::sequence::tuple;
use ::nom::{AsChar, InputTakeAtPosition};
use nom::character::complete::alphanumeric0;
use nom::sequence::terminated;
use nom_supreme::ParserExt;
use crate::space::case::{CamelCase, DirCase, DomainCase, FileCase, SkewerCase, VarCase};
use crate::space::parse::tag;
use crate::space::parse::tag::{tag, Tag};

pub fn file_case<'a,I: Input>(input: I) -> Res<I, FileCase> {
    recognize(many0(alt((alphanumeric1, tag(CharTag::Dash),tag(CharTag::Underscore)))).ctx(CaseCtx::FileCase))(input)
        .map(|(next, rtn)| (next, FileCase(rtn.to_string())))
}

pub fn dir_case<'a,I: Input>(input: I) -> Res<I, DirCase> {
    recognize(terminated(many0(alt((alphanumeric1, tag(CharTag::Dash),tag(CharTag::Underscore)))), tag::tag(Tag::Slash)).ctx(CaseCtx::DirCase))(input)
        .map(|(next, rtn)| (next, DirCase(rtn.to_string())))
}

pub fn skewer_case<'a,I: Input>(input: I) -> Res<I, SkewerCase> {
    recognize(tuple((peek(alpha1), many0(alt((lowercase_alphanumeric, tag(CharTag::Dash)))))).ctx(CaseCtx::SkewerCase))(input)
        .map(|(next, rtn)| (next, SkewerCase(rtn.to_string())))
}

pub fn camel_case<'a,I: Input>(input: I) -> Res<I, CamelCase> {
    recognize(tuple((peek(alpha1), many0(alt((alphanumeric0, tag(CharTag::Dash),tag(CharTag::Dash)))))).ctx(CaseCtx::SkewerCase))(input)
        .map(|(next, rtn)| (next, CamelCase(rtn.to_string())))
}

pub fn var_case<'a,I: Input>(input: I) -> Res<I, VarCase> {
    recognize(tuple((peek(alpha1), many0(alt((alphanumeric1, tag(CharTag::Underscore)))))).ctx(CaseCtx::VarCase))(input)
        .map(|(next, rtn)| (next, VarCase(rtn.to_string())))
}

pub fn domain_case<'a,I: Input>(input: I) -> Res<I, DomainCase> {
    recognize(tuple((peek(alpha1), many0(alt((lowercase_alphanumeric, tag(CharTag::Dash), tag(CharTag::Dot)))))).ctx(CaseCtx::DomainCase))(input)
        .map(|(next, rtn)| (next, DomainCase(rtn.to_string())))
}

pub fn lowercase_alphanumeric<'a,I: Input>(input: I) -> Res<I, I> {
    recognize(tuple((lowercase1, alphanumeric0)))(input)
}


pub fn point_case<'a,I: Input>(input: I) -> Res<I, I> {
    alt((recognize(skewer_case),recognize(camel_case),recognize(skewer_case), recognize(alphanumeric1)))(input)
}

pub fn lowercase1<'a,T: Input>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item.is_alpha() && char_item.is_lowercase())
        },
        ErrorKind::AlphaNumeric,
    )
}

#[derive(Clone,Eq,PartialEq,Debug)]
pub enum CharTag {
    Dash,
    Underscore,
    Dot,
}

impl CharTag {
    pub fn as_str(&self) -> &'static str {
        match self {
            CharTag::Dash => "-",
            CharTag::Underscore => "_",
            CharTag::Dot => "."
        }
    }
}

impl Into<Tag> for CharTag {
    fn into(self) -> Tag {
        Tag::Char(self)
    }
}