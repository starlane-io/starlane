use crate::lib::std::cmp::min;
use crate::lib::std::ops::Range;
use crate::lib::std::string::{String,ToString};
use crate::lib::std::ops::Deref;
use crate::lib::std::str::FromStr;


use crate::space::parse::ctx::{CaseCtx, InputCtx, PrimCtx};
use crate::space::parse::nomplus::err::ParseErr;
use crate::space::parse::nomplus::{Input, MyParser, Res, Tag};
use crate::space::parse::util::recognize;
use ::nom::branch::alt;
use ::nom::bytes::complete::{is_a, tag};
use ::nom::character::complete::{alpha1, alphanumeric1};
use ::nom::combinator::peek;
use ::nom::error::ErrorKind;
use ::nom::multi::many0;
use ::nom::sequence::tuple;
use ::nom::{AsChar, InputTakeAtPosition};
use core::fmt;
use nom::character::complete::alphanumeric0;
use nom::sequence::terminated;
use nom::Slice;
use nom_supreme::ParserExt;
use thiserror_no_std::Error;
use starlane_primitive_macros::Case;
use crate::space::parse::err::{ParseErrsDef, ParseErrsOwn};
use crate::space::parse::nomplus;
use crate::space::util::AsStr;


pub trait Case {
    fn validate<S>( s: &S ) -> Result<(),ParseErr> where S: AsRef<str>;
}


#[derive(Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct SkewerCase(pub(crate) String);

impl Case for SkewerCase {
    fn validate<S>(string: &S) -> Result<(), ParseErr>
    where
        S: AsRef<str>
    {
        for (index, c) in string.as_ref().char_indices() {
            if (index == 0)
            {
                if (!c.is_alpha() || !c.is_lowercase()) {
                    let range = Range::from(0..1);
                    let err = ParseErr::new(CaseCtx::SkewerCase, "skewer case must start with a lowercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10) || c == '-') {
                    let range = Range::from(index - 1..index);
                    let err = ParseErr::new(CaseCtx::SkewerCase, "valid skewer case characters are lowercase alpha, digits 0-9 and dash '-'", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}



#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct VarCase(pub(crate) String);



impl Case for VarCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str>{
        for (index,c) in string.as_ref().char_indices() {
            if( index == 0 )
            {
                if (!c.is_alpha() || !c.is_lowercase()) {
                    let range = Range::from( 0..1);
                    let err = ParseErr::new(CaseCtx::VarCase,"VarCase must start with a lowercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10)||c == '_') {
                    let range = Range::from(index-1..index );
                    let err = ParseErr::new(CaseCtx::VarCase,"valid VarCase case characters are lowercase alpha, digits 0-9 and underscore '_'", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}



#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct DomainCase(pub(crate) String);



impl Case for  DomainCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str>{
        for (index,c) in string.as_ref().char_indices() {
            if( index == 0 )
            {
                if (!c.is_alpha() || !c.is_lowercase()) {
                    let range = Range::from( 0..1);
                    let err = ParseErr::new(CaseCtx::DomainCase,"DomainCase must start with a lowercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10)||c == '-'||c == '.') {
                    let range = Range::from(index-1..index );
                    let err = ParseErr::new(CaseCtx::DomainCase,"valid DomainCase case characters are lowercase alpha, digits 0-9 and dash '-' and dot '.'", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }

}

#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct CamelCase(pub(crate) String);

impl Case for CamelCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str>{
        for (index,c) in string.as_ref().char_indices() {
            if( index == 0 )
            {
                if (!c.is_alpha() || !c.is_uppercase()) {
                    let range = Range::from( 0..1);
                    let err = ParseErr::new(CaseCtx::CamelCase,"CamelCase must start with an uppercase alpha character", range);
                    return Err(err);
                }
            } else {
                if (c.is_alpha() && !c.is_lowercase()) || !(c.is_digit(10)||c == '-'||c == '.') {
                    let range = Range::from(index-1..index );
                    let err = ParseErr::new(CaseCtx::CamelCase,"valid CamelCase characters are mixed case alpha, digits 0-9", range);
                    return Err(err);
                }
            }
        }
        Ok(())
    }
}

#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct FileCase(pub(crate)String);


impl Case for FileCase{

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str>{
        for (index,c) in string.as_ref().char_indices() {
                if !(c.is_alpha() || c.is_digit(10)||c == '-'||c == '.'|| c=='_') {
                    let start = min(0,index-1);
                    let range = Range::from(start..index );
                    let err = ParseErr::new(CaseCtx::FileCase,"valid FileCase case characters are lowercase alpha, digits 0-9 and dash '-', dot '.' and underscore '_'", range);
                    return Err(err);
                }
        }
        Ok(())
    }
}



#[derive( Case, Debug, Clone, Eq, PartialEq, Hash)]
pub struct DirCase(pub(crate)String);


impl Case for DirCase {

    fn validate<S>( string: &S ) -> Result<(),ParseErr> where S: AsRef<str>{
        for (index,c) in string.as_ref().char_indices() {
            if !(c.is_alpha() || c.is_digit(10)||c == '-'||c == '.'|| c=='_') {
                let start = min(0,index-1);
                let range = Range::from(start..index );
                let err = ParseErr::new(CaseCtx::DirCase,"valid DirCase case characters are lowercase alpha, digits 0-9 and dash '-', dot '.' and underscore '_' and must terminate with a '/'", range);
                return Err(err);
            }
        }
        Ok(())
    }
}

pub fn file_case<'a,I: Input>(input: I) -> Res<I, FileCase> {
    recognize(many0(alt((alphanumeric1, tag("-"),tag("_")))).ctx(&CaseCtx::FileCase))(input)
        .map(|(next, rtn)| (next, FileCase(rtn.to_string())))
}

pub fn dir_case<'a,I: Input>(input: I) -> Res<I, DirCase> {
    recognize(terminated(many0(alt((alphanumeric1, tag("-"),tag("_")))),nomplus::tag(Tag::Slash)).ctx(&CaseCtx::DirCase))(input)
        .map(|(next, rtn)| (next, DirCase(rtn.to_string())))
}

pub fn skewer_case<'a,I: Input>(input: I) -> Res<I, SkewerCase> {
    recognize(tuple((peek(alpha1), many0(alt((alphanumeric1, tag("-")))))).ctx(&CaseCtx::SkewerCase))(input)
        .map(|(next, rtn)| (next, SkewerCase(rtn.to_string())))
}

pub fn var_case<'a,I: Input>(input: I) -> Res<I, VarCase> {
    recognize(tuple((peek(alpha1), many0(alt((alphanumeric1, tag("_")))))).ctx(&CaseCtx::VarCase))(input)
        .map(|(next, rtn)| (next, VarCase(rtn.to_string())))
}

pub fn domain_case<'a,I: Input>(input: I) -> Res<I, DomainCase> {
    recognize(tuple((
        peek(alpha1),
        many0(alt((alphanumeric1, tag("-"), tag(".")))),
    )).ctx(CaseCtx::DomainCase))(input)
    .map(|(next, rtn)| (next, DomainCase(rtn.to_string())))
}

pub fn lowercase_alphanumeric<'a,I: Input>(input: I) -> Res<I, I> {
    recognize(tuple((lowercase1, alphanumeric0)))(input)
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
