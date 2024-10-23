use crate::lib::std::string::String;
use nom::{AsChar, InputTakeAtPosition};
use nom::branch::alt;
use nom::character::complete::{alphanumeric1, multispace1, space1};
use nom::combinator::{cut, eof, not, opt, peek, recognize};
use nom::error::ErrorKind;
use nom::sequence::{delimited, pair, terminated, tuple};
use nom_supreme::ParserExt;
use nom::multi::{many0, separated_list0};
use starlane_primitive_macros::Autobox;
use crate::space::parse::case::{dir_case, domain_case, file_case, skewer_case, var_case, CharTag, DirCase, DomainCase, FileCase, SkewerCase, VarCase};
use crate::space::parse::nomplus::{Input, Res};
use crate::space::point::HyperSegment;
use crate::space::parse::tag::{tag, Tag};
use crate::space::parse::token;
use crate::space::parse::token::{PointTokens, Token};
/*fn var<'a,I, O>(input: I) -> Res<I, Variable> where I: Input{
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

 */


#[derive(Clone,Eq,PartialEq,Debug)]
pub enum PointTag {
    Hyper(HyperPointTag),
    EndOfSegment(EndOfSegmentTag),
    TagOpen,
    TagClose,
    SysOpen,
    SysClose
}

impl Into<Tag> for PointTag {
    fn into(self) -> Tag {
        Tag::Point(self)

    }
}

impl PointTag {
    pub fn as_str(&self) -> &'static str  {
        match self {
            PointTag::Hyper(hyp) => hyp.as_str(),
            PointTag::EndOfSegment(eos) => eos.as_str(),
            PointTag::TagOpen => "#[",
            PointTag::TagClose => "]",
            PointTag::SysOpen => "<<",
            PointTag::SysClose => ">>"
        }
    }
}

#[derive(Clone,Eq,PartialEq,Debug)]
pub enum HyperPointTag {
    Space,
    Global,
    Remote,
}

impl HyperPointTag{
    pub fn as_str(&self) -> &'static str  {
        match self {
            HyperPointTag::Space => "SPACE",
            HyperPointTag::Global => "GLOBAL",
            HyperPointTag::Remote => "REMOTE"
        }
    }
}

impl Into<Tag> for HyperPointTag {
    fn into(self) -> Tag {
        Tag::Point(PointTag::Hyper(self))
    }
}





pub fn hyper_segment<'a,I>(input: I) -> Res<I, HyperSegment> where I: Input{
    alt((
        space_segment,
        hyper_sys_segment,
        hyper_tag_segment,
        hyper_domain_segment,
        hyper_global_segment,
        hyper_remote_segment,
    ))(input)
}

#[derive(Clone,Eq,PartialEq,Debug)]
pub enum EndOfSegmentTag {
    SegSep,
    Slash,
    AutoName,
}

impl EndOfSegmentTag{
    pub fn as_str(&self) -> &'static str  {
        match self {
            EndOfSegmentTag::SegSep => ":",
            EndOfSegmentTag::Slash => "/",
            EndOfSegmentTag::AutoName => "%"
        }
    }
}

impl Into<Tag> for EndOfSegmentTag {
    fn into(self) -> Tag {
        Tag::Point(PointTag::EndOfSegment(self))
    }
}

pub fn eos<'a,I>(input: I) -> Res<I, ()> where I: Input{
    peek(alt((tag(EndOfSegmentTag::SegSep), tag(EndOfSegmentTag::Slash),tag(EndOfSegmentTag::AutoName), multispace1, eof)))(input).map(|(next, _)| (next, ()))
}



struct PointSegParser {

}






fn any_resource_path_segment<'a,T>(i: T) -> Res< T, T>
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

fn sys_hyper_segment_chars<'a,T>(i: T) -> Res<T, T>
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

/*
pub fn hyper_segment_this<'a,I: Input>(input: I) -> Res<I, RouteSeg> {
    alt((recognize(tag(HyperPointTag::This)), recognize(not(hyper_segment))))(input)
        .map(|(next, _)| (next, RouteSeg::This))
}

 */

pub fn space_segment<'a,I: Input>(input: I) -> Res<I, HyperSegment> {
    tag(HyperPointTag::Space)(input).map(|(next, _)| (next, HyperSegment::Space))
}

pub fn hyper_remote_segment<'a,I: Input>(input: I) -> Res<I, HyperSegment> {
    tag(HyperPointTag::Remote)(input).map(|(next, _)| (next, HyperSegment::Remote))
}

pub fn hyper_global_segment<'a,I: Input>(input: I) -> Res<I, HyperSegment> {
    tag(HyperPointTag::Global)(input).map(|(next, _)| (next, HyperSegment::Global))
}

pub fn hyper_domain_segment<'a,I: Input>(input: I) -> Res<I, HyperSegment> {
    domain_case(input).map(|(next, domain)| (next, HyperSegment::Domain(domain)))
}

pub fn hyper_tag_segment<'a,I: Input>(input: I) -> Res<I, HyperSegment> {
    delimited(tag(PointTag::TagOpen), skewer_case, tag(PointTag::TagClose))(input)
        .map(|(next, tag)| (next, HyperSegment::Tag(tag)))
}

pub fn hyper_sys_segment<'a,I: Input>(input: I) -> Res<I, HyperSegment> {
    delimited(tag(PointTag::SysOpen), sys_hyper_segment_chars, tag(PointTag::SysClose))(input)
        .map(|(next, tag)| (next, HyperSegment::Star(tag.to_string())))
}


// end of point
pub fn eop<'a,I: Input>(input: I) -> Res<I, I> {
    peek(alt((
        eof,
        multispace1,
    )))(input)
}

pub(crate) enum PntFragment {
    HyperSegment(HyperSegment),
    Var(VarCase),
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

pub(crate) fn point_fragments<'a, I>(input: I) -> Res<I, PointTokens>
where
    I: 'a + Input,
{
    terminated(
        tuple((
            opt(terminated(token::tk(point_hyper_segment), tag(Tag::HyperSegmentSep))),
            separated_list0(point_fragment_concat, token::tk(point_fragment_base)),
            opt(tuple((
                token::tk(point_fragment_file_root),
                many0(token::tk(point_fragment_file)),
                opt(point_fragment_file),
            ))),
        )),
        point_fragments_end,
    )(input)
    .map(|(next, (route, base, files))| (next, PointTokens::new()))
}

pub(crate) fn point_hyper_segment<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    terminated(hyper_segment, tag(Tag::HyperSegmentSep))(input).map(|(r, t)| (r, PntFragment::HyperSegment(t)))
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
    alt((multispace1,eof))(input)
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

pub(crate) fn point_fragment_hyperoute_seg<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    hyper_segment(input).map(|(r, t)| (r, PntFragment::HyperSegment(t)))
}

pub(crate) fn point_fragment_route_seg_sep<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    tag(Tag::HyperSegmentSep)(input).map(|(next, _)| (next, PntFragment::SegSep))
}

pub(crate) fn point_fragment_concat<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: 'a + Input,
{
    tag(Tag::Concat)(input).map(|(next, _)| (next, PntFragment::ConCat))
}

pub(crate) fn point_fragment_var<'a, I>(input: I) -> Res<I, PntFragment>
where
    I: Input + 'a,
{

//    tuple((
//        peek(tag(Tag::VarPrefix)),
        delimited(tag(Tag::VarOpen), var_case, tag(Tag::VarClose))(input)
            .map(|(next, var)| (next, PntFragment::Var(var)))
}

/*
pub(crate) fn base_segment_tokens<'a, I>(input: I) -> Res<I, PointTokens>
where
    I: 'a + Input,
{

}

 */