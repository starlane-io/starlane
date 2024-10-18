use alloc::string::String;
use crate::space::parse::case::{domain_case, skewer_case, var_case, SkewerCase, VarCase};
use crate::space::parse::nomplus::{tag, Input, Res, Tag};
use crate::space::parse::point::PointSeg;
use crate::space::parse::util::{recognize, tron, Trace};
use crate::space::point::RouteSeg;
use alloc::vec::Vec;
use nom::branch::alt;
use nom::combinator::{cut, peek};
use nom::multi::{many0, separated_list0};
use nom::sequence::{delimited, pair, tuple};
use nom_supreme::ParserExt;
use crate::space::parse::ctx::{PointCtx, ToInputCtx};

pub struct Token<'a, I>
where
    I: Input + 'a,
{
    input: I,
    kind: TokenKind,
}

pub enum TokenKind {
    Comment,
    Point(PointSegFragment),
}

pub(crate) enum PointSegFragment {
    Var(Variable),
    RouteSegPart(RouteSeg),
    BaseSeg(SkewerCase),
    FileRoot,
    FileSegPart(String),
    FilePart,
    /// ${some_var}+something (just the + symbol)
    ConCat,
}

pub type Variable = Trace<VarCase>;

pub type PointTokens = Vec<PointSegFragment>;

pub(crate) fn point_fragments<'a, I>(input: I) -> Res<'a, I, PointTokens>
where
    I: 'a + Input,
{
    tuple((separated_list0( point_fragment_concat, point_fragment_base_or_var ),
          separated_list0( point_fragment_concat, point_fragment_base_or_var )
    ))
}

pub(crate) fn point_fragment_base_or_var<'a, I>(input: I) -> Res<'a, I, PointSegFragment>
where
    I: 'a + Input
{
    alt((point_fragment_base,point_fragment_var))(input)
}

pub(crate) fn point_fragment_base<'a, I>(input: I) -> Res<'a, I, PointSegFragment>
where
    I: 'a + Input
{
   skewer_case(input).map( |(next,skewer)| {
       (next, PointSegFragment::BaseSeg(skewer))
   })
}


pub(crate) fn point_fragment_file_root<'a, I>(input: I) -> Res<'a, I, PointSegFragment>
where
    I: 'a + Input
{
    tag(Tag::FileRoot)(input).map( |(next,_)| (next, PointSegFragment::FileRoot))
}


pub(crate) fn point_fragment_concat<'a, I>(input: I) -> Res<'a, I, PointSegFragment>
where
    I: 'a + Input,
{
    tag(Tag::Concat)(input).map(|(next, _)| (next, PointSegFragment::ConCat))
}

pub(crate) fn point_fragment_var<I>(input: I) -> Res<I, PointSegFragment>
where
    I: Input,
{
    pair(
        peek(tag(Tag::VarPrefix)),
        cut(delimited(tag(Tag::VarOpen), tron(var_case), tag(Tag::VarClose))).ctx(PointCtx::Var),
    )(input).map( |(next,(_,var_name))|{
        (next,var_name)
    })

}

pub(crate) fn base_segment_tokens<'a, I>(input: I) -> Res<'a, I, PointTokens>
where
    I: 'a + Input,
{
}
