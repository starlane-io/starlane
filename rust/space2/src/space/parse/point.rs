use alloc::string::String;
use nom::{AsChar, IResult, InputTakeAtPosition};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{multispace1, space1};
use nom::combinator::{cut, eof, not, peek, recognize};
use nom::error::ErrorKind;
use nom::sequence::{delimited, pair, preceded};
use nom_supreme::ParserExt;
use serde::{Deserialize, Serialize};
use log::trace;
use crate::space::parse::case::{domain_case, lowercase1, skewer_case, var_case};
use crate::space::parse::ctx::{InputCtx, PointCtx};
use crate::space::parse::nom::{ErrTree, Input, Res, Tag};
use crate::space::point::{Point, PointDef, RouteSeg};
use crate::space::parse::nom::MyParser;
use crate::space::parse::util::tron;
use crate::space::parse::vars::Variable;

fn var<I, O>(input: I) -> Res<I, Variable> where I: Input{
    pair(
        peek(tag("$")),
        cut(tron(delimited(
            tag("${"),
            var_case,
            tag("}"),
        ))),
    )(input)
        .map(|(next, (_, var))| (next, var))
}
pub fn tokenize_point<I>(input: I) -> Res<I,Point> where I: Input {

}

pub fn point_def<I,R,S,RouteSeg,Seg>(route: R, seg: S) -> Res<I,PointCtx,PointDef<RouteSeg,Seg>> where I: Input, R: FnOnce(I) -> Res<I, RouteSeg>+Copy {
}

pub fn mesh_eos<I: Input>(input: I) -> Res<I, I> {
    peek(alt((tag(":"), eop)))(input)
}



pub fn base_point_segment<I>(input: I) -> Res<I, PointSeg> where I: Input{
    preceded(
        peek(lowercase1),
        cut(pair(recognize(skewer_case), mesh_eos)),
    )(input)
        .map(|(next, (base, _))| (next, PointSeg::Base(base.to_string())))
}

pub fn route_seg<I>(input: I) -> Res<I, RouteSeg> {
    alt((
        this_route_segment,
        sys_route_segment,
        tag_route_segment,
        domain_route_segment,
        global_route_segment,
        local_route_segment,
        remote_route_segment,
    ))(input)
}

pub fn eos<I>(input: I) -> Res<I, ()> where I: Input{
    peek(alt((tag("/"), tag(":"), tag("%"), space1, eof)))(input).map(|(next, _)| (next, ()))
}



struct PointSegParser {

}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum PointSeg {
    Root,
    Base(String),
    FsRootDir,
    Dir(String),
    File(String),
    Version(Version),
}

impl <I,O,Ctx> MyParser<I,O,Ctx> for PointSegParser where I: Input , Ctx: InputCtx{
    fn parse(&mut self, input: I) -> Res<I, O, ErrTree<'a, I, Ctx>> {

    }
}


fn any_resource_path_segment<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !(char_item == '/')
                && !(char_item == '_')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

fn sys_route_chars<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !(char_item == '/')
                && !(char_item == '_')
                && !(char_item == ':')
                && !(char_item == '(')
                && !(char_item == ')')
                && !(char_item == '[')
                && !(char_item == ']')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn this_route_segment<I: Input>(input: I) -> Res<I, RouteSeg> {
    alt((recognize(tag(".")), recognize(not(other_route_segment))))(input)
        .map(|(next, _)| (next, RouteSeg::This))
}

pub fn local_route_segment<I: Input>(input: I) -> Res<I, RouteSeg> {
    tag("LOCAL")(input).map(|(next, _)| (next, RouteSeg::Local))
}

pub fn remote_route_segment<I: Input>(input: I) -> Res<I, RouteSeg> {
    tag("REMOTE")(input).map(|(next, _)| (next, RouteSeg::Remote))
}

pub fn global_route_segment<I: Input>(input: I) -> Res<I, RouteSeg> {
    tag("GLOBAL")(input).map(|(next, _)| (next, RouteSeg::Global))
}

pub fn domain_route_segment<I: Input>(input: I) -> Res<I, RouteSeg> {
    domain_case(input).map(|(next, domain)| (next, RouteSeg::Domain(domain.to_string())))
}

pub fn tag_route_segment<I: Input>(input: I) -> Res<I, RouteSeg> {
    delimited(tag("#["), skewer_case, tag("]"))(input)
        .map(|(next, tag)| (next, RouteSeg::Tag(tag.to_string())))
}

pub fn sys_route_segment<I: Input>(input: I) -> Res<I, RouteSeg> {
    delimited(tag("<<"), sys_route_chars, tag(">>"))(input)
        .map(|(next, tag)| (next, RouteSeg::Star(tag.to_string())))
}


pub fn eop<I: Input>(input: I) -> Res<I, I> {
    peek(alt((
        eof,
        multispace1,
        tag("<"),
        tag("\""),
        tag("'"),
        tag("]"),
        tag(")"),
        tag("}"),
        tag("^"),
        tag("["),
        tag("("),
        tag("{"),
        tag("%"),
    )))(input)
}
