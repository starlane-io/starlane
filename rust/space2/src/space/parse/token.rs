use crate::lib::std::string::String;
use crate::lib::std::vec::Vec;
use crate::space::parse::case::{
    dir_case, domain_case, file_case, skewer_case, var_case, CamelCase, DirCase, DomainCase,
    FileCase, SkewerCase, VarCase,
};
use crate::space::parse::ctx::{PointCtx, ToInputCtx};
use crate::space::parse::nomplus::{tag, Input, MyParser, Res, Tag};
use crate::space::parse::point::{route_seg, PointSeg};
use crate::space::parse::util::{recognize, tron, Trace};
use crate::space::point::RouteSeg;
use nom::branch::alt;
use nom::character::complete::{multispace0, multispace1};
use nom::combinator::{cut, opt, peek};
use nom::multi::{many0, separated_list0};
use nom::sequence::{delimited, pair, terminated, tuple};
use nom_supreme::ParserExt;

pub type TokenTron = Trace<Token>;

pub enum Token {
    Comment,
    Point(PntFragment),
}

pub(crate) enum PntFragment {
    RouteSeg(RouteSeg),
    Var(VarCase),
    CamelCase(CamelCase),
    SkewerCase(SkewerCase),
    /// the first slash '/'
    FileRoot,
    DirFrag(FileCase),
    DirEnd(DirCase),
    File(FileCase),
    DomainCase(DomainCase),
    FilePart,
    /// ${some_var}+something+${something_else}+${suffix} (just the + symbol)
    ConCat,
    Def,
    SegSep,
    RouteSegSep,
}

impl Into<Token> for PntFragment {
    fn into(self) -> Token {
        Token::Point(self)
    }
}

pub type Variable = Trace<VarCase>;

pub type PointTokens = Vec<PntFragment>;

pub(crate) fn tk<'a, I, F, O>(f: F) -> impl FnMut(I) -> Res<I, TokenTron>
where
    I: Input,
    F: FnMut(I) -> Res<I, O> + Copy,
    O: Into<Token>,
{
    move |input| {
        tron(f)(input).map(|(next, output)| {
            let o = output.w.into();
            let trace = Trace {
                w: o,
                range: output.range,
            };

            (next, trace)
        })
    }
}

pub(crate) fn point_fragments<'a, I>(input: I) -> Res<I, PointTokens>
where
    I: 'a + Input,
{
    terminated(
        tuple((
            opt(terminated(tk(point_route_segment), tag(Tag::RouteSegSep))),
            separated_list0(point_fragment_concat, tk(point_fragment_base)),
            opt(tuple((
                tk(point_fragment_file_root),
                many0(tk(point_fragment_file)),
                opt(point_fragment_file),
            ))),
        )),
        point_fragments_end,
    )(input)
    .map(|(next, (route, base, files))| (next, PointTokens::new()))
}

pub(crate) fn point_route_segment<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    terminated(route_seg, tag(Tag::RouteSegSep))(input).map(|(r, t)| (r, PntFragment::RouteSeg(t)))
}

pub(crate) fn point_fragment_base<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    alt((
        point_fragment_domain,
        point_fragment_var,
        point_fragment_concat,
    ))(input)
}

pub(crate) fn point_fragment_domain<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    domain_case(input).map(|(next, domain)| (next, PntFragment::DomainCase(domain)))
}
pub(crate) fn point_fragment_file_root<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    tag(Tag::FileRoot)(input).map(|(next, _)| (next, PntFragment::FileRoot))
}

pub(crate) fn point_fragment_file<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    fn dir_end<'a, I>(input: I) -> Res<I, PntFragment>
    where
        I: 'a + Input,
    {
        dir_case(input).map(|(next, dir)| (next, PntFragment::DirEnd(dir)))
    }

    fn dir_fragment<'a, I>(input: I) -> Res<I, PntFragment>
    where
        I: 'a + Input,
    {
        file_case(input).map(|(next, file)| (next, PntFragment::DirFrag(file)))
    }

    alt((
        dir_end,
        dir_fragment,
        point_fragment_var,
        point_fragment_concat,
    ))(input)
}

pub(crate) fn point_fragments_end<'a, I>(input: I) -> Res<I, I>
where
    I: 'a + Input,
{
    alt((multispace1))
}

pub(crate) fn point_fragment_base_sep<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    alt((point_fragment_segment_delimeter, point_fragment_concat))(input)
}

pub(crate) fn point_fragment_segment_delimeter<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    tag(Tag::SegSep)(input).map(|(next, _)| (next, PntFragment::SegSep))
}

pub(crate) fn point_fragment_route_seg<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    route_seg(input).map(|(r, t)| (r, PntFragment::RouteSeg(t)))
}

pub(crate) fn point_fragment_route_seg_sep<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    tag(Tag::RouteSegSep)(input).map(|(next, _)| (next, PntFragment::SegSep))
}

pub(crate) fn point_fragment_concat<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    tag(Tag::Concat)(input).map(|(next, _)| (next, PntFragment::ConCat))
}

pub(crate) fn point_fragment_var<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: Input,
{
    pair(
        peek(tag(Tag::VarPrefix)),
        cut(delimited(tag(Tag::VarOpen), var_case, tag(Tag::VarClose)))(input)
            .map(|(next, var_name)| (next, PntFragment::Var(var_name))),
    )
}

pub(crate) fn base_segment_tokens<'a, I>(input: I) -> Res<I, PointTokens>
where
    I: 'a + Input,
{
}
