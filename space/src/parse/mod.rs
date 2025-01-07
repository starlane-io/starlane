pub mod nomplus;
#[cfg(test)]
pub mod test;
pub mod util;
//pub mod error;
//pub mod error;

use crate::command::common::{PropertyMod, SetProperties, StateSrcVar};
use crate::command::direct::create::{
    CreateVar, KindTemplate, PointSegTemplate, PointTemplateSeg, PointTemplateVar, Strategy,
    TemplateVar,
};
use crate::command::direct::get::{GetOp, GetVar};
use crate::command::direct::select::{SelectIntoSubstance, SelectKind, SelectVar};
use crate::command::direct::set::SetVar;
use crate::command::direct::CmdKind;
use crate::command::CommandVar;
use crate::config::bind::{
    BindConfig, PipelineStepVar, PipelineStopVar, RouteSelector, WaveDirection,
};
use crate::config::mechtron::MechtronConfig;
use crate::config::{DocKind, Document};
use crate::err::report::{Label, Report, ReportKind};
use crate::err::ParseErrs;
use crate::kind::{
    ArtifactSubKind, BaseKind, DatabaseSubKind, FileSubKind, Kind, KindParts, Specific, StarSub,
    Sub, UserBaseSubKind,
};
use crate::loc::StarKey;
use crate::loc::{Layer, PointSegment, Surface, Topic, Uuid, VarVal, Version};
use crate::parse::util::unstack;
use crate::parse::util::{log_parse_err, preceded, recognize, result};
use crate::particle::PointKindVar;
use crate::point::{
    Point, PointCtx, PointSeg, PointSegCtx, PointSegDelim, PointSegVar, PointVar, RouteSeg,
    RouteSegVar,
};
use crate::security::{
    AccessGrantKind, AccessGrantKindDef, ChildPerms, ParticlePerms, Permissions, PermissionsMask,
    PermissionsMaskKind, Privilege,
};
use crate::selector::{
    ExactPointSeg, KindBaseSelector, KindSelector, LabeledPrimitiveTypeDef, MapEntryPattern,
    MapEntryPatternVar, Pattern, PatternBlockVar, PayloadBlockVar, PayloadType2Def, PointHierarchy,
    PointKindSeg, PointSegKindHop, PointSegSelector, Selector, SelectorDef, SpecificSelector,
    SubKindSelector, UploadBlock, VersionReq,
};
use crate::substance::Bin;
use crate::substance::{
    CallKind, CallVar, CallWithConfigVar, ExtCall, HttpCall, ListPattern, MapPatternVar, NumRange,
    Substance, SubstanceFormat, SubstanceKind, SubstancePattern, SubstancePatternVar,
    SubstanceTypePatternDef, SubstanceTypePatternVar,
};
use crate::util::{HttpMethodPattern, StringMatcher, ToResolved, ValuePattern};
use crate::wave::core::cmd::CmdMethod;
use crate::wave::core::ext::ExtMethod;
use crate::wave::core::http2::HttpMethod;
use crate::wave::core::hyper::HypMethod;
use crate::wave::core::MethodKind;
use crate::wave::core::{Method, MethodPattern};
use anyhow::Context;
use core::fmt;
use core::fmt::Display;
use derive_name::Name;
use model::{
    BindScope, BindScopeKind, Block, BlockKind, Chunk, DelimitedBlockKind, LexBlock,
    LexParentScope, LexRootScope, LexScope, LexScopeSelector, MechtronScope, NestedBlockKind,
    PipelineSegmentVar, PipelineVar, RootScopeSelector, RouteScope, ScopeFilterDef,
    ScopeFiltersDef, Spanned, Subst, TerminatedBlockKind, TextType, VarParser,
};
use nom::branch::alt;
use nom::bytes::complete::{is_a, is_not};
use nom::bytes::complete::{tag, take_until};
use nom::character::complete::{alpha1, digit1};
use nom::character::complete::{
    alphanumeric0, alphanumeric1, anychar, char, multispace0, multispace1, satisfy, space1,
};
use nom::combinator::{all_consuming, into, opt};
use nom::combinator::{cut, eof, fail, not, peek, value, verify};
use nom::error::{ErrorKind, ParseError};
use nom::multi::{many0, many1, separated_list0};
use nom::sequence::{delimited, pair, terminated, tuple};
use nom::{
    AsChar, Compare, FindToken, InputIter, InputLength, InputTake, InputTakeAtPosition, Offset,
    Parser, Slice,
};
use nom::{Err, IResult};
use nom_locate::LocatedSpan;
use nom_supreme::context::ContextError;
use nom_supreme::error::GenericErrorTree;
use nom_supreme::final_parser::ExtractContext;
use nom_supreme::ParserExt;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with_macros::{DeserializeFromStr, SerializeDisplay};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::Formatter;
use std::ops::{Deref, RangeFrom, RangeTo};
use std::str::FromStr;
use std::sync::Arc;
use thiserror::Error;
use starlane_space::types::parse::TzoParser;
use util::{new_span, span_with_extra, trim, tw, Span, Trace, Wrap};

pub type SpaceContextError<I: Span> = dyn nom_supreme::context::ContextError<I, ErrCtx>;
pub type StarParser<I: Span, O> = dyn nom_supreme::parser_ext::ParserExt<I, O, NomErr<I>>;

pub type Xpan<'a> = Wrap<LocatedSpan<&'a str, Arc<String>>>;

impl<I> From<NomErr<I>> for ParseErrs
where
    I: Span,
{
    fn from(err: NomErr<I>) -> Self {
        match err {
            NomErr::Base { location, kind } => ParseErrs::from_loc_span(
                "undefined parse error (error is not associated with an ErrorContext",
                "undefined",
                location.clone(),
            ),
            NomErr::Stack { base, contexts } => {
                let mut contexts = contexts.clone();
                contexts.reverse();
                let mut message = String::new();

                if !contexts.is_empty() {
                    if let (location, err) = contexts.remove(0) {
                        let mut last = &err;
                        let line = unstack(&err);
                        message.push_str(line.as_str());

                        for (span, context) in contexts.iter() {
                            last = context;
                            let line = format!("\n\t\tcaused by: {}", unstack(&context));
                            message.push_str(line.as_str());
                        }
                        return ParseErrs::from_loc_span(
                            message.as_str(),
                            last.to_string(),
                            location,
                        );
                    }
                }

                ParseErrs::default()
            }
            NomErr::Alt(_) => {
                //println!("ALT!");
                ParseErrs::default()
            }
        }
    }
}

impl<I> From<nom::Err<NomErr<I>>> for ParseErrs
where
    I: Span,
{
    fn from(err: nom::Err<NomErr<I>>) -> Self {
        match err {
            Err::Incomplete(i) => ParseErrs::default(),
            Err::Error(err) => err.into(),
            Err::Failure(err) => err.into(),
        }
    }
}
pub fn context<I, F, O>(context: &'static str, mut f: F) -> impl FnMut(I) -> Res<I, O>
where
    F: Parser<I, O, NomErr<I>>,
    I: Span,
{
    /*
    let context = ErrCtx::Yikes;
    move |i: I| match f.parse(i.clone()) {
        Ok(o) => Ok(o),
        Err(Err::Incomplete(i)) => Err(Err::Incomplete(i)),

        Err(Err::Error(e)) => Err(Err::Error(ParseTree::add_context(i, context.clone(), e))),
        Err(Err::Failure(e)) => Err(Err::Failure(ParseTree::add_context(i, context.clone(), e))),

    }
     */

    move |input: I| f.parse(input)
}

pub enum SpaceParseErr {}

#[cfg(test)]
pub mod test2 {
    use crate::parse::point_var;
    use crate::parse::util::{new_span, result};

    #[test]
    pub fn test() {
        assert!(result(point_var(new_span("$the:blasted"))).is_err());
    }
}

#[derive(Debug, Clone, Error)]
pub enum ErrCtx {
    #[error("Yikes!")]
    Yikes,
    #[error(transparent)]
    Var(#[from] VarErrCtx),
    #[error("{0}")]
    PointSeg(#[from] PointSegErrCtx),
    #[error("{0}")]
    InvalidBaseKind(String),
    #[error("{0}{1}")]
    InvalidSubKind(BaseKind, String),
    #[error("variable substitution is not supported in this context")]
    ResolverNotAvailable,
    #[error(transparent)]
    Primitive(#[from] PrimitiveErrCtx),
}

#[derive(Debug, Clone, Error)]
pub enum VarErrCtx {
    #[error("invalid variable declaration after '$'")]
    VarToken,
    #[error("invalid variable name. Legal values: alphanumeric + '_' (must start with a letter)")]
    VarName,
}

#[derive(Debug, Clone, Error)]
pub enum PointSegErrCtx {
    #[error("invalid Space PointSegment")]
    Space,
}

#[derive(Debug, Clone, Error)]
pub enum PrimitiveErrCtx {
    #[error("expecting alpha")]
    Alpha,
    #[error("expecting digit")]
    Digit,
    #[error("expecting upper case alpha")]
    Upper,
    #[error("expecting lower case alpha")]
    Lower,
    #[error("expecting lower case alpha numeric")]
    LowerAlphaNumeric,
    #[error("expecting lower case alpha numeric '.' and '-'")]
    Domain,
    #[error("expecting {0}")]
    Brace(#[from] BraceErrCtx),
    #[error("consecutive '..' dots not allowed")]
    ConsecutiveDots,
    #[error("error processing route")]
    RouteScopeTag,
}

#[derive(Debug, Clone, Error)]
pub struct BraceErrCtx {
    pub kind: BraceKindErrCtx,
    pub side: BraceSideErrCtx,
}

impl Into<ErrCtx> for BraceErrCtx {
    fn into(self) -> ErrCtx {
        ErrCtx::Primitive(self.into())
    }
}

impl BraceErrCtx {
    pub fn new(kind: BraceKindErrCtx, side: BraceSideErrCtx) -> Self {
        Self { kind, side }
    }

    pub fn describe(&self) -> &'static str {
        match &self.kind {
            BraceKindErrCtx::Curly => match &self.side {
                BraceSideErrCtx::Open => "{",
                BraceSideErrCtx::Close => "}",
            },
        }
    }
}

impl Display for BraceErrCtx {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} '{}'", self.side, self.kind, self.describe())
    }
}

#[derive(Debug, Clone, Error)]
pub enum BraceSideErrCtx {
    #[error("opening")]
    Open,
    #[error("closing")]
    Close,
}

#[derive(Debug, Clone, Error)]
pub enum BraceKindErrCtx {
    #[error("curly brace")]
    Curly,
}

impl BraceKindErrCtx {}

pub type NomErr<I: Span> = GenericErrorTree<I, &'static str, ErrCtx, ParseErrs>;


pub type Res<I: Span, O> = IResult<I, O, NomErr<I>>;

pub trait MyParser<I:Span,O> :Parser<I,O,NomErr<I>> {}

/*impl <I> From<SpaceTree<I>> for ParseErrs where I: Span {
    fn from(value: SpaceTree<I>) -> Self {
        ParseErrs::from(value).into()
    }
}

 */

/*
impl <I> From<nom::Err<SpaceTree<I>>> for ParseErrs where I: Span {
    fn from(value: nom::Err<SpaceTree<I>>) -> Self {
        ParseErrs::from(value).into()
    }
}

 */

/*
pub struct Parser {}

impl Parser {
    pub fn point(input: Span) -> Res<Span, Point> {
        point_subst(input)
    }

    pub fn consume_point(input: Span) -> Result<Point, ExtErr> {
        let (_, point) = all_consuming(point_subst)(input)?;
        Ok(point)
    }
}
 */

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

pub fn this_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    alt((recognize(tag(".")), recognize(not(other_route_segment))))(input)
        .map(|(next, _)| (next, RouteSeg::This))
}

pub fn local_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    tag("LOCAL")(input).map(|(next, _)| (next, RouteSeg::Local))
}

pub fn remote_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    tag("REMOTE")(input).map(|(next, _)| (next, RouteSeg::Remote))
}

pub fn global_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    tag("GLOBAL")(input).map(|(next, _)| (next, RouteSeg::Global))
}

pub fn domain_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    domain_chars(input).map(|(next, domain)| (next, RouteSeg::Domain(domain.to_string())))
}

pub fn tag_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    delimited(tag("#["), into(skewer_case), tag("]"))(input)
        .map(|(next, tag)| (next, RouteSeg::Tag(tag)))
}

pub fn sys_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    delimited(tag("<<"), sys_route_chars, tag(">>"))(input)
        .map(|(next, tag)| (next, RouteSeg::Star(tag.to_string())))
}

pub fn other_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    alt((
        sys_route_segment,
        tag_route_segment,
        domain_route_segment,
        global_route_segment,
        local_route_segment,
        remote_route_segment,
    ))(input)
}

pub fn point_route_segment<I: Span>(input: I) -> Res<I, RouteSeg> {
    alt((this_route_segment, other_route_segment))(input)
}

/*
pub fn point_segment(input: Span) -> Res<Span, PointSegCtx> {
    alt((
        base_point_segment,
        space_point_segment,
        version_point_segment,
        filesystem_point_segment,
        file_point_segment,
    ))(input)
}

 */

pub fn mesh_eos<I: Span>(input: I) -> Res<I, I> {
    peek(alt((tag(":"), eop)))(input)
}

pub fn mesh_kind_eos<I: Span>(input: I) -> Res<I, I> {
    peek(alt((tag(":"), eop)))(input)
}

pub fn fs_trailing<I: Span>(input: I) -> Res<I, I> {
    peek(pair(
        recognize(tag(":")),
        context("point:version:root_not_trailing", cut(tag("/"))),
    ))(input)
    .map(|(next, (rtn, _))| (next, rtn))
}

// version end of segment
pub fn ver_eos<I: Span>(input: I) -> Res<I, I> {
    peek(alt((fs_trailing, tag(":/"), eop)))(input)
}

// end of point
pub fn eop<I: Span>(input: I) -> Res<I, I> {
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

pub fn space_no_dupe_dots<I: Span>(input: I) -> Res<I, ()> {
    peek(cut(not(take_until(".."))))(input).map(|(next, _)| (next, ()))
}

pub fn space_point_segment<I: Span>(input: I) -> Res<I, PointSeg> {
    cut(terminated(
        recognize(pair(
            peek(lowercase1).context(PrimitiveErrCtx::Lower.into()),
            space_chars,
        ))
        .context(PrimitiveErrCtx::Domain.into()),
        mesh_eos.context(
            PrimitiveErrCtx::Brace(BraceErrCtx {
                kind: BraceKindErrCtx::Curly,
                side: BraceSideErrCtx::Open,
            })
            .into(),
        ),
    )
    .context(PointSegErrCtx::Space.into()))(input)
    .map(|(next, space)| (next, PointSeg::Space(space.to_string())))
}

pub fn base_point_segment<I: Span>(input: I) -> Res<I, PointSeg> {
    preceded(
        peek(lowercase1).context(PrimitiveErrCtx::Lower.into()),
        cut(pair(rec_skewer, mesh_eos)),
    )(input)
    .map(|(next, (base, _))| (next, PointSeg::Base(base.to_string())))
}

pub fn version_point_segment<I: Span>(input: I) -> Res<I, PointSeg> {
    preceded(
        peek(digit1),
        context("point:version_segment", cut(tuple((version, ver_eos)))),
    )(input)
    .map(|(next, (version, _))| (next, PointSeg::Version(version)))
}

pub fn dir_pop<I: Span>(input: I) -> Res<I, PointSegVar> {
    context("point:dir_pop", tuple((tag(".."), opt(tag("/")))))(input).map(|(next, _)| {
        (
            next.clone(),
            PointSegVar::Pop(Trace {
                range: next.location_offset() - 2..next.location_offset(),
                extra: next.extra(),
            }),
        )
    })
}

pub fn filesystem_point_segment<I: Span>(input: I) -> Res<I, PointSeg> {
    tuple((
        peek(not(eop)),
        context(
            "point:file_or_directory",
            cut(alt((dir_point_segment, file_point_segment))),
        ),
    ))(input)
    .map(|(next, (_, seg))| (next, seg))
}

pub fn dir_point_segment<I: Span>(input: I) -> Res<I, PointSeg> {
    context("point:dir_segment", file_chars)(input)
        .map(|(next, dir)| (next, PointSeg::Dir(dir.to_string())))
}

pub fn root_dir_point_segment<I: Span>(input: I) -> Res<I, PointSeg> {
    context("point:root_filesystem_segment", tag(":/"))(input)
        .map(|(next, _)| (next, PointSeg::FsRootDir))
}

pub fn root_dir_point_segment_ctx<I: Span>(input: I) -> Res<I, PointSegVar> {
    context("point:root_filesystem_segment", tag(":/"))(input)
        .map(|(next, _)| (next, PointSegVar::FilesystemRootDir))
}

pub fn root_dir_point_segment_var<I: Span>(input: I) -> Res<I, PointSegVar> {
    context("point:root_filesystem_segment", tag(":/"))(input)
        .map(|(next, _)| (next, PointSegVar::FilesystemRootDir))
}

pub fn file_point_segment<I: Span>(input: I) -> Res<I, PointSeg> {
    context("point:file_segment", file_chars)(input)
        .map(|(next, filename)| (next, PointSeg::File(filename.to_string())))
}

pub fn point_var<I: Span>(input: I) -> Res<I, PointVar> {
    context(
        "point",
        tuple((alt((root_point_var, point_non_root_var)), eop)),
    )(input.clone())
    .map(|(next, (point, _))| (next, point))
}

/*
pub fn var<O,F,P>(mut f: F ) -> impl FnMut(I) -> Res<I,Var<O,P>> where F: Parser<I,O,ErrorTree<I>>, P: VarParser<O> {
    move | input: I | {
        let result = recognize(pair(peek(tag("$")),context("var",cut(delimited(tag("${"), skewer_case, tag("}") )))))(input.clone());
        match result {
            Ok((next,var)) => {
                Ok( (next, Var::Var{ name: var.to_string(), parser: f }) )
            }
            Err(err) => {
                match &err {
                    Err::Incomplete(_) => {
                        Err(err)
                    }
                    Err::Failure(_) => {
                        Err(err)
                    }
                    // in this case the peek failed which means it is not a variable declaration
                    Err::Error(_) => {
                        let input = to_owned_span(&input);
                        f.parse(input)
                    }
                }
            }
        }


        let input = to_owned_span(&input);

        input.parse(input)
    }
}

 */

fn val<I: Span, O, F>(f: F) -> impl FnMut(I) -> Res<I, VarVal<O>>
where
    F: FnMut(I) -> Res<I, O> + Copy,
{
    move |input| tw(f)(input).map(|(next, val)| (next, VarVal::Val(val)))
}

fn var<I: Span, O>(input: I) -> Res<I, VarVal<O>> {
    pair(
        peek(tag("$")),
        cut(delimited(
            tag("${")
                .context(BraceErrCtx::new(BraceKindErrCtx::Curly, BraceSideErrCtx::Open).into()),
            tw(var_name),
            tag("}")
                .context(BraceErrCtx::new(BraceKindErrCtx::Curly, BraceSideErrCtx::Close).into()),
        )),
    )(input)
    .map(|(next, (_, var))| (next, VarVal::Var(var)))
}

#[cfg(test)]
pub mod test3 {
    use crate::parse::util::{new_span, print, trim};
    use crate::parse::{point_var, Res};
    use crate::point::PointVar;
    use nom::combinator::cut;
    use nom_supreme::ParserExt;

    #[test]
    pub fn test() {
        /*
        let span = new_span("\n\n        ${the }\n");
        //let result: Res<_,PointSegVar>   = variable_ize(pop(base_point_segment))(span);
        let result: Res<_,PointVar>   = cut(trim(point_var))(span);

        match result.unwrap_err() {
            nom::Err::Incomplete(_) => {
                assert!(false)
            }
            nom::Err::Error(_) => {
                assert!(false)
            }
            nom::Err::Failure(err) => {
                print(&err);
            }
        }


         */

        let span = new_span("\n\n\n\n        yHadron\n");
        //let result: Res<_,PointSegVar>   = variable_ize(pop(base_point_segment))(span);
        let result: Res<_, PointVar> = cut(trim(point_var))(span);

        match result.unwrap_err() {
            nom::Err::Incomplete(_) => {
                assert!(false)
            }
            nom::Err::Error(_) => {
                assert!(false)
            }
            nom::Err::Failure(err) => {
                print(&err);
            }
        }
    }
}

fn var_name<I>(input: I) -> Res<I, VarCase>
where
    I: Span,
{
    var_chars(input).map(|(next, var)| {
        (
            next,
            VarCase {
                string: var.to_string(),
            },
        )
    })
}

/*
pub fn point_seg_var<F, I: Span>(mut f: F) -> impl FnMut(I) -> Res<I, PointSegVar> + Copy
where
    F: Parser<I, PointSeg, ParseTree<I>> + Copy,
{
    move |input: I| {
         var_decl(f)(input).map(|(next,v)| (next,v))
    }
}

 */

/*


*/

fn point_var_seg<I, F>(mut f: F) -> impl FnMut(I) -> Res<I, PointSegVar> + Copy
where
    F: FnMut(I) -> Res<I, PointSegCtx> + Copy,
    I: Span,
{
    move |input: I| variable_ize(f)(input)
}

pub fn variable_ize<F, I, O, T>(mut f: F) -> impl FnMut(I) -> Res<I, O> + Copy
where
    F: FnMut(I) -> Res<I, T> + Copy,
    I: Span,
    O: From<VarVal<T>>,
{
    move |input: I| var_or_val(f)(input).map(|(next, varval)| (next, O::from(varval)))
}
/*
pub fn var_seg<F, I: Span>(mut f: F) -> impl FnMut(I) -> Res<I, PointSegVar> + Copy
where
    F: Parser<I, PointSegCtx, ParseTree<I>> + Copy,
{
    move |input: I| {
        let offset = input.location_offset();
        let result = pair(
            peek(tag("$")),
            context(
                "var",
                cut(delimited(tag("${"), skewer_case_chars, tag("}"))),
            ),
        )(input.clone());

        match result {
            Ok((next, (_, var))) => {
                let range = Range {
                    start: offset,
                    end: next.location_offset(),
                };
                let trace = Trace {
                    range,
                    extra: next.extra(),
                };
                let var = Variable::new(var.to_string(), trace);
                Ok((next, PointSegVar::Var(var)))
            }
            Err(err) => match err {
                Err::Incomplete(needed) => return Err(nom::Err::Incomplete(needed)),
                Err::Failure(err) => return Err(nom::Err::Failure(err)),
                Err::Error(_) => f.parse(input).map(|(next, seg)| (next, seg.into())),
            },
        }
    }
}

 */

/*fn variable<I,F,O>(input: I) -> impl FnMut(I) -> Res<I,Variable> where I: Span
{
    let offset = input.location_offset();
    let result = var_name(input.clone());

    match result {
        Ok((next, name)) => {
            let range = Range {
                start: offset,
                end: next.location_offset(),
            };
            let trace = Trace {
                range,
                extra: next.extra(),
            };
            let var = Variable::new(name, trace);
            Ok((next, var))
        }
        Err(err) => match err {
            Err::Incomplete(needed) => nom::Err::Incomplete(needed),
            Err::Failure(err) => nom::Err::Error(err),
            Err::Error(err) => nom::Err::Error(err)
        },
    }
}

 */

fn var_or_val<I, F, O>(mut f: F) -> impl FnMut(I) -> Res<I, VarVal<O>>
where
    F: FnMut(I) -> Res<I, O> + Copy,
    I: Span,
{
    move |input: I| alt((var, val(f)))(input)
}

/*
// scan for a Var declaration ${} or a cont var_or_val<'a, F, I: SpO>(mut f: F) -> impl FnMut(I) -> R, VarVal<O>>re F: Parser<I, O, ParseTree<I>>,   move |input:{     alt( (var,val(f)) put) }

 */

pub fn var_route<'a, F, I: Span>(mut f: F) -> impl FnMut(I) -> Res<I, RouteSegVar>
where
    F: FnMut(I) -> Res<I, RouteSeg> + Copy,
{
    move |input: I| variable_ize(f)(input)
}

pub fn root_point_var<I: Span>(input: I) -> Res<I, PointVar> {
    context(
        "root_point",
        tuple((
            opt(terminated(var_route(point_route_segment), tag("::"))),
            tag("ROOT"),
        )),
    )(input)
    .map(|(next, (route, _))| {
        let route = route.unwrap_or(RouteSegVar::This);
        let point = PointVar {
            route,
            segments: vec![],
        };
        (next, point)
    })
}

pub fn point_non_root_var<I: Span>(input: I) -> Res<I, PointVar> {
    context(
        "point_non_root",
        tuple((
            opt(terminated(var_route(point_route_segment), tag("::")))
                .context(PrimitiveErrCtx::RouteScopeTag.into()),
            point_var_seg(root_ctx_seg(space_point_segment)),
            many0(base_seg(point_var_seg(pop(base_point_segment)))),
            opt(base_seg(point_var_seg(pop(version_point_segment)))),
            opt(tuple((
                root_dir_point_segment_var,
                many0(recognize(tuple((
                    point_var_seg(pop(dir_point_segment)),
                    tag("/"),
                )))),
                opt(point_var_seg(pop(file_point_segment))),
                eop,
            ))),
            eop,
        )),
    )(input)
    .map(
        |(next, (route, space, mut bases, version, filesystem, _))| {
            let route = route.unwrap_or(RouteSegVar::This);
            let mut segments = vec![];
            let mut bases: Vec<PointSegVar> = bases;
            segments.push(space);
            segments.append(&mut bases);
            match version {
                None => {}
                Some(version) => {
                    segments.push(version);
                }
            }

            if let Option::Some((fsroot, mut dirs, file, _)) = filesystem {
                let mut dirs: Vec<PointSegVar> = dirs
                    .into_iter()
                    .map(|i| PointSegVar::Dir(i.to_string()))
                    .collect();
                segments.push(fsroot);
                segments.append(&mut dirs);
                if let Some(file) = file {
                    segments.push(file);
                }
            }

            let point = PointVar { route, segments };

            (next, point)
        },
    )
}

pub fn consume_point(input: &str) -> Result<Point, ParseErrs> {
    consume_point_ctx(input)?.collapse()
}

pub fn consume_point_ctx(input: &str) -> Result<PointCtx, ParseErrs> {
    consume_point_var(input)?.collapse()
}

pub fn consume_point_var(input: &str) -> Result<PointVar, ParseErrs> {
    let span = new_span(input);
    let point = result(context("consume", all_consuming(point_var))(span))?;
    Ok(point)
}

/*
pub fn point_old(input: Span) -> Res<Span, Point> {
    let (next, point) = point_subst(input.clone())?;
    match point.brute_resolve() {
        Ok(point) => Ok((next, point)),
        Err(err) => {
            let e = ErrorTree::from_error_kind(input.clone(), ErrorKind::Fail);
            let e = ErrorTree::add_context(input, "point-subst-brute-force", e);
            return Err(nom::Err::Failure(e));
        }
    }
}

 */

/*pub fn capture_point(input: Span) -> Res<Span, CaptureAddress> {
    context(
        "Address",
        tuple((
            tuple((
                point_route_segment,
                alt((root_point_capture_segment, space_point_capture_segment)),
            )),
            many0(base_point_capture_segment),
            opt(version_point_segment),
            opt(root_dir_point_segment),
            many0(filesystem_point_capture_segment),
        )),
    )(input)
    .map(
        |(next, ((hub, space), mut bases, version, root, mut files))| {
            let mut segments = vec![];
            segments.push(space);
            segments.append(&mut bases);
            match version {
                None => {}
                Some(version) => {
                    segments.push(version);
                }
            }

            if let Option::Some(root) = root {
                segments.push(root);
                segments.append(&mut files);
            }

            let point = CaptureAddress {
                route: hub,
                segments,
            };

            (next, point)
        },
    )
}


pub fn root_point_capture_segment(input: Span) -> Res<Span, PointSeg> {
    tag("ROOT")(input).map(|(next, space)| (next, PointSeg::Root))
}

pub fn space_point_capture_segment(input: Span) -> Res<Span, PointSeg> {
    space_chars_plus_capture(input).map(|(next, space)| (next, PointSeg::Space(space.to_string())))
}

pub fn base_point_capture_segment(input: Span) -> Res<Span, PointSeg> {
    preceded(tag(":"), rec_skewer_capture)(input)
        .map(|(next, base)| (next, PointSeg::Base(base.to_string())))
}

pub fn filesystem_point_capture_segment(input: Span) -> Res<Span, PointSeg> {
    alt((dir_point_capture_segment, file_point_capture_segment))(input)
}

pub fn dir_point_capture_segment(input: Span) -> Res<Span, PointSeg> {
    context(
        "dir_point_capture_segment",
        terminated(file_chars_plus_capture, tag("/")),
    )(input)
    .map(|(next, dir)| (next, PointSeg::Dir(format!("{}/", dir))))
}

pub fn file_point_capture_segment(input: Span) -> Res<Span, PointSeg> {
    context("file_point_capture_segment", file_chars_plus_capture)(input)
        .map(|(next, filename)| (next, PointSeg::File(filename.to_string())))
}
 */

pub fn space_point_kind_segment<I: Span>(input: I) -> Res<I, PointKindSeg> {
    pair(space_point_segment, delim_kind)(input).map(|(next, (point_segment, kind))| {
        (
            next,
            PointKindSeg {
                segment: point_segment,
                kind,
            },
        )
    })
}

pub fn base_point_kind_segment<I: Span>(input: I) -> Res<I, PointKindSeg> {
    tuple((base_point_segment, delim_kind))(input).map(|(next, (point_segment, kind))| {
        (
            next,
            PointKindSeg {
                segment: point_segment,
                kind,
            },
        )
    })
}

pub fn filepath_point_kind_segment<I: Span>(input: I) -> Res<I, PointKindSeg> {
    alt((file_point_kind_segment, dir_point_kind_segment))(input)
}
pub fn dir_point_kind_segment<I: Span>(input: I) -> Res<I, PointKindSeg> {
    tuple((dir_point_segment, delim_kind))(input).map(|(next, (point_segment, kind))| {
        (
            next,
            PointKindSeg {
                segment: point_segment,
                kind,
            },
        )
    })
}

pub fn file_point_kind_segment<I: Span>(input: I) -> Res<I, PointKindSeg> {
    tuple((file_point_segment, delim_kind))(input).map(|(next, (point_segment, kind))| {
        (
            next,
            PointKindSeg {
                segment: point_segment,
                kind,
            },
        )
    })
}

pub fn file_root_kind_segment<I: Span>(input: I) -> Res<I, PointKindSeg> {
    tuple((tag(":/"), delim_kind))(input).map(|(next, (point_segment, kind))| {
        (
            next,
            PointKindSeg {
                segment: PointSeg::FsRootDir,
                kind,
            },
        )
    })
}

pub fn version_point_kind_segment<I: Span>(input: I) -> Res<I, PointKindSeg> {
    tuple((version_point_segment, delim_kind))(input).map(|(next, (point_segment, kind))| {
        (
            next,
            PointKindSeg {
                segment: point_segment,
                kind,
            },
        )
    })
}

pub fn consume_hierarchy<I: Span>(input: I) -> Result<PointHierarchy, ParseErrs> {
    let (next, rtn) = all_consuming(point_kind_hierarchy)(input)?;
    Ok(rtn)
}

pub fn point_kind_hierarchy<I: Span>(input: I) -> Res<I, PointHierarchy> {
    tuple((
        opt(terminated(point_route_segment, tag("::")))
            .context(PrimitiveErrCtx::RouteScopeTag.into()),
        terminated(space_point_kind_segment, tag(":")),
        separated_list0(tag(":"), base_point_kind_segment),
        opt(preceded(tag(":"), version_point_kind_segment)),
        opt(file_root_kind_segment),
        many0(terminated(dir_point_kind_segment, tag("/"))),
        opt(file_point_kind_segment),
    ))(input)
    .map(
        |(next, (route_seg, space, mut bases, version, file_root, mut dirs, file))| {
            let mut segments: Vec<PointKindSeg> = vec![];
            segments.push(space);
            segments.append(&mut bases);
            match version {
                None => {}
                Some(version) => {
                    segments.push(version);
                }
            }

            let route_seg = match route_seg {
                None => RouteSeg::Local,
                Some(route_seg) => route_seg,
            };

            if file_root.is_some() {
                segments.push(PointKindSeg {
                    segment: PointSeg::FsRootDir,
                    kind: Kind::FileStore,
                });
            }

            segments.append(&mut dirs);

            if let Some(file) = file {
                segments.push(file);
            }

            let point = PointHierarchy::new(route_seg, segments);

            (next, point)
        },
    )
}

pub fn asterisk<T: Span>(input: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    input.split_at_position_complete(|item| item.as_char() != '*')
}

pub fn upper<T>(input: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    input.split_at_position_complete(|item| {
        let char_item = item.as_char();

        !char_item.is_uppercase()
    })
}

/*    fn any_resource_path_segment<T>(i: T) -> Res<T, T>
       where
           T: InputTakeAtPosition+nom::InputLength,
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

*/

pub fn in_double_quotes<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            char_item == '\"'
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn skewer_colon<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == ':')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn skewer_dot<I: Span>(i: I) -> Res<I, I>
where
    I: InputTakeAtPosition + nom::InputLength,
    <I as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn domain<I: Span>(i: I) -> Res<I, Domain> {
    domain_chars(i).map(|(next, domain)| {
        (
            next,
            Domain {
                string: domain.to_string(),
            },
        )
    })
}

pub fn point_segment_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn version_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            char_item != '.'
                && char_item != '-'
                && !char_item.is_digit(10)
                && !(char_item.is_alpha() && char_item.is_lowercase())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn version_req_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '>')
                && !(char_item == '<')
                && !(char_item == '^')
                && !(char_item == '=')
                && !(char_item == '.')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn lowercase1<I>(i: I) -> Res<I, I>
where
    I: Span,
{
    nomplus::lowercase1(i)
}

pub fn rec_skewer<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((lowercase1, opt(skewer))))(input)
}

pub fn rec_skewer_capture<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((lowercase1, opt(skewer_chars_plus_capture))))(input)
}

pub fn camel_chars<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength + Clone + Offset + Slice<RangeTo<usize>>,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    recognize(pair(upper, alphanumeric0))(i)
}

pub fn skewer_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            char_item != '-'
                && !char_item.is_digit(10)
                && !(char_item.is_alpha() && char_item.is_lowercase())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn parse_uuid<I: Span>(i: I) -> Res<I, Uuid> {
    let (next, uuid) = uuid_chars(i.clone())?;
    Ok((
        next,
        Uuid::from(uuid)
            .map_err(|e| nom::Err::Error(NomErr::from_error_kind(i, ErrorKind::Tag)))?,
    ))
}

pub fn uuid_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    // same as skewer_chars
    skewer_chars(i)
}

pub fn skewer_chars_plus_capture<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            char_item != '-'
                && char_item != '$'
                && !char_item.is_digit(10)
                && !(char_item.is_alpha() && char_item.is_lowercase())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn skewer_chars_template<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            char_item != '-'
                && char_item.as_char() != '%'
                && !char_item.is_digit(10)
                && !(char_item.is_alpha() && char_item.is_lowercase())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn space_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn space_chars_plus_capture<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !(char_item == '$')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn domain_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn path_regex<I: Span>(input: I) -> Res<I, I> {
    let (next, regex_span) = context("regex", recognize(pair(tag("/"), nospace0)))(input.clone())?;

    let regex_string = regex_span.to_string();
    match Regex::new(regex_string.as_str()) {
        Ok(regex) => Ok((next, regex_span)),
        Err(err) => {
            println!("regex error {}", err.to_string());
            return Err(nom::Err::Error(NomErr::from_error_kind(
                input,
                ErrorKind::Tag,
            )));
        }
    }
}

pub fn regex<T: Span>(i: T) -> Res<T, T>
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
                && !(char_item == ':')
                && !(char_item == '_')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn filepath_chars<T: Span>(i: T) -> Res<T, T>
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
                && !(char_item == ':')
                && !(char_item == '_')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn file_chars_plus_capture<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !(char_item == '_')
                && !(char_item == '$')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn file_chars<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !(char_item == '_')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn file_chars_template<T: Span>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '.')
                && !(char_item == '_')
                && !(char_item == '%')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn not_space<I: Span>(input: I) -> Res<I, I> {
    is_not(" \n\r\t")(input)
}

pub fn path<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((tag("/"), opt(filepath_chars))))(input)
}

pub fn subst_path<I: Span>(input: I) -> Res<I, Subst<I>> {
    pair(peek(tag("/")), subst(filepath_chars))(input).map(|(next, (_, path))| (next, path))
}

pub fn consume_path<I: Span>(input: I) -> Res<I, I> {
    all_consuming(path)(input)
}

#[derive(
    Debug, Clone, Eq, PartialEq, Hash, SerializeDisplay, DeserializeFromStr, derive_name::Name,
)]
pub struct CamelCase {
    string: String,
}

impl CamelCase {
    pub fn as_str(&self) -> &str {
        self.string.as_str()
    }
}


/*
impl <E> TryInto<E> for CamelCase where E: TryInto<String> {
    type Error = ();

    fn try_into(self) -> Result<E, Self::Error> {
        self.string.try_into().map_err(|_| ())
    }
}


 */


impl FromStr for CamelCase {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(camel_case)(new_span(s)))
    }
}

/*

impl Serialize for CamelCase {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.string.as_str())
    }
}

impl<'de> Deserialize<'de> for CamelCase {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;

        let result = result(camel_case(new_span(string.as_str())));
        match result {
            Ok(camel) => Ok(camel),
            Err(err) => Err(serde::de::Error::custom(err.to_string().as_str())),
        }
    }
}

 */

impl Display for CamelCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for CamelCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

/// this mapping may not be totally correct.... could the string "localhost" pass
/// as a [Domain] ?  Will return if problems arise
pub type Hostname = Domain;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Domain {
    string: String,
}

impl TzoParser for Domain {
    fn inner<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        domain(input)
    }
}

impl Serialize for Domain {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.string.as_str())
    }
}

impl<'de> Deserialize<'de> for Domain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;

        let result = result(domain(new_span(string.as_str())));
        match result {
            Ok(domain) => Ok(domain),
            Err(err) => Err(serde::de::Error::custom(err.to_string())),
        }
    }
}

impl FromStr for Domain {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(domain)(new_span(s)))
    }
}

impl Display for Domain {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for Domain {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct SkewerCase {
    string: String,
}

impl TzoParser for SkewerCase {
    fn inner<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        skewer_case(input)
    }
}









pub struct SnakeCase {
    string: String,
}

pub type DbCase = VarCase;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct VarCase {
    string: String,
}

impl Serialize for VarCase {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.string.as_str())
    }
}

impl<'de> Deserialize<'de> for VarCase {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;

        let result = result(var_case(new_span(string.as_str())));
        match result {
            Ok(var) => Ok(var),
            Err(err) => Err(serde::de::Error::custom(err.to_string())),
        }
    }
}

impl FromStr for VarCase {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(var_chars)(new_span(s)))?;
        Ok(Self {
            string: s.to_string(),
        })
    }
}

impl Display for VarCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for VarCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}

impl Serialize for SkewerCase {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.string.as_str())
    }
}

impl<'de> Deserialize<'de> for SkewerCase {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string = String::deserialize(deserializer)?;

        let result = result(skewer_case(new_span(string.as_str())));
        match result {
            Ok(skewer) => Ok(skewer),
            Err(err) => Err(serde::de::Error::custom(err.to_string())),
        }
    }
}

impl FromStr for SkewerCase {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        result(all_consuming(skewer_case)(new_span(s)))
    }
}

impl Display for SkewerCase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.string.as_str())
    }
}

impl Deref for SkewerCase {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.string
    }
}


/*
pub fn from<I,Fn,In,Out>(mut f: Fn) -> impl FnMut(I) -> Res<I, Out> where Fn: FnMut(I) -> Res<I,In>+Copy, Out: From<In>, I: Span {
    move |input| {
        f(input).map(|(next,t)|(next,Out::from(t)))
    }
}

 */


pub fn from_camel<I,O>(input:I) -> Res<I,O> where I: Span, O: From<CamelCase>{
    into(camel_case)(input)
}

pub fn from_skewer<I,O>(input:I) -> Res<I,O> where I: Span, O: From<SkewerCase>{
    into(skewer_case)(input)
}

pub fn camel_case<I: Span>(input: I) -> Res<I, CamelCase> {
    context("expect-camel-case", camel_case_chars)(input).map(|(next, camel_case_chars)| {
        (
            next,
            CamelCase {
                string: camel_case_chars.to_string(),
            },
        )
    })
}

pub fn skewer_case<I: Span>(input: I) -> Res<I, SkewerCase> {
    context("expect-skewer-case", skewer_case_chars)(input).map(|(next, skewer_case_chars)| {
        (
            next,
            SkewerCase {
                string: skewer_case_chars.to_string(),
            },
        )
    })
}

pub fn var_case<I: Span>(input: I) -> Res<I, VarCase> {
    var_chars(input).map(|(next, var_case_chars)| {
        (
            next,
            VarCase {
                string: var_case_chars.to_string(),
            },
        )
    })
}

pub fn camel_case_chars<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((is_a("ABCDEFGHIJKLMNOPQRSTUVWXYZ"), alphanumeric0)))(input)
}

pub fn skewer_case_chars<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((
        is_a("abcdefghijklmnopqrstuvwxyz"),
        many0(alt((alphanumeric1, tag("-")))),
    )))(input)
}

fn var_chars<I: Span>(input: I) -> Res<I, I> {
    recognize(
        pair(alpha1, many0(alt((alphanumeric1, tag("_"))))).context(VarErrCtx::VarName.into()),
    )(input)
}

#[cfg(test)]
#[test]
fn test_varcars() {
    log_parse_err(var_chars(new_span("_blah")));
}

pub fn lowercase_alphanumeric<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((lowercase1, alphanumeric0)))(input)
}

pub fn single_lowercase<T: Span, Input, Error: ParseError<Input>>(
    arr: T,
) -> impl Fn(Input) -> IResult<Input, Input, Error>
where
    Input: InputTakeAtPosition,
    T: FindToken<<Input as InputTakeAtPosition>::Item>,
{
    move |i: Input| {
        let e: ErrorKind = ErrorKind::IsA;
        i.split_at_position1_complete(|c| !arr.find_token(c), e)
    }
}

pub fn single_lowerscase<I: Span>(input: I) -> Res<I, I> {
    is_a("abcdefghijklmnopqrstuvwxyz")(input)
}
pub fn single_digit<I: Span>(input: I) -> Res<I, I> {
    is_a("abcdefghijklmnopqrstuvwxyz")(input)
}

pub fn camel_case_to_string_matcher<I: Span>(input: I) -> Res<I, StringMatcher> {
    camel_case_chars(input).map(|(next, camel)| (next, StringMatcher::new(camel.to_string())))
}

fn parse_version_major_minor_patch<I: Span>(input: I) -> Res<I, (I, I, I)> {
    context(
        "version_major_minor_patch",
        tuple((
            terminated(digit1, tag(".")),
            terminated(digit1, tag(".")),
            terminated(digit1, not(digit1)),
        )),
    )(input)
}

pub fn parse_version<I: Span>(input: I) -> Res<I, ((I, I, I), Option<I>)> {
    tuple((
        parse_version_major_minor_patch,
        opt(preceded(tag("-"), skewer_chars)),
    ))(input)
}

pub fn rec_version<I: Span>(input: I) -> Res<I, I> {
    recognize(parse_version)(input)
}

pub fn base_point_segment_wildcard<I: Span>(input: I) -> Res<I, PointTemplateSeg> {
    preceded(
        tag(":"),
        recognize(tuple((many0(skewer), tag("%"), many0(skewer)))),
    )(input)
    .map(|(next, base)| (next, PointTemplateSeg::Wildcard(base.to_string())))
}

pub fn base_point_segment_template<I: Span>(input: I) -> Res<I, PointTemplateSeg> {
    preceded(tag(":"), rec_skewer)(input).map(|(next, base)| {
        (
            next,
            PointTemplateSeg::ExactSeg(PointSeg::Base(base.to_string())),
        )
    })
}

pub fn filepath_point_segment_wildcard<I: Span>(input: I) -> Res<I, PointTemplateSeg> {
    recognize(tuple((
        many0(filepath_chars),
        tag("%"),
        many0(filepath_chars),
    )))(input)
    .map(|(next, base)| (next, PointTemplateSeg::Wildcard(base.to_string())))
}

pub fn filepath_point_segment_template<I: Span>(input: I) -> Res<I, PointTemplateSeg> {
    filesystem_point_segment(input)
        .map(|(next, segment)| (next, PointTemplateSeg::ExactSeg(segment)))
}

/*pub fn point_template<I: Span>(input: I) -> Res<I, PointTemplate> {
    let (next, ((hub, space), mut bases, version, root, mut files)) = tuple((
        tuple((point_route_segment, space_point_segment)),
        many0(alt((
            base_point_segment_wildcard,
            base_point_segment_template,
        ))),
        opt(version_point_segment),
        opt(root_dir_point_segment),
        many0(alt((
            filepath_point_segment_wildcard,
            filepath_point_segment_template,
        ))),
    ))(input.clone())?;

    let mut base_wildcard = false;
    for (index, segment) in bases.iter().enumerate() {
        if segment.is_wildcard() {
            if index != bases.len() - 1 {
                return Err(nom::Err::Error(ErrorTree::from_error_kind(
                    input,
                    ErrorKind::Tag,
                )));
            } else {
                base_wildcard = true;
            }
        }
    }

    if base_wildcard && version.is_some() {
        return Err(nom::Err::Error(ErrorTree::from_error_kind(
            input,
            ErrorKind::Tag,
        )));
    }

    if base_wildcard && root.is_some() {
        return Err(nom::Err::Error(ErrorTree::from_error_kind(
            input,
            ErrorKind::Tag,
        )));
    }

    let mut files_wildcard = false;
    for (index, segment) in files.iter().enumerate() {
        if segment.is_wildcard() {
            if index != files.len() - 1 {
                return Err(nom::Err::Error(ErrorTree::from_error_kind(
                    input.clone(),
                    ErrorKind::Tag,
                )));
            } else {
                files_wildcard = true;
            }
        }
    }

    let mut space_last = false;
    let last = if !files.is_empty() {
        match files.remove(files.len() - 1) {
            PointTemplateSeg::ExactSeg(exact) => PointSegFactory::Exact(exact.to_string()),
            PointTemplateSeg::Wildcard(pattern) => PointSegFactory::Pattern(pattern),
        }
    } else if root.is_some() {
        PointSegFactory::Exact("/".to_string())
    } else if let Option::Some(version) = &version {
        PointSegFactory::Exact(version.to_string())
    } else if !bases.is_empty() {
        match bases.remove(bases.len() - 1) {
            PointTemplateSeg::ExactSeg(exact) => PointSegFactory::Exact(exact.to_string()),
            PointTemplateSeg::Wildcard(pattern) => PointSegFactory::Pattern(pattern),
        }
    } else {
        space_last = true;
        PointSegFactory::Exact(space.to_string())
    };

    let mut bases: Vec<PointSeg> = bases
        .into_iter()
        .map(|b| match b {
            PointTemplateSeg::ExactSeg(seg) => seg,
            PointTemplateSeg::Wildcard(_) => {
                panic!("should have filtered wildcards already!")
            }
        })
        .collect();

    let mut files: Vec<PointSeg> = files
        .into_iter()
        .map(|b| match b {
            PointTemplateSeg::ExactSeg(seg) => seg,
            PointTemplateSeg::Wildcard(_) => {
                panic!("should have filtered wildcards already!")
            }
        })
        .collect();

    let mut segments = vec![];

    if !space_last {
        segments.push(space);
    }

    segments.append(&mut bases);

    match version {
        None => {}
        Some(version) => {
            segments.push(version);
        }
    }

    if let Option::Some(root) = root {
        segments.push(root);
        segments.append(&mut files);
    }

    let point = Point {
        route: hub,
        segments,
    };

    let point_template = PointTemplate {
        parent: point,
        child_segment_template: last,
    };

    Ok((next, point_template))
}

 */

pub fn point_template<I: Span>(input: I) -> Res<I, PointTemplateVar> {
    let (next, (point, wildcard)) = pair(point_var, opt(recognize(tag("%"))))(input.clone())?;

    if point.is_root() {
        return Ok((
            next,
            PointTemplateVar {
                parent: point,
                child_segment_template: PointSegTemplate::Root,
            },
        ));
    }

    let parent = point
        .parent()
        .expect("expect that point template has a parent");
    let child = point
        .last_segment()
        .expect("expect that point template has a last segment");

    match wildcard {
        None => Ok((
            next,
            PointTemplateVar {
                parent,
                child_segment_template: PointSegTemplate::Exact(child.to_string()),
            },
        )),
        Some(_) => {
            let child = format!("{}%", child.to_string());
            Ok((
                next,
                PointTemplateVar {
                    parent,
                    child_segment_template: PointSegTemplate::Exact(child),
                },
            ))
        }
    }
}

pub fn kind_template<I: Span>(input: I) -> Res<I, KindTemplate> {
    tuple((
        base_kind,
        opt(delimited(
            tag("<"),
            tuple((
                camel_case,
                opt(delimited(tag("<"), specific_selector, tag(">"))),
            )),
            tag(">"),
        )),
    ))(input)
    .map(|(next, (kind, more))| {
        let mut parts = KindTemplate {
            base: kind,
            sub: None,
            specific: None,
        };

        match more {
            Some((sub, specific)) => {
                parts.sub = Option::Some(sub);
                parts.specific = specific;
            }
            None => {}
        }

        (next, parts)
    })
}

pub fn template<I: Span>(input: I) -> Res<I, TemplateVar> {
    tuple((point_template, delimited(tag("<"), kind_template, tag(">"))))(input)
        .map(|(next, (point, kind))| (next, TemplateVar { point, kind }))
}

pub fn set_property_mod<I: Span>(input: I) -> Res<I, PropertyMod> {
    tuple((tag("+"), skewer_dot, tag("="), property_value))(input).map(
        |(next, (_, key, _, value))| {
            (
                next,
                PropertyMod::Set {
                    key: key.to_string(),
                    value: value.to_string(),
                    lock: false,
                },
            )
        },
    )
}

pub fn set_property_mod_lock<I: Span>(input: I) -> Res<I, PropertyMod> {
    tuple((tag("+@"), skewer_dot, tag("="), property_value))(input).map(
        |(next, (_, key, _, value))| {
            (
                next,
                PropertyMod::Set {
                    key: key.to_string(),
                    value: value.to_string(),
                    lock: true,
                },
            )
        },
    )
}

pub fn property_value_not_space_or_comma<I: Span>(input: I) -> Res<I, I> {
    is_not(" \n\r\t,")(input)
}

pub fn property_value_single_quotes<I: Span>(input: I) -> Res<I, I> {
    delimited(tag("'"), is_not("'"), tag("'"))(input)
}

pub fn property_value_double_quotes<I: Span>(input: I) -> Res<I, I> {
    delimited(tag("\""), is_not("\""), tag("\""))(input)
}

pub fn property_value<I: Span>(input: I) -> Res<I, I> {
    alt((
        property_value_single_quotes,
        property_value_double_quotes,
        property_value_not_space_or_comma,
    ))(input)
}

pub fn unset_property_mod<I: Span>(input: I) -> Res<I, PropertyMod> {
    tuple((tag("!"), skewer_dot))(input)
        .map(|(next, (_, name))| (next, PropertyMod::UnSet(name.to_string())))
}

pub fn property_mod<I: Span>(input: I) -> Res<I, PropertyMod> {
    alt((set_property_mod, unset_property_mod))(input)
}

pub fn set_properties<I: Span>(input: I) -> Res<I, SetProperties> {
    separated_list0(tag(","), tuple((multispace0, property_mod, multispace0)))(input).map(
        |(next, properties)| {
            let mut set_properties = SetProperties::new();
            for (_, property, _) in properties {
                set_properties.push(property);
            }
            (next, set_properties)
        },
    )
}

pub fn get_properties<I: Span>(input: I) -> Res<I, Vec<String>> {
    separated_list0(tag(","), tuple((multispace0, skewer, multispace0)))(input).map(
        |(next, keys)| {
            let keys: Vec<String> = keys.iter().map(|(_, key, _)| key.to_string()).collect();
            (next, keys)
        },
    )
}

pub fn create<I: Span>(input: I) -> Res<I, CreateVar> {
    tuple((
        opt(alt((
            value(Strategy::Override, tag("!")),
            value(Strategy::Ensure, tag("?")),
        ))),
        space1,
        template,
        opt(delimited(tag("{"), set_properties, tag("}"))),
    ))(input)
    .map(|(next, (strategy, _, template, properties))| {
        let strategy = match strategy {
            None => Strategy::Commit,
            Some(strategy) => strategy,
        };
        let properties = match properties {
            Some(properties) => properties,
            None => SetProperties::new(),
        };
        let create = CreateVar {
            template,
            state: StateSrcVar::None,
            properties,
            strategy,
        };
        (next, create)
    })
}

pub fn set<I: Span>(input: I) -> Res<I, SetVar> {
    tuple((point_var, delimited(tag("{"), set_properties, tag("}"))))(input).map(
        |(next, (point, properties))| {
            let set = SetVar { point, properties };
            (next, set)
        },
    )
}

pub fn get<I: Span>(input: I) -> Res<I, GetVar> {
    tuple((
        point_var,
        opt(delimited(tag("{"), get_properties, tag("}"))),
    ))(input)
    .map(|(next, (point, keys))| {
        let op = match keys {
            None => GetOp::State,
            Some(keys) => GetOp::Properties(keys),
        };
        let get = GetVar { point, op };

        (next, get)
    })
}

pub fn select<I: Span>(input: I) -> Res<I, SelectVar> {
    point_selector(input).map(|(next, point_kind_pattern)| {
        let select = SelectVar {
            pattern: point_kind_pattern,
            properties: Default::default(),
            into_substance: SelectIntoSubstance::Stubs,
            kind: SelectKind::Initial,
        };
        (next, select)
    })
}

pub fn publish<I: Span>(input: I) -> Res<I, CreateVar> {
    let (next, (upload, _, point)) = tuple((upload_block, space1, point_template))(input.clone())?;

    /*
    let parent = match point.parent() {
        None => {
            return Err(nom::Err::Error(ErrorTree::from_error_kind(
                input,
                ErrorKind::Tag,
            )));
        }
        Some(parent) => parent,
    };

    let last = match point.last_segment() {
        None => {
            return Err(nom::Err::Error(ErrorTree::from_error_kind(
                input.clone(),
                ErrorKind::Tag,
            )));
        }
        Some(last) => last,
    };
     */

    let template = TemplateVar {
        point,
        kind: KindTemplate {
            base: BaseKind::Bundle,
            sub: None,
            specific: None,
        },
    };

    let create = CreateVar {
        template,
        state: StateSrcVar::None,
        properties: Default::default(),
        strategy: Strategy::Commit,
    };

    Ok((next, create))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ctx {
    WorkingPoint,
    PointFromRoot,
}

impl ToString for Ctx {
    fn to_string(&self) -> String {
        match self {
            Ctx::WorkingPoint => ".".to_string(),
            Ctx::PointFromRoot => "...".to_string(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct File {
    pub name: String,
    pub content: Bin,
}

impl File {
    pub fn new<S: ToString>(name: S, content: Bin) -> Self {
        Self {
            name: name.to_string(),
            content,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct FileResolver {
    pub files: HashMap<String, Bin>,
}

impl FileResolver {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    pub fn file<N: ToString>(&self, name: N) -> Result<File, ResolverErr> {
        if let Some(content) = self.files.get(&name.to_string()) {
            Ok(File::new(name, content.clone()))
        } else {
            Err(ResolverErr::NotFound)
        }
    }

    /// grab the only file
    pub fn singleton(&self) -> Result<File, ResolverErr> {
        if self.files.len() == 1 {
            let i = &mut self.files.iter();
            if let Some((name, content)) = i.next() {
                Ok(File::new(name.clone(), content.clone()))
            } else {
                Err(ResolverErr::NotFound)
            }
        } else {
            Err(ResolverErr::NotFound)
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Env {
    parent: Option<Box<Env>>,
    pub point: Point,
    pub vars: HashMap<String, Substance>,
    pub file_resolver: FileResolver,

    #[serde(skip_serializing)]
    #[serde(skip_deserializing)]
    #[serde(default)]
    pub var_resolvers: MultiVarResolver,
}

impl Env {
    pub fn new(working: Point) -> Self {
        Self {
            parent: None,
            point: working,
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        }
    }

    pub fn no_point() -> Self {
        Self::new(Point::root())
    }

    pub fn push(self) -> Self {
        Self {
            point: self.point.clone(),
            parent: Some(Box::new(self)),
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        }
    }

    pub fn push_working<S: ToString>(self, segs: S) -> Result<Self, ParseErrs> {
        Ok(Self {
            point: self.point.push(segs.to_string())?,
            parent: Some(Box::new(self)),
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        })
    }

    pub fn point_or(&self) -> Result<Point, ParseErrs> {
        Ok(self.point.clone())
    }

    pub fn pop(self) -> Result<Env, ParseErrs> {
        Ok(*self
            .parent
            .ok_or(ParseErrs::new(&"expected parent scopedVars"))?)
    }

    pub fn add_var_resolver(&mut self, var_resolver: Arc<dyn VarResolver>) {
        self.var_resolvers.push(var_resolver);
    }

    pub fn val<K: ToString>(&self, var: K) -> Result<Substance, ResolverErr> {
        match self.vars.get(&var.to_string()) {
            None => {
                if let Ok(val) = self.var_resolvers.val(var.to_string().as_str()) {
                    Ok(val.clone())
                } else if let Some(parent) = self.parent.as_ref() {
                    parent.val(var.to_string())
                } else {
                    Err(ResolverErr::NotFound)
                }
            }
            Some(val) => Ok(val.clone()),
        }
    }

    pub fn set_working(&mut self, point: Point) {
        self.point = point;
    }

    pub fn working(&self) -> &Point {
        &self.point
    }

    pub fn set_var_str<V: ToString>(&mut self, key: V, value: V) {
        self.vars
            .insert(key.to_string(), Substance::Text(value.to_string()));
    }

    pub fn set_var<V: ToString>(&mut self, key: V, value: Substance) {
        self.vars.insert(key.to_string(), value);
    }

    pub fn file<N: ToString>(&self, name: N) -> Result<File, ResolverErr> {
        match self.file_resolver.files.get(&name.to_string()) {
            None => {
                if let Some(parent) = self.parent.as_ref() {
                    parent.file(name.to_string())
                } else {
                    Err(ResolverErr::NotFound)
                }
            }
            Some(bin) => Ok(File::new(name.to_string(), bin.clone())),
        }
    }

    pub fn set_file<N: ToString>(&mut self, name: N, content: Bin) {
        self.file_resolver.files.insert(name.to_string(), content);
    }
}

impl Default for Env {
    fn default() -> Self {
        Self {
            parent: None,
            point: Point::root(),
            vars: HashMap::new(),
            file_resolver: FileResolver::new(),
            var_resolvers: MultiVarResolver::new(),
        }
    }
}

/*
#[derive(Clone)]
pub struct Env {
    point: Option<Point>,
    var_resolver: Option<CompositeResolver>,
    file_resolver: Option<FileResolver>,
}

impl Env {
    pub fn add_var_resolver(&mut self, resolver: Arc<dyn VarResolver>) {
        if let Some(r) = self.var_resolver.as_mut() {
            r.other_resolver.push(resolver)
        }
    }

    pub fn no_point() -> Self {
        Self {
            point: None,
            var_resolver: Some(CompositeResolver::new()),
            file_resolver: None,
        }
    }

    pub fn unavailable() -> Self {
        Self {
            point: None,
            var_resolver: None,
            file_resolver: None,
        }
    }

    pub fn new(point: Point) -> Self {
        Self {
            point: Some(point),
            var_resolver: Some(CompositeResolver::new()),
            file_resolver: Some(FileResolver::new()),
        }
    }

    pub fn just_point(point: Point) -> Self {
        Self {
            point: Some(point),
            var_resolver: None,
            file_resolver: None,
        }
    }

    pub fn point_or(&self) -> Result<&Point, ExtErr> {
        self.point
            .as_ref()
            .ok_or("cannot reference working point in this context".into())
    }

    pub fn val(&self, var: &str) -> Result<String, ResolverErr> {
        if let None = self.var_resolver {
            Err(ResolverErr::NotAvailable)
        } else {
            self.var_resolver.as_ref().unwrap().val(var)
        }
    }

    pub fn set_working(&mut self, point: Point) {
        self.point.replace(point);
    }

    pub fn set_var<V: ToString>(&mut self, key: V, value: V) {
        match self.var_resolver.as_mut() {
            None => {
                let mut var_resolver = CompositeResolver::new();
                var_resolver.set(key, value);
                self.var_resolver.replace(var_resolver);
            }
            Some(var_resolver) => {
                var_resolver.set(key, value);
            }
        }
    }

    pub fn file<N: ToString>(&self, name: N) -> Result<File, ResolverErr> {
        match &self.file_resolver {
            None => Err(ResolverErr::NotAvailable),
            Some(file_resolver) => file_resolver.file(name),
        }
    }

    pub fn set_file<N: ToString>(&mut self, name: N, content: Bin) {
        match self.file_resolver.as_mut() {
            None => {
                let mut file_resolver = FileResolver::new();
                file_resolver.files.insert(name.to_string(), content);
                self.file_resolver.replace(file_resolver);
            }
            Some(file_resolver) => {
                file_resolver.files.insert(name.to_string(), content);
            }
        }
    }
}

 */

#[derive(Clone)]
pub struct CompositeResolver {
    pub env_resolver: Arc<dyn VarResolver>,
    pub scope_resolver: MapResolver,
    pub other_resolver: MultiVarResolver,
}

impl CompositeResolver {
    pub fn new() -> Self {
        Self {
            env_resolver: Arc::new(NoResolver::new()),
            scope_resolver: MapResolver::new(),
            other_resolver: MultiVarResolver::new(),
        }
    }

    pub fn set<S>(&mut self, key: S, value: Substance)
    where
        S: ToString,
    {
        self.scope_resolver.insert(key.to_string(), value);
    }
}

impl VarResolver for CompositeResolver {
    fn val(&self, var: &str) -> Result<Substance, ResolverErr> {
        if let Ok(val) = self.scope_resolver.val(var) {
            Ok(val)
        } else if let Ok(val) = self.scope_resolver.val(var) {
            Ok(val)
        } else if let Ok(val) = self.other_resolver.val(var) {
            Ok(val)
        } else {
            Err(ResolverErr::NotFound)
        }
    }
}

pub trait CtxResolver {
    fn working_point(&self) -> Result<&Point, ParseErrs>;
}

pub struct PointCtxResolver(Point);

impl CtxResolver for PointCtxResolver {
    fn working_point(&self) -> Result<&Point, ParseErrs> {
        Ok(&self.0)
    }
}

#[derive(Clone, Debug, Error)]
#[error("{err}: {context}")]
pub struct ResolverErrCtx {
    thing: String,
    context: String,
    err: ResolverErr,
}

#[derive(Clone, Debug, Error)]
pub enum ResolverErr {
    #[error("not available")]
    NotAvailable,
    #[error("not found")]
    NotFound,
}

pub trait VarResolver: Send + Sync {
    fn val(&self, var: &str) -> Result<Substance, ResolverErr> {
        Err(ResolverErr::NotFound)
    }
}

#[derive(Clone)]
pub struct NoResolver;

impl NoResolver {
    pub fn new() -> Self {
        Self {}
    }
}

impl VarResolver for NoResolver {}

#[derive(Clone)]
pub struct MapResolver {
    pub map: HashMap<String, Substance>,
}

impl MapResolver {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert<K: ToString>(&mut self, key: K, value: Substance) {
        self.map.insert(key.to_string(), value);
    }
}

impl VarResolver for MapResolver {
    fn val(&self, var: &str) -> Result<Substance, ResolverErr> {
        self.map
            .get(&var.to_string())
            .cloned()
            .ok_or(ResolverErr::NotFound)
    }
}

#[derive(Clone)]
pub struct RegexCapturesResolver {
    regex: Regex,
    text: String,
}

impl RegexCapturesResolver {
    pub fn new(regex: Regex, text: String) -> Result<Self, ParseErrs> {
        regex
            .captures(text.as_str())
            .ok_or(ParseErrs::new("no regex captures"))?;
        Ok(Self { regex, text })
    }
}

impl VarResolver for RegexCapturesResolver {
    fn val(&self, id: &str) -> Result<Substance, ResolverErr> {
        let captures = self
            .regex
            .captures(self.text.as_str())
            .expect("expected captures");
        match captures.name(id) {
            None => Err(ResolverErr::NotFound),
            Some(m) => Ok(Substance::Text(m.as_str().to_string())),
        }
    }
}

#[derive(Clone)]
pub struct MultiVarResolver(Vec<Arc<dyn VarResolver>>);

impl Default for MultiVarResolver {
    fn default() -> Self {
        MultiVarResolver::new()
    }
}

impl MultiVarResolver {
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn push(&mut self, resolver: Arc<dyn VarResolver>) {
        self.0.push(resolver);
    }
}

impl VarResolver for MultiVarResolver {
    fn val(&self, var: &str) -> Result<Substance, ResolverErr> {
        for resolver in &self.0 {
            match resolver.val(&var.to_string()) {
                Ok(ok) => return Ok(ok),
                Err(_) => {}
            }
        }
        Err(ResolverErr::NotFound)
    }
}

/*
pub trait BruteResolver<Resolved>
where
    Self: Sized + ToResolved<Resolved>,
{
    fn brute_resolve(self) -> Result<Resolved, ExtErr> {
        let resolver = NoResolver::new().wrap();
        Ok(self.to_resolved(&resolver)?)
    }
}

 */

pub fn diagnose<I: Clone, O, F>(tag: &'static str, mut f: F) -> impl FnMut(I) -> Res<I, O>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    F: nom::Parser<I, O, NomErr<I>>,
    O: Clone,
{
    move |input: I| {
        let (next, i) = f.parse(input)?;
        Ok((next, i))
    }
}

pub trait SubstParser<T: Sized> {
    fn parse_string(&self, string: String) -> Result<T, ParseErrs> {
        let span = new_span(string.as_str());
        let output = result(self.parse_span(span))?;
        Ok(output)
    }

    fn parse_span<I: Span>(&self, input: I) -> Res<I, T>;
}

pub fn root_ctx_seg<I: Span, F>(mut f: F) -> impl FnMut(I) -> Res<I, PointSegCtx> + Copy
where
    F: Parser<I, PointSeg, NomErr<I>> + Copy,
{
    move |input: I| match pair(tag::<&str, I, NomErr<I>>(".."), eos)(input.clone()) {
        Ok((next, v)) => Ok((
            next.clone(),
            PointSegCtx::Pop(Trace {
                range: next.location_offset() - 2..next.location_offset(),
                extra: next.extra(),
            }),
        )),
        Err(err) => match pair(tag::<&str, I, NomErr<I>>("."), eos)(input.clone()) {
            Ok((next, _)) => Ok((
                next.clone(),
                PointSegCtx::Working(Trace {
                    range: next.location_offset() - 1..next.location_offset(),
                    extra: next.extra(),
                }),
            )),
            Err(err) => match f.parse(input) {
                Ok((next, seg)) => Ok((next, seg.into())),
                Err(err) => Err(err),
            },
        },
    }
}

pub fn working<I: Span, F>(mut f: F) -> impl FnMut(I) -> Res<I, PointSegCtx>
where
    F: nom::Parser<I, PointSeg, NomErr<I>>,
{
    move |input: I| match pair(tag::<&str, I, NomErr<I>>("."), eos)(input.clone()) {
        Ok((next, v)) => Ok((
            next.clone(),
            PointSegCtx::Working(Trace {
                range: next.location_offset() - 1..next.location_offset(),
                extra: next.extra(),
            }),
        )),
        Err(err) => match f.parse(input.clone()) {
            Ok((next, seg)) => Ok((next, seg.into())),
            Err(err) => Err(err),
        },
    }
}

pub fn pop<I: Span, F>(mut f: F) -> impl FnMut(I) -> Res<I, PointSegCtx> + Copy
where
    F: nom::Parser<I, PointSeg, NomErr<I>> + Copy,
{
    move |input: I| match pair(tag::<&str, I, NomErr<I>>(".."), eos)(input.clone()) {
        Ok((next, v)) => Ok((
            next.clone(),
            PointSegCtx::Working(Trace {
                range: next.location_offset() - 2..next.location_offset(),
                extra: next.extra(),
            }),
        )),
        Err(err) => match f.parse(input.clone()) {
            Ok((next, seg)) => Ok((next, seg.into())),
            Err(err) => Err(err),
        },
    }
}

pub fn base_seg<I, F, S>(mut f: F) -> impl FnMut(I) -> Res<I, S>
where
    I: Span,
    F: nom::Parser<I, S, NomErr<I>> + Copy,
    S: PointSegment,
{
    move |input: I| preceded(tag(":"), f)(input)
}

pub fn mesh_seg<I: Span, F, S1, S2>(mut f: F) -> impl FnMut(I) -> Res<I, S2>
where
    F: nom::Parser<I, S1, NomErr<I>> + Copy,
    S1: PointSegment + Into<S2>,
    S2: PointSegment,
{
    move |input: I| {
        tuple((seg_delim, f, eos))(input).map(|(next, (delim, seg, _))| (next, seg.into()))
    }
}

// end of segment
pub fn seg_delim<I: Span>(input: I) -> Res<I, PointSegDelim>
where
    I: ToString
        + Clone
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar + Clone,
{
    alt((
        value(PointSegDelim::File, tag("/")),
        value(PointSegDelim::Mesh, tag(":")),
    ))(input)
    .map(|(next, delim)| (next, delim))
}

// end of segment
pub fn eos<I: Span>(input: I) -> Res<I, ()> {
    peek(alt((tag("/"), tag(":"), tag("%"), space1, eof)))(input).map(|(next, _)| (next, ()))
}

/*
pub fn var<O>(f: impl Fn(Span) -> Res<Span,O>+'static+Clone) -> impl FnMut(Span) -> Res<Span, Symbol<O>>
{
    unimplemented!()
    /*
    move |input: Span| {
        preceded(
            tag("$"),
            context(
                "variable",
                cut(delimited(
                    context("variable:open", tag("(")),
                    variable_name,
                    tag(")"),
                )),
            ),
        )(input)
        .map(|(next, name)| {
            (
                next,
                Symbol::named(name.to_string(), f.clone() ),
            )
        })
    }

     */
}

 */

/*jjj
pub fn ctx<O><I:Span>(input: Span) -> Res<Span, Symbol<O>>

{
    alt((
        value(Ctx::RelativePointPop, tuple((tag(".."), eos))),
        value(Ctx::RelativePoint, tuple((tag("."), eos))),
    ))(input)
        .map(|(next, ctx)| (next, Symbol::ctx(ctx)))
}

 */

/*
pub fn ctx<I, O, F, E><I:Span>(input: I) -> Res<I, Symbol<O>>
where
    I: ToString
        + Clone
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar + Clone,
    F: nom::Parser<I, O, E> + Clone,
    E: nom_supreme::context::ContextError<I,ParseTree<I>>,
{
    alt((
        value(Ctx::RelativePointPop, tuple((tag(".."), eos))),
        value(Ctx::RelativePoint, tuple((tag("."), eos))),
    ))(input)
    .map(|(next, ctx)| (next, Symbol::ctx(ctx)))
}

 */

pub fn ispan<'a, I: Clone, O, F>(mut f: F) -> impl FnMut(I) -> Res<I, Spanned<I, O>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    F: nom::Parser<I, O, NomErr<I>>,
    O: Clone + FromStr<Err = ParseErrs>,
{
    move |input: I| {
        let (next, element) = f.parse(input.clone())?;
        Ok((next, Spanned::new(element, input.clone())))
    }
}

pub fn sub<I: Span, O, F>(mut f: F) -> impl FnMut(I) -> Res<I, Spanned<I, O>>
where
    F: nom::Parser<I, O, NomErr<I>>,
    O: Clone,
{
    move |input: I| {
        let (next, element) = f.parse(input.clone())?;
        Ok((
            next.clone(),
            Spanned::new(element, input.slice(0..(input.len() - next.len()))),
        ))
    }
}

pub fn access_grant_kind<I: Span>(input: I) -> Res<I, AccessGrantKind> {
    tuple((
        context(
            "access_grant_kind",
            peek(alt((
                tuple((tag("perm"), space1)),
                tuple((tag("priv"), space1)),
            ))),
        ),
        alt((access_grant_kind_perm, access_grant_kind_priv)),
    ))(input)
    .map(|(next, (_, kind))| (next, kind))
}

pub fn access_grant_kind_priv<I: Span>(input: I) -> Res<I, AccessGrantKind> {
    tuple((
        tag("priv"),
        context("access_grant:priv", tuple((space1, privilege))),
    ))(input)
    .map(|(next, (_, (_, privilege)))| (next, AccessGrantKindDef::Privilege(privilege)))
}

pub fn access_grant_kind_perm<I: Span>(input: I) -> Res<I, AccessGrantKind> {
    tuple((
        tag("perm"),
        context("access_grant:perm", tuple((space1, permissions_mask))),
    ))(input)
    .map(|(next, (_, (_, perms)))| (next, AccessGrantKindDef::PermissionsMask(perms)))
}

pub fn privilege<I: Span>(input: I) -> Res<I, Privilege> {
    context("privilege", alt((tag("*"), skewer_colon)))(input).map(|(next, prv)| {
        let prv = match prv.to_string().as_str() {
            "*" => Privilege::Full,
            prv => Privilege::Single(prv.to_string()),
        };
        (next, prv)
    })
}

pub fn permissions_mask<I: Span>(input: I) -> Res<I, PermissionsMask> {
    context(
        "permissions_mask",
        tuple((
            alt((
                value(PermissionsMaskKind::Or, char('+')),
                value(PermissionsMaskKind::And, char('&')),
            )),
            permissions,
        )),
    )(input)
    .map(|(next, (kind, permissions))| {
        let mask = PermissionsMask { kind, permissions };

        (next, mask)
    })
}

pub fn permissions<I: Span>(input: I) -> Res<I, Permissions> {
    context(
        "permissions",
        tuple((child_perms, tag("-"), particle_perms)),
    )(input)
    .map(|(next, (child, _, particle))| {
        let permissions = Permissions { child, particle };
        (next, permissions)
    })
}

pub fn child_perms<I: Span>(input: I) -> Res<I, ChildPerms> {
    context(
        "child_perms",
        alt((
            tuple((
                alt((value(false, char('c')), value(true, char('C')))),
                alt((value(false, char('s')), value(true, char('S')))),
                alt((value(false, char('d')), value(true, char('D')))),
            )),
            fail,
        )),
    )(input)
    .map(|(next, (create, select, delete))| {
        let block = ChildPerms {
            create,
            select,
            delete,
        };
        (next, block)
    })
}

pub fn particle_perms<I: Span>(input: I) -> Res<I, ParticlePerms> {
    context(
        "particle_perms",
        tuple((
            alt((value(false, char('r')), value(true, char('R')))),
            alt((value(false, char('w')), value(true, char('W')))),
            alt((value(false, char('x')), value(true, char('X')))),
        )),
    )(input)
    .map(|(next, (read, write, execute))| {
        let block = ParticlePerms {
            read,
            write,
            execute,
        };
        (next, block)
    })
}

/*
pub fn grant<I><I:Span>(input: I) -> Res<I,AccessGrant> where I:Clone+InputIter+InputLength+InputTake{

}

 */

pub fn none<I: Span, O, E>(input: I) -> IResult<I, Option<O>, E> {
    Ok((input, None))
}

pub fn some<I: Span, O, F>(mut f: F) -> impl FnMut(I) -> Res<I, Option<O>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar,
    F: nom::Parser<I, O, NomErr<I>> + Clone,
{
    move |input: I| {
        f.clone()
            .parse(input)
            .map(|(next, output)| (next, Some(output)))
    }
}

pub fn lex_block_alt<I: Span>(kinds: Vec<BlockKind>) -> impl FnMut(I) -> Res<I, LexBlock<I>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar + Copy,
{
    move |input: I| {
        for kind in &kinds {
            let result = lex_block(kind.clone())(input.clone());
            match &result {
                Ok((next, block)) => return result,
                Err(err) => {
                    match err {
                        nom::Err::Incomplete(Needed) => return result,
                        nom::Err::Error(e) => {
                            // let's hope for another match...
                        }
                        nom::Err::Failure(e) => return result,
                    }
                }
            }
        }

        Err(nom::Err::Failure(NomErr::from_error_kind(
            input.clone(),
            ErrorKind::Alt,
        )))
    }
}

pub fn lex_block<I: Span>(kind: BlockKind) -> impl FnMut(I) -> Res<I, LexBlock<I>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar + Copy,
{
    move |input: I| match kind {
        BlockKind::Nested(kind) => lex_nested_block(kind).parse(input),
        BlockKind::Terminated(kind) => lex_terminated_block(kind).parse(input),
        BlockKind::Delimited(kind) => lex_delimited_block(kind).parse(input),
        BlockKind::Partial => Err(nom::Err::Failure(NomErr::from_error_kind(
            input,
            ErrorKind::IsNot,
        ))),
    }
}

pub fn lex_terminated_block<I: Span>(
    kind: TerminatedBlockKind,
) -> impl FnMut(I) -> Res<I, LexBlock<I>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar,
{
    move |input: I| {
        terminated(
            recognize(many0(satisfy(|c| c != kind.as_char()))),
            tag(kind.tag()),
        )(input)
        .map(|(next, content)| {
            let block = LexBlock {
                kind: BlockKind::Terminated(kind),
                content,
                data: (),
            };

            (next, block)
        })
    }
}

/// rough block simply makes sure that the opening and closing symbols match
/// it accounts for multiple embedded blocks of the same kind but NOT of differing kinds
pub fn lex_nested_block<I: Span>(kind: NestedBlockKind) -> impl FnMut(I) -> Res<I, LexBlock<I>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar + Copy,
{
    move |input: I| {
        let (next, content) = context(
            kind.context(),
            delimited(
                context(kind.open_context(), tag(kind.open())),
                recognize(many0(alt((
                    recognize(lex_nested_block(kind.clone())),
                    recognize(tuple((
                        not(peek(tag(kind.close()))),
                        alt((recognize(pair(tag("\\"), anychar)), recognize(anychar))),
                    ))),
                )))),
                context(kind.close_context(), cut(tag(kind.close()))),
            ),
        )(input)?;
        let block = Block::parse(BlockKind::Nested(kind), content);
        Ok((next, block))
    }
}

pub fn nested_block_content<I: Span>(kind: NestedBlockKind) -> impl FnMut(I) -> Res<I, I> {
    move |input: I| nested_block(kind)(input).map(|(next, block)| (next, block.content))
}

pub fn nested_block<I: Span>(kind: NestedBlockKind) -> impl FnMut(I) -> Res<I, Block<I, ()>> {
    move |input: I| {
        let (next, content) = context(
            kind.context(),
            delimited(
                context(kind.open_context(), tag(kind.open())),
                recognize(many0(tuple((
                    not(peek(tag(kind.close()))),
                    context(
                        kind.unpaired_closing_scope(),
                        cut(peek(expected_block_terminator_or_non_terminator(
                            kind.clone(),
                        ))),
                    ),
                    alt((
                        recognize(pair(peek(block_open), any_block)),
                        recognize(anychar),
                    )),
                )))),
                context(kind.close_context(), cut(tag(kind.close()))),
            ),
        )(input)?;
        let block = Block::parse(BlockKind::Nested(kind), content);
        Ok((next, block))
    }
}

pub fn lex_delimited_block<I: Span>(
    kind: DelimitedBlockKind,
) -> impl FnMut(I) -> Res<I, LexBlock<I>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar + Copy,
{
    move |input: I| {
        let (next, content) = context(
            kind.context(),
            delimited(
                context(kind.context(), tag(kind.delim())),
                recognize(many0(tuple((
                    not(peek(tag(kind.delim()))),
                    alt((recognize(pair(tag("\\"), anychar)), recognize(anychar))),
                )))),
                context(kind.missing_close_context(), cut(tag(kind.delim()))),
            ),
        )(input)?;
        let block = Block::parse(BlockKind::Delimited(kind), content);
        Ok((next, block))
    }
}

fn block_open<I: Span>(input: I) -> Res<I, NestedBlockKind> {
    alt((
        value(NestedBlockKind::Curly, tag(NestedBlockKind::Curly.open())),
        value(NestedBlockKind::Angle, tag(NestedBlockKind::Angle.open())),
        value(NestedBlockKind::Parens, tag(NestedBlockKind::Parens.open())),
        value(NestedBlockKind::Square, tag(NestedBlockKind::Square.open())),
    ))(input)
}

fn any_surrounding_lex_block<I: Span>(input: I) -> Res<I, LexBlock<I>>
where
    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar + Copy,
{
    alt((
        lex_nested_block(NestedBlockKind::Curly),
        lex_nested_block(NestedBlockKind::Angle),
        lex_nested_block(NestedBlockKind::Parens),
        lex_nested_block(NestedBlockKind::Square),
        lex_delimited_block(DelimitedBlockKind::DoubleQuotes),
        lex_delimited_block(DelimitedBlockKind::SingleQuotes),
    ))(input)
}

fn any_block<I: Span>(input: I) -> Res<I, LexBlock<I>> {
    alt((
        nested_block(NestedBlockKind::Curly),
        nested_block(NestedBlockKind::Angle),
        nested_block(NestedBlockKind::Parens),
        nested_block(NestedBlockKind::Square),
        lex_delimited_block(DelimitedBlockKind::DoubleQuotes),
        lex_delimited_block(DelimitedBlockKind::SingleQuotes),
    ))(input)
}

pub fn expected_block_terminator_or_non_terminator<I: Span>(
    expect: NestedBlockKind,
) -> impl FnMut(I) -> Res<I, ()>
where
    I: InputIter + InputLength + Slice<RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar,
    I: Clone,
{
    move |input: I| -> Res<I, ()> {
        verify(anychar, move |c| {
            if NestedBlockKind::is_block_terminator(*c) {
                *c == expect.close_as_char()
            } else {
                true
            }
        })(input)
        .map(|(next, _)| (next, ()))
    }
}

/*
pub fn lex_hierarchy_scope<'a>(
    scope: LexScope<Span<'a>>,
    max_depth: usize,
) -> Result<LexHierarchyScope<'a>, ExtErr> {
    let mut errs = vec![];
    let scope = lex_child_scopes(scope)?;
    let mut children = vec![];

    for child in scope.block {
        if max_depth <= 0 {
            let mut builder = Report::build(ReportKind::Error, (), 0);
            let report = builder
                .with_message("exceeded max depth hierarchy for nested scopes")
                .with_label(
                    Label::new(
                        child.block.content.location_offset()
                            ..child.block.content.location_offset() + child.block.content.len(),
                    )
                    .with_message("Nest Limit Exceeded"),
                )
                .finish();
            return Err(ParseErrs::new(report, child.block.content.extra.clone()).into());
        }
        match lex_hierarchy_scope(child, max_depth - 1) {
            Ok(child) => {
                children.push(child);
            }
            Err(err) => errs.push(err),
        }
    }

    Ok(LexHierarchyScope::new(scope.selector.clone(), children))
}*/

pub fn unwrap_block<I: Span, F, O>(kind: BlockKind, mut f: F) -> impl FnMut(I) -> Res<I, O>
where
    F: FnMut(I) -> Res<I, O>,
{
    move |input: I| {
        let (next, block) = lex_block(kind)(input)?;
        let (_, content) = f.parse(block.content)?;
        //        let (_, content) = context("block", f)(block.content)?;
        Ok((next, content))
    }
}

pub fn lex_child_scopes<I: Span>(parent: LexScope<I>) -> Result<LexParentScope<I>, ParseErrs> {
    if parent.selector.children.is_some() {
        let (_, child_selector) = all_consuming(lex_scope_selector)(
            parent
                .selector
                .children
                .as_ref()
                .expect("child names...")
                .clone(),
        )?;

        let child = LexScope::new(child_selector.into(), parent.block);

        Ok(LexParentScope {
            selector: parent.selector.clone(),
            pipeline_step: None,
            block: vec![child],
        })
    } else {
        let scopes = lex_scopes(parent.block.content)?;

        Ok(LexParentScope {
            selector: parent.selector.into(),
            pipeline_step: parent.pipeline_step,
            block: scopes,
        })
    }
}

pub fn lex_scope<I: Span>(input: I) -> Res<I, LexScope<I>> {
    context(
        "scope",
        tuple((
            peek(alt((tag("*"), alpha1, tag("<")))),
            lex_scope_selector,
            multispace1,
            lex_scope_pipeline_step_and_block,
        )),
    )(input)
    .map(|(next, (_, selector, _, (pipeline_step, block)))| {
        let scope = LexScope {
            selector,
            pipeline_step,
            block,
        };
        (next, scope)
    })
}

pub fn lex_scoped_block_kind<I: Span>(input: I) -> Res<I, BlockKind> {
    alt((
        value(
            BlockKind::Nested(NestedBlockKind::Curly),
            recognize(tuple((
                multispace0,
                rough_pipeline_step,
                multispace0,
                lex_block(BlockKind::Nested(NestedBlockKind::Curly)),
            ))),
        ),
        value(
            BlockKind::Terminated(TerminatedBlockKind::Semicolon),
            recognize(pair(
                rough_pipeline_step,
                lex_block(BlockKind::Terminated(TerminatedBlockKind::Semicolon)),
            )),
        ),
    ))(input)
}

pub fn lex_scope_pipeline_step_and_block<I: Span>(input: I) -> Res<I, (Option<I>, LexBlock<I>)> {
    let (_, block_kind) = peek(lex_scoped_block_kind)(input.clone())?;
    match block_kind {
        BlockKind::Nested(_) => tuple((
            rough_pipeline_step,
            multispace1,
            lex_block(BlockKind::Nested(NestedBlockKind::Curly)),
        ))(input)
        .map(|(next, (step, _, block))| (next, (Some(step), block))),
        BlockKind::Terminated(_) => {
            lex_block(BlockKind::Terminated(TerminatedBlockKind::Semicolon))(input)
                .map(|(next, block)| (next, (None, block)))
        }
        _ => unimplemented!(),
    }
}

pub fn lex_sub_scope_selectors_and_filters_and_block<I: Span>(input: I) -> Res<I, LexBlock<I>> {
    recognize(pair(
        nested_block_content(NestedBlockKind::Angle),
        tuple((
            opt(scope_filters),
            multispace0,
            opt(rough_pipeline_step),
            multispace0,
            lex_block_alt(vec![
                BlockKind::Nested(NestedBlockKind::Curly),
                BlockKind::Terminated(TerminatedBlockKind::Semicolon),
            ]),
        )),
    ))(input)
    .map(|(next, content)| {
        (
            next,
            LexBlock {
                kind: BlockKind::Partial,
                content,
                data: (),
            },
        )
    })
}

pub fn root_scope<I: Span>(input: I) -> Res<I, LexRootScope<I>> {
    context(
        "root-scope",
        tuple((
            root_scope_selector,
            multispace0,
            context("root-scope:block", cut(peek(tag("{")))),
            context(
                "root-scope:block",
                cut(lex_nested_block(NestedBlockKind::Curly)),
            ),
        )),
    )(input)
    .map(|(next, (selector, _, _, block))| {
        let scope = LexRootScope::new(selector, block);
        (next, scope)
    })
}

pub fn lex_scopes<I: Span>(input: I) -> Result<Vec<LexScope<I>>, ParseErrs> {
    if input.len() == 0 {
        return Ok(vec![]);
    }

    if wrapper(input.clone(), all_consuming(multispace1)).is_ok() {
        return Ok(vec![]);
    }

    result(
        context(
            "parsed-scopes",
            all_consuming(many0(delimited(
                multispace0,
                context(
                    "scope",
                    pair(peek(not(alt((tag("}"), eof)))), cut(lex_scope)),
                ),
                multispace0,
            ))),
        )(input)
        .map(|(next, scopes)| {
            let scopes: Vec<LexScope<I>> = scopes.into_iter().map(|scope| scope.1).collect();
            (next, scopes)
        }),
    )
}

/*
pub fn sub_scope_selector<I:Span>(input: Span) -> Res<Span, ScopeSelector<Span>> {
    alt((sub_scope_selector_expanded, sub_scope_selector_collapsed))
}




pub fn lex_scope_selector_no_filters(
    input: Span,
) -> Res<Span, ParsedScopeSelectorAndFilters<Span>> {
    context("parsed-scope-selector-no-filters", lex_scope_selector)(input)
        .map(|(next, selector)| (next, ParsedScopeSelectorAndFilters::new(selector, vec![])))
}

 */

pub fn next_stacked_name<I: Span>(input: I) -> Res<I, (I, Option<I>)> {
    match wrapper(
        input.clone(),
        pair(
            peek(tag("<")),
            tuple((
                tag("<"),
                pair(
                    context("scope-selector", alt((alphanumeric1, tag("*")))),
                    opt(recognize(nested_block(NestedBlockKind::Angle))),
                ),
                tag(">"),
            )),
        ),
    )
    .map(|(next, (_, (_, (name, children), _)))| (next, (name, children)))
    {
        Ok((next, (name, children))) => return Ok((next, (name, children))),
        Err(_) => {}
    }
    pair(
        context("scope-selector", cut(alt((alphanumeric1, tag("*"))))),
        opt(recognize(nested_block(NestedBlockKind::Angle))),
    )(input)
}

pub fn lex_scope_selector<I: Span>(input: I) -> Res<I, LexScopeSelector<I>> {
    let (next, ((name, children), filters, path)) = context(
        "parsed-scope-selector",
        tuple((next_stacked_name, scope_filters, opt(path_regex))),
    )(input.clone())?;

    Ok((next, LexScopeSelector::new(name, filters, path, children)))
}

pub fn lex_name_stack<I: Span>(mut input: I) -> Res<I, Vec<I>> {
    let mut stack = vec![];
    let (next, (name, mut children)) = next_stacked_name(input)?;
    stack.push(name);
    loop {
        match &children {
            None => {
                break;
            }
            Some(children) => {
                input = children.clone();
            }
        }
        let (_, (name, c)) = next_stacked_name(input)?;
        children = c;
        stack.push(name);
    }

    Ok((next, stack))
}

pub struct LexRouteSelector<I> {
    pub names: Vec<I>,
    pub filters: ScopeFiltersDef<I>,
    pub path: Option<I>,
}

pub fn lex_route_selector<I: Span>(input: I) -> Res<I, LexRouteSelector<I>> {
    tuple((lex_name_stack, scope_filters, opt(path_regex)))(input).map(
        |(next, (names, filters, path))| {
            let selector = LexRouteSelector {
                names,
                filters,
                path,
            };
            (next, selector)
        },
    )
}

pub fn wrapper<I: Span, O, F>(input: I, mut f: F) -> Res<I, O>
where
    F: FnMut(I) -> Res<I, O>,
{
    f.parse(input)
}

pub fn parse_inner_block<I, F>(kind: NestedBlockKind, mut f: &F) -> impl FnMut(I) -> Res<I, I> + '_
where
    I: Span,
    &'static str: FindToken<<I as InputTakeAtPosition>::Item>,

    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar + Copy,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar + Copy,
    F: Fn(char) -> bool,
    F: Clone,
{
    move |input: I| {
        let (next, rtn) = alt((
            delimited(
                tag(kind.open()),
                recognize(many1(alt((
                    recognize(any_surrounding_lex_block),
                    recognize(verify(anychar, move |c| {
                        f(*c) && *c != kind.close_as_char()
                    })),
                )))),
                tag(kind.close()),
            ),
            recognize(many1(verify(anychar, move |c| {
                f(*c) && *c != kind.close_as_char()
            }))),
        ))(input)?;
        Ok((next, rtn))
    }
}

pub fn parse_include_blocks<I, O2, F>(kind: NestedBlockKind, mut f: F) -> impl FnMut(I) -> Res<I, I>
where
    I: Span,
    &'static str: FindToken<<I as InputTakeAtPosition>::Item>,

    I: ToString
        + InputLength
        + InputTake
        + Compare<&'static str>
        + InputIter
        + Clone
        + InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar,
    I: ToString,
    I: Offset + nom::Slice<std::ops::RangeTo<usize>>,
    I: nom::Slice<std::ops::RangeFrom<usize>>,
    <I as InputIter>::Item: AsChar,
    F: FnMut(I) -> Res<I, O2>,
    F: Clone,
    <I as InputIter>::Item: std::marker::Copy,
{
    move |input: I| {
        recognize(many0(alt((
            recognize(any_surrounding_lex_block),
            recognize(verify(anychar, move |c| *c != kind.close_as_char())),
        ))))(input)
    }
}

pub fn scope_filters<I: Span>(input: I) -> Res<I, ScopeFiltersDef<I>> {
    pair(opt(scope_filter), many0(preceded(tag("-"), scope_filter)))(input).map(
        |(next, (first, mut many_filters))| {
            let mut filters = vec![];
            match first {
                None => {}
                Some(first) => {
                    filters.push(first);
                }
            }
            filters.append(&mut many_filters);
            let filters = ScopeFiltersDef { filters };
            (next, filters)
        },
    )
}

pub fn scope_filter<I: Span>(input: I) -> Res<I, ScopeFilterDef<I>> {
    delimited(
        tag("("),
        context(
            "scope-filter",
            cut(tuple((
                context("filter-name", cut(scope_name)),
                opt(context(
                    "filter-arguments",
                    preceded(
                        multispace1,
                        parse_include_blocks(NestedBlockKind::Parens, args),
                    ),
                )),
            ))),
        ),
        tag(")"),
    )(input)
    .map(|(next, (name, args))| {
        let filter = ScopeFilterDef { name, args };
        (next, filter)
    })
}

pub fn scope_name<I>(input: I) -> Res<I, I>
where
    I: Span,
{
    recognize(pair(
        skewer_case_chars,
        peek(alt((eof, multispace1, tag(")")))),
    ))(input)
}

pub fn root_scope_selector<I: Span>(input: I) -> Res<I, RootScopeSelector<I, Spanned<I, Version>>> {
    context(
        "root-scope-selector",
        cut(preceded(
            multispace0,
            pair(
                context("root-scope-selector:name", cut(root_scope_selector_name)),
                context("root-scope-selector:version", cut(scope_version)),
            ),
        )),
    )(input)
    .map(|(next, (name, version))| (next, RootScopeSelector { version, name }))
}

pub fn scope_version<I: Span>(input: I) -> Res<I, Spanned<I, Version>> {
    context(
        "scope-selector-version",
        tuple((
            tag("(version="),
            sub(version),
            context("scope-selector-version-closing-tag", tag(")")),
        )),
    )(input)
    .map(|((next, (_, version, _)))| (next, version))
}

/*
pub fn mytag<O>( tag: &str ) -> impl Fn(Span) -> Res<Span,O>
{
    move |i: Span| {
        let tag_len = tag.input_len();
        let t = tag.clone();
        let res: IResult<_, _, Error> = match i.compare(t) {
            CompareResult::Ok => Ok(i.take_split(tag_len)),
            _ => {
                let e: ErrorKind = ErrorKind::Tag;
                Err(Err::Error(Error::from_error_kind(i, e)))
            }
        };
        res
    }
}

 */

pub fn scope_selector_name<I: Span>(input: I) -> Res<I, I> {
    context(
        "scope-selector-name",
        delimited(
            (context(
                "scope-selector-name:expect-alphanumeric-leading",
                cut(peek(alpha1)),
            )),
            alphanumeric1,
            context(
                "scope-selector-name:expect-termination",
                cut(peek(alt((
                    multispace1,
                    tag("{"),
                    tag("("),
                    tag("<"),
                    tag(">"),
                )))),
            ),
        ),
    )(input)
    .map(|(next, name)| (next, name))
}

pub fn root_scope_selector_name<I: Span>(input: I) -> Res<I, I> {
    context(
        "root-scope-selector-name",
        pair((peek(alpha1)), alphanumeric1),
    )(input)
    .map(|(next, (_, name))| (next, name))
}

pub fn lex_root_scope<I: Span>(span: I) -> Result<LexRootScope<I>, ParseErrs> {
    let root_scope = result(delimited(multispace0, root_scope, multispace0)(span))?;
    Ok(root_scope)
}

pub fn method_kind<I: Span>(input: I) -> Res<I, MethodKind> {
    let (next, v) = recognize(alt((tag("Cmd"), tag("Ext"), tag("Http"), tag("Hyp"))))(input)?;
    Ok((next, MethodKind::from_str(v.to_string().as_str()).unwrap()))
}

pub mod model {
    use std::fmt::Write;
    use std::ops::{Deref, DerefMut};
    use std::str::FromStr;

    use crate::config::bind::{PipelineStepDef, PipelineStopDef};
    use crate::err::ParseErrs;
    use crate::loc::Version;
    use crate::parse::util::{new_span, result, Span, Trace, Tw};
    use crate::parse::{lex_child_scopes, method_kind, pipeline, subst_path, unwrap_block, value_pattern, wrapped_cmd_method, wrapped_ext_method, wrapped_http_method, wrapped_sys_method, Assignment, Env, Res};
    use crate::point::{Point, PointCtx, PointVar};
    use crate::util::{ToResolved, ValueMatcher, ValuePattern};
    use crate::wave::core::{Method, MethodKind};
    use crate::wave::{DirectedWave, SingularDirectedWave};
    use regex::Regex;
    use serde::de::Visitor;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use thiserror::Error;

    #[derive(Clone)]
    pub struct ScopeSelectorAndFiltersDef<S, I> {
        pub selector: S,
        pub filters: ScopeFiltersDef<I>,
    }

    impl<S, I> Deref for ScopeSelectorAndFiltersDef<S, I> {
        type Target = S;

        fn deref(&self) -> &Self::Target {
            &self.selector
        }
    }

    impl<S, I> ScopeSelectorAndFiltersDef<S, I> {
        pub fn new(selector: S, filters: ScopeFiltersDef<I>) -> Self {
            Self { selector, filters }
        }
    }

    pub enum ParsePhase {
        Root,
        SubScopes,
    }

    #[derive(Clone)]
    pub struct Spanned<I, E>
    where
        E: Clone,
        I: ToString,
    {
        pub span: I,
        pub element: E,
    }

    impl<I, E> Spanned<I, E>
    where
        E: Clone,
        I: ToString,
    {
        pub fn new(element: E, span: I) -> Spanned<I, E> {
            Self { span, element }
        }
    }

    impl<I, E> Spanned<I, E>
    where
        E: Clone + ToString,
        I: ToString,
    {
        pub fn len(&self) -> usize {
            self.element.to_string().len()
        }
    }

    impl<I, E> ToString for Spanned<I, E>
    where
        E: Clone + ToString,
        I: ToString,
    {
        fn to_string(&self) -> String {
            self.element.to_string()
        }
    }

    impl<I, E> Deref for Spanned<I, E>
    where
        E: Clone,
        I: ToString,
    {
        type Target = E;

        fn deref(&self) -> &Self::Target {
            &self.element
        }
    }

    impl<I, E> DerefMut for Spanned<I, E>
    where
        E: Clone,
        I: ToString,
    {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.element
        }
    }

    #[derive(Clone, Eq, PartialEq, Hash)]
    pub struct RootScopeSelector<I, V> {
        pub name: I,
        pub version: V,
    }

    impl<I, V> RootScopeSelector<I, V> {
        pub fn new(name: I, version: V) -> Self {
            RootScopeSelector { name, version }
        }
    }

    impl<I: ToString, V: ToString> RootScopeSelector<I, V> {
        pub fn to_concrete(self) -> Result<RootScopeSelector<String, Version>, ParseErrs> {
            Ok(RootScopeSelector {
                name: self.name.to_string(),
                version: Version::from_str(self.version.to_string().as_str())?,
            })
        }
    }

    impl RouteScope {
        pub fn select(&self, directed: &DirectedWave) -> Vec<&WaveScope> {
            let mut scopes = vec![];
            for scope in &self.block {
                if scope.selector.is_match(directed).is_ok() {
                    scopes.push(scope);
                }
            }
            scopes
        }
    }

    #[derive(Clone)]
    pub struct RouteScopeSelector {
        pub selector: ScopeSelectorDef<String, Regex>,
    }

    impl RouteScopeSelector {
        pub fn new<I: ToString>(path: Option<I>) -> Result<Self, ParseErrs> {
            let path = match path {
                None => Regex::new(".*")?,
                Some(path) => Regex::new(path.to_string().as_str())?,
            };
            Ok(Self {
                selector: ScopeSelectorDef {
                    path,
                    name: "Route".to_string(),
                },
            })
        }

        pub fn from<I: ToString>(selector: LexScopeSelector<I>) -> Result<Self, ParseErrs> {
            if selector.name.to_string().as_str() != "Route" {
                return Err(ParseErrs::expected(
                    "",
                    "expected Route",
                    selector.name.to_string(),
                ));
            }
            let path = match selector.path {
                None => None,
                Some(path) => Some(path.to_string()),
            };

            Ok(RouteScopeSelector::new(path)?)
        }
    }

    impl Deref for RouteScopeSelector {
        type Target = ScopeSelectorDef<String, Regex>;

        fn deref(&self) -> &Self::Target {
            &self.selector
        }
    }

    #[derive(Clone)]
    pub struct ScopeSelectorDef<N, P> {
        pub name: N,
        pub path: P,
    }

    impl ValueMatcher<DirectedWave> for RouteScopeSelector {
        fn is_match(&self, directed: &DirectedWave) -> Result<(), ()> {
            if self.name.as_str() != "Route" {
                return Err(());
            }
            match self.selector.path.is_match(&directed.core().uri.path()) {
                true => Ok(()),
                false => Err(()),
            }
        }
    }

    impl ValueMatcher<SingularDirectedWave> for RouteScopeSelector {
        fn is_match(&self, directed: &SingularDirectedWave) -> Result<(), ()> {
            if self.name.as_str() != "Route" {
                return Err(());
            }
            match self.selector.path.is_match(&directed.core().uri.path()) {
                true => Ok(()),
                false => Err(()),
            }
        }
    }

    impl ValueMatcher<DirectedWave> for MessageScopeSelector {
        fn is_match(&self, directed: &DirectedWave) -> Result<(), ()> {
            self.name.is_match(&directed.core().method.kind())?;
            match self.path.is_match(&directed.core().uri.path()) {
                true => Ok(()),
                false => Err(()),
            }
        }
    }

    impl ValueMatcher<SingularDirectedWave> for MessageScopeSelector {
        fn is_match(&self, directed: &SingularDirectedWave) -> Result<(), ()> {
            self.name.is_match(&directed.core().method.kind())?;
            match self.path.is_match(&directed.core().uri.path()) {
                true => Ok(()),
                false => Err(()),
            }
        }
    }

    fn default_path<I: ToString>(path: Option<I>) -> Result<Regex, ParseErrs> {
        match path {
            None => Ok(Regex::new(".*")?),
            Some(path) => Ok(Regex::new(path.to_string().as_str())?),
        }
    }
    impl WaveScope {
        pub fn from_scope<I: Span>(scope: LexParentScope<I>) -> Result<Self, ParseErrs> {
            let selector = MessageScopeSelectorAndFilters::from_selector(scope.selector)?;
            let mut block = vec![];

            for scope in scope.block.into_iter() {
                let method = MethodScope::from_scope(&selector.selector.name, scope)?;
                block.push(method);
            }

            Ok(Self { selector, block })
        }

        pub fn select(&self, directed: &DirectedWave) -> Vec<&MethodScope> {
            let mut scopes = vec![];
            for scope in &self.block {
                if scope.selector.is_match(directed).is_ok() {
                    scopes.push(scope);
                }
            }
            scopes
        }
    }

    impl MessageScopeSelectorAndFilters {
        pub fn from_selector<I: Span>(selector: LexScopeSelector<I>) -> Result<Self, ParseErrs> {
            let filters = selector.filters.clone().to_scope_filters();
            let selector = MessageScopeSelector::from_selector(selector)?;
            Ok(Self { selector, filters })
        }
    }

    impl RouteScopeSelectorAndFilters {
        pub fn from_selector<I: Span>(selector: LexScopeSelector<I>) -> Result<Self, ParseErrs> {
            let filters = selector.filters.clone().to_scope_filters();
            let selector = RouteScopeSelector::new(selector.path.clone())?;
            Ok(Self { selector, filters })
        }
    }

    impl ValueMatcher<DirectedWave> for RouteScopeSelectorAndFilters {
        fn is_match(&self, request: &DirectedWave) -> Result<(), ()> {
            // nothing for filters at this time...
            self.selector.is_match(request)
        }
    }

    impl ValueMatcher<SingularDirectedWave> for RouteScopeSelectorAndFilters {
        fn is_match(&self, wave: &SingularDirectedWave) -> Result<(), ()> {
            // nothing for filters at this time...
            self.selector.is_match(wave)
        }
    }

    impl ValueMatcher<DirectedWave> for MessageScopeSelectorAndFilters {
        fn is_match(&self, request: &DirectedWave) -> Result<(), ()> {
            // nothing for filters at this time...
            self.selector.is_match(request)
        }
    }

    impl ValueMatcher<SingularDirectedWave> for MessageScopeSelectorAndFilters {
        fn is_match(&self, request: &SingularDirectedWave) -> Result<(), ()> {
            // nothing for filters at this time...
            self.selector.is_match(request)
        }
    }

    impl ValueMatcher<DirectedWave> for MethodScopeSelectorAndFilters {
        fn is_match(&self, directed: &DirectedWave) -> Result<(), ()> {
            // nothing for filters at this time...
            self.selector.is_match(directed)
        }
    }

    impl ValueMatcher<SingularDirectedWave> for MethodScopeSelectorAndFilters {
        fn is_match(&self, directed: &SingularDirectedWave) -> Result<(), ()> {
            // nothing for filters at this time...
            self.selector.is_match(directed)
        }
    }

    impl MethodScope {
        pub fn from_scope<I: Span>(
            parent: &ValuePattern<MethodKind>,
            scope: LexScope<I>,
        ) -> Result<Self, ParseErrs> {
            let selector = MethodScopeSelectorAndFilters::from_selector(parent, scope.selector)?;
            let block = result(pipeline(scope.block.content))?;
            Ok(Self { selector, block })
        }
    }

    impl MessageScopeSelector {
        pub fn from_selector<I: Span>(selector: LexScopeSelector<I>) -> Result<Self, ParseErrs> {
            let kind = match result(value_pattern(method_kind)(selector.name.clone())) {
                Ok(kind) => kind,
                Err(_) => {
                    return Err(ParseErrs::from_loc_span(
                        format!(
                            "unknown MessageKind: {} valid message kinds: Ext, Http, Cmd or *",
                            selector.name.to_string()
                        )
                        .as_str(),
                        "unknown message kind",
                        selector.name,
                    )
                    .into());
                }
            };

            Ok(Self {
                name: kind,
                path: default_path(selector.path)?,
            })
        }
    }

    impl ValueMatcher<DirectedWave> for MethodScopeSelector {
        fn is_match(&self, directed: &DirectedWave) -> Result<(), ()> {
            self.name.is_match(&directed.core().method)?;
            match self.path.is_match(&directed.core().uri.path()) {
                true => Ok(()),
                false => Err(()),
            }
        }
    }

    impl ValueMatcher<SingularDirectedWave> for MethodScopeSelector {
        fn is_match(&self, directed: &SingularDirectedWave) -> Result<(), ()> {
            self.name.is_match(&directed.core().method)?;
            match self.path.is_match(&directed.core().uri.path()) {
                true => Ok(()),
                false => Err(()),
            }
        }
    }
    impl MethodScopeSelectorAndFilters {
        pub fn from_selector<I: Span>(
            parent: &ValuePattern<MethodKind>,
            selector: LexScopeSelector<I>,
        ) -> Result<Self, ParseErrs> {
            let filters = selector.filters.clone().to_scope_filters();
            let selector = MethodScopeSelector::from_selector(parent, selector)?;
            Ok(Self { selector, filters })
        }
    }

    impl MethodScopeSelector {
        pub fn from_selector<I: Span>(
            parent: &ValuePattern<MethodKind>,
            selector: LexScopeSelector<I>,
        ) -> Result<Self, ParseErrs> {
            let name = match parent {
                ValuePattern::Always => ValuePattern::Always,
                ValuePattern::Never => ValuePattern::Never,
                ValuePattern::Pattern(message_kind) => match message_kind {
                    MethodKind::Hyp => {
                        match result(value_pattern(wrapped_sys_method)(selector.name.clone())) {
                            Ok(r) => r,
                            Err(_) => {
                                return Err(ParseErrs::from_loc_span(
                                    format!(
                                        "invalid Hyp method '{}'.  Hyp should be CamelCase",
                                        selector.name.to_string()
                                    )
                                    .as_str(),
                                    "invalid Hyp",
                                    selector.name,
                                )
                                .into())
                            }
                        }
                    }
                    MethodKind::Cmd => {
                        match result(value_pattern(wrapped_cmd_method)(selector.name.clone())) {
                            Ok(r) => r,
                            Err(_) => {
                                return Err(ParseErrs::from_loc_span(
                                    format!(
                                        "invalid Cmd method '{}'.  Cmd should be CamelCase",
                                        selector.name.to_string()
                                    )
                                    .as_str(),
                                    "invalid Cmd",
                                    selector.name,
                                )
                                .into())
                            }
                        }
                    }
                    MethodKind::Ext => {
                        match result(value_pattern(wrapped_ext_method)(selector.name.clone())) {
                            Ok(r) => r,
                            Err(_) => {
                                return Err(ParseErrs::from_loc_span(
                                    format!(
                                        "invalid Ext method '{}'.  Ext should be CamelCase",
                                        selector.name.to_string()
                                    )
                                    .as_str(),
                                    "invalid Ext",
                                    selector.name,
                                )
                                .into())
                            }
                        }
                    }
                    MethodKind::Http => {
                        match result(value_pattern(wrapped_http_method)(selector.name.clone())) {
                            Ok(r) => r,
                            Err(_) => {
                                return Err(ParseErrs::from_loc_span(format!("invalid Http Pattern '{}'.  Http should be camel case 'Get' and a valid Http method", selector.name.to_string()).as_str(), "invalid Http method", selector.name).into())
                            }
                        }
                    }
                },
            };

            Ok(Self {
                name,
                path: default_path(selector.path)?,
            })
        }
    }

    impl<N, P> ScopeSelectorDef<N, P> {
        pub fn new(name: N, path: P) -> Self {
            Self { name, path }
        }
    }

    #[derive(Clone)]
    pub struct LexScopeSelector<I> {
        pub name: I,
        pub filters: ScopeFiltersDef<I>,
        pub children: Option<I>,
        pub path: Option<I>,
    }

    impl<I: ToString> LexScopeSelector<I> {
        pub fn new(
            name: I,
            filters: ScopeFiltersDef<I>,
            path: Option<I>,
            children: Option<I>,
        ) -> Self {
            Self {
                name,
                filters,
                children,
                path,
            }
        }
    }

    impl<I> LexScopeSelector<I> {
        pub fn has_children(&self) -> bool {
            self.children.is_some()
        }
    }

    #[derive(Clone)]
    pub struct ScopeFiltersDef<I> {
        pub filters: Vec<ScopeFilterDef<I>>,
    }

    impl Default for ScopeFilters {
        fn default() -> Self {
            Self { filters: vec![] }
        }
    }

    impl<I> Deref for ScopeFiltersDef<I> {
        type Target = Vec<ScopeFilterDef<I>>;

        fn deref(&self) -> &Self::Target {
            &self.filters
        }
    }

    impl<I> ScopeFiltersDef<I> {
        pub fn is_empty(&self) -> bool {
            self.filters.is_empty()
        }
    }

    impl<I: ToString> ScopeFiltersDef<I> {
        pub fn to_scope_filters(self) -> ScopeFilters {
            ScopeFilters {
                filters: self
                    .filters
                    .into_iter()
                    .map(|f| f.to_scope_filter())
                    .collect(),
            }
        }

        pub fn len(&self) -> usize {
            self.filters.len()
        }

        pub fn empty() -> Self {
            Self { filters: vec![] }
        }
    }

    #[derive(Clone)]
    pub struct ScopeFilterDef<I> {
        pub name: I,
        pub args: Option<I>,
    }

    impl<I: ToString> ScopeFilterDef<I> {
        pub fn to_scope_filter(self) -> ScopeFilter {
            ScopeFilter {
                name: self.name.to_string(),
                args: match self.args {
                    None => None,
                    Some(args) => Some(args.to_string()),
                },
            }
        }
    }

    pub type RegexStr = String;
    pub type ScopeFilter = ScopeFilterDef<String>;
    pub type ScopeFilters = ScopeFiltersDef<String>;
    pub type LexBlock<I> = Block<I, ()>;
    pub type LexRootScope<I> = Scope<RootScopeSelector<I, Spanned<I, Version>>, Block<I, ()>, I>;
    pub type LexScope<I> = Scope<LexScopeSelector<I>, Block<I, ()>, I>;
    pub type LexParentScope<I> = Scope<LexScopeSelector<I>, Vec<LexScope<I>>, I>;

    //pub type LexPipelineScope<I> = PipelineScopeDef<I, VarPipeline>;
    pub type PipelineSegmentCtx = PipelineSegmentDef<PointCtx>;
    pub type PipelineSegmentVar = PipelineSegmentDef<PointVar>;

    #[derive(Debug, Clone)]
    pub struct PipelineSegmentDef<Pnt> {
        pub step: PipelineStepDef<Pnt>,
        pub stop: PipelineStopDef<Pnt>,
    }

    impl ToResolved<PipelineSegment> for PipelineSegmentVar {
        fn to_resolved(self, env: &Env) -> Result<PipelineSegment, ParseErrs> {
            let rtn: PipelineSegmentCtx = self.to_resolved(env)?;
            rtn.to_resolved(env)
        }
    }

    impl ToResolved<PipelineSegment> for PipelineSegmentCtx {
        fn to_resolved(self, env: &Env) -> Result<PipelineSegment, ParseErrs> {
            Ok(PipelineSegment {
                step: self.step.to_resolved(env)?,
                stop: self.stop.to_resolved(env)?,
            })
        }
    }

    impl ToResolved<PipelineSegmentCtx> for PipelineSegmentVar {
        fn to_resolved(self, env: &Env) -> Result<PipelineSegmentCtx, ParseErrs> {
            Ok(PipelineSegmentCtx {
                step: self.step.to_resolved(env)?,
                stop: self.stop.to_resolved(env)?,
            })
        }
    }

    /*
    impl CtxSubst<PipelineSegment> for PipelineSegmentCtx{
        fn resolve_ctx(self, resolver: &dyn CtxResolver) -> Result<PipelineSegment, ExtErr> {
            let mut errs = vec![];
            let step = match self.step.resolve_ctx(resolver) {
                Ok(step) => Some(step),
                Err(err) => {
                    errs.push(err);
                    None
                }
            };
            let stop = match self.stop.resolve_ctx(resolver) {
                Ok(stop) => Some(stop),
                Err(err) => {
                    errs.push(err);
                    None
                }
            };
            if errs.is_empty() {
                Ok(PipelineSegment {
                    step: step.expect("step"),
                    stop: stop.expect("stop")
                })
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }

     */

    pub type PipelineSegment = PipelineSegmentDef<Point>;
    pub type RouteScope = ScopeDef<RouteScopeSelectorAndFilters, Vec<WaveScope>>;
    pub type WaveScope = ScopeDef<MessageScopeSelectorAndFilters, Vec<MethodScope>>;
    pub type MethodScope = ScopeDef<MethodScopeSelectorAndFilters, PipelineVar>;
    //    pub type ValuePatternScopeSelector = ScopeSelectorDef<ValuePattern<String>, String,Regex>;
    pub type MessageScopeSelector = ScopeSelectorDef<ValuePattern<MethodKind>, Regex>;
    pub type MethodScopeSelector = ScopeSelectorDef<ValuePattern<Method>, Regex>;
    pub type RouteScopeSelectorAndFilters = ScopeSelectorAndFiltersDef<RouteScopeSelector, String>;
    pub type MessageScopeSelectorAndFilters =
        ScopeSelectorAndFiltersDef<MessageScopeSelector, String>;
    pub type MethodScopeSelectorAndFilters =
        ScopeSelectorAndFiltersDef<MethodScopeSelector, String>;

    /*    pub type ValuePatternScopeSelectorAndFilters =
           ScopeSelectorAndFiltersDef<ValuePatternScopeSelector, String>;

    */
    pub type LexScopeSelectorAndFilters<I> = ScopeSelectorAndFiltersDef<LexScopeSelector<I>, I>;
    //    pub type Pipeline = Vec<PipelineSegment>;

    impl<I: Span> TryFrom<LexParentScope<I>> for RouteScope {
        type Error = ParseErrs;

        fn try_from(scope: LexParentScope<I>) -> Result<Self, Self::Error> {
            let mut errs = vec![];
            let mut message_scopes = vec![];
            let route_selector = RouteScopeSelectorAndFilters::from_selector(scope.selector)?;
            for message_scope in scope.block {
                match lex_child_scopes(message_scope) {
                    Ok(message_scope) => match WaveScope::from_scope(message_scope) {
                        Ok(message_scope) => message_scopes.push(message_scope),
                        Err(err) => errs.push(err),
                    },
                    Err(err) => {
                        errs.push(err);
                    }
                }
            }
            if errs.is_empty() {
                Ok(RouteScope {
                    selector: route_selector,
                    block: message_scopes,
                })
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }

    /*
    impl<I: Span> LexScopeSelectorAndFilters<I> {
        pub fn to_value_pattern_scope_selector(
            self,
        ) -> Result<ValuePatternScopeSelectorAndFilters, ExtErr> {
            Ok(ValuePatternScopeSelectorAndFilters {
                selector: self.selector.to_value_pattern_scope_selector()?,
                filters: self.filters.to_scope_filters(),
            })
        }
    }

     */

    /*
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct VarPipelineSegmentDef<Step, Stop> {
        pub step: Step,
        pub stop: Stop,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PipelineSegmentDef<Pnt> {
        pub step: PipelineStepDef<Pnt>,
        pub stop: PipelineStopDef<Pnt>,
    }

     */

    pub type Pipeline = PipelineDef<PipelineSegment>;
    pub type PipelineCtx = PipelineDef<PipelineSegmentCtx>;
    pub type PipelineVar = PipelineDef<PipelineSegmentVar>;

    impl ToResolved<Pipeline> for PipelineCtx {
        fn to_resolved(self, env: &Env) -> Result<Pipeline, ParseErrs> {
            let mut segments = vec![];
            for segment in self.segments.into_iter() {
                segments.push(segment.to_resolved(env)?);
            }

            Ok(Pipeline { segments })
        }
    }

    impl ToResolved<PipelineCtx> for PipelineVar {
        fn to_resolved(self, env: &Env) -> Result<PipelineCtx, ParseErrs> {
            let mut segments = vec![];
            for segment in self.segments.into_iter() {
                segments.push(segment.to_resolved(env)?);
            }

            Ok(PipelineCtx { segments })
        }
    }

    /*
    impl CtxSubst<Pipeline> for PipelineCtx {
        fn resolve_ctx(self, resolver: &dyn CtxResolver) -> Result<Pipeline, ExtErr> {
            let mut errs = vec![];
            let mut segments = vec![];
            for segment in self.segments {
                match segment.resolve_ctx(resolver) {
                    Ok(segment) => segments.push(segment),
                    Err(err) => errs.push(err)
                }
            }
            if errs.is_empty() {
                Ok( Pipeline { segments })
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }

     */

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PipelineDef<S> {
        pub segments: Vec<S>,
    }

    impl<S> PipelineDef<S> {
        pub fn new() -> Self {
            Self { segments: vec![] }
        }
    }

    impl<S> Deref for PipelineDef<S> {
        type Target = Vec<S>;

        fn deref(&self) -> &Self::Target {
            &self.segments
        }
    }

    impl<S> DerefMut for PipelineDef<S> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.segments
        }
    }

    impl<S> PipelineDef<S> {
        pub fn consume(&mut self) -> Option<S> {
            if self.segments.is_empty() {
                None
            } else {
                Some(self.segments.remove(0))
            }
        }
    }

    /*
    impl <I:Span> VarSubst<PipelineCtx> for VarPipeline<I> {
        fn resolve_vars(self, resolver: &dyn VarResolver) -> Result<PipelineCtx, ExtErr> {
            let mut pipeline = PipelineCtx::new();
            let mut errs = vec![];
            for segment in self.segments {
                match segment.resolve_vars(resolver) {
                    Ok(segment) => {
                        pipeline.segments.push(segment);
                    }
                    Err(err) => {
                        errs.push(err);
                    }
                }
            }

            if errs.is_empty() {
                Ok(pipeline)
            } else {
                Err(ParseErrs::fold(errs).into())
            }
        }
    }

     */

    /*
    impl <I:Span> VarSubst<PipelineSegmentCtx> for VarPipelineSegment<I> {
        fn resolve_vars(self, resolver: &dyn VarResolver) -> Result<PipelineSegmentCtx, ExtErr> {
            unimplemented!()
            /*
            let mut errs = vec![];

            if self.stop.is_none() {
                errs.push(ParseErrs::from_owned_span(
                    "expecting Pipeline Stop to follow Pipeline Step",
                    "Needs a following Pipeline Stop",
                    self.step.span(),
                ));
            }

            let step = match self.step.resolve(resolver) {
                Ok(step) => Some(step),
                Err(err) => {
                    errs.push(err);
                    None
                }
            };

            let stop = match self.stop {
                Some(stop) => match stop.resolve(resolver) {
                    Ok(stop) => Some(stop),
                    Err(err) => {
                        errs.push(err);
                        None
                    }
                },
                None => None,
            };

            if step.is_some() && stop.is_some() && errs.is_empty() {
                let step = step.expect("step");
                let stop = stop.expect("stop");
                Ok(PipelineSegmentCtx { step, stop })
            } else {
                Err(ParseErrs::fold(errs).into())
            }

             */
        }
    }

     */

    #[derive(Clone)]
    pub enum MechtronScope {
        WasmScope(Vec<Assignment>),
    }

    #[derive(Clone)]
    pub enum BindScope {
        RequestScope(RouteScope),
    }

    #[derive(Debug, Clone)]
    pub struct ScopeDef<S, B> {
        pub selector: S,
        pub block: B,
    }

    #[derive(Clone)]
    pub enum BindScopeKind {
        Pipelines,
    }

    #[derive(Clone)]
    pub enum BuiltInFilter {
        Auth,
        NoAuth,
    }

    #[derive(Clone)]
    pub struct Scope<S, B, P>
    where
        S: Clone,
    {
        pub selector: S,
        pub pipeline_step: Option<P>,
        pub block: B,
    }

    impl<S, B, P> Scope<S, B, P>
    where
        S: Clone,
    {
        pub fn new(selector: S, block: B) -> Self {
            Self {
                selector,
                block,
                pipeline_step: None,
            }
        }

        pub fn new_with_pipeline_step(selector: S, block: B, pipeline_step: Option<P>) -> Self {
            Self {
                selector,
                block,
                pipeline_step,
            }
        }
    }

    impl<S, FromBlock, P> Scope<S, FromBlock, P>
    where
        S: Clone,
    {
        pub fn upgrade<ToBlock>(self, block: ToBlock) -> Scope<S, ToBlock, P> {
            Scope {
                selector: self.selector,
                block,
                pipeline_step: self.pipeline_step,
            }
        }
    }

    #[derive(Clone)]
    pub struct Block<I, D> {
        pub kind: BlockKind,
        pub content: I,
        pub data: D,
    }

    impl<I> Block<I, ()> {
        pub fn parse(kind: BlockKind, content: I) -> Block<I, ()> {
            Block {
                kind,
                content,
                data: (),
            }
        }
    }

    #[derive(Debug, Copy, Clone, Error, Eq, PartialEq)]
    pub enum BlockKind {
        #[error("nexted block")]
        Nested(#[from] NestedBlockKind),
        #[error("terminated")]
        Terminated(#[from] TerminatedBlockKind),
        #[error("delimited")]
        Delimited(#[from] DelimitedBlockKind),
        #[error("partial")]
        Partial,
    }

    #[derive(Debug, Copy, Clone, Error, Eq, PartialEq)]
    pub enum TerminatedBlockKind {
        #[error("semicolon")]
        Semicolon,
    }

    impl TerminatedBlockKind {
        pub fn tag(&self) -> &'static str {
            match self {
                TerminatedBlockKind::Semicolon => ";",
            }
        }

        pub fn as_char(&self) -> char {
            match self {
                TerminatedBlockKind::Semicolon => ';',
            }
        }
    }

    #[derive(Debug, Copy, Clone, Error, Eq, PartialEq)]
    pub enum DelimitedBlockKind {
        #[error("single quotes")]
        SingleQuotes,
        #[error("double quotes")]
        DoubleQuotes,
    }

    impl DelimitedBlockKind {
        pub fn delim(&self) -> &'static str {
            match self {
                DelimitedBlockKind::SingleQuotes => "'",
                DelimitedBlockKind::DoubleQuotes => "\"",
            }
        }

        pub fn escaped(&self) -> &'static str {
            match self {
                DelimitedBlockKind::SingleQuotes => "\'",
                DelimitedBlockKind::DoubleQuotes => "\"",
            }
        }

        pub fn context(&self) -> &'static str {
            match self {
                DelimitedBlockKind::SingleQuotes => "single:quotes:block",
                DelimitedBlockKind::DoubleQuotes => "double:quotes:block",
            }
        }

        pub fn missing_close_context(&self) -> &'static str {
            match self {
                DelimitedBlockKind::SingleQuotes => "single:quotes:block:missing-close",
                DelimitedBlockKind::DoubleQuotes => "double:quotes:block:missing-close",
            }
        }
    }

    #[derive(Debug, Copy, Clone, Error, Eq, PartialEq)]
    pub enum NestedBlockKind {
        #[error("curly")]
        Curly,
        #[error("parenthesis")]
        Parens,
        #[error("square")]
        Square,
        #[error("angle")]
        Angle,
    }

    impl NestedBlockKind {

        pub fn unwrap<I: Span, F, O>(&self, mut f: F) -> impl FnMut(I) -> Res<I, O>
        where
            F: FnMut(I) -> Res<I, O>,
        {
            unwrap_block(BlockKind::Nested(self.clone()), f )
        }

        pub fn wrap(&self, string: impl AsRef<str>) -> String {
            format!("{}{}{}", self.open(), string.as_ref(), self.close()).to_string()
        }

        pub fn is_block_terminator(c: char) -> bool {
            match c {
                '}' => true,
                ')' => true,
                ']' => true,
                '>' => true,
                _ => false,
            }
        }

        pub fn error_message<I: Span>(span: &I, context: &str) -> Result<&'static str, ()> {
            if Self::Curly.open_context() == context {
                Ok("expecting '{' (open scope block)")
            } else if Self::Parens.open_context() == context {
                Ok("expecting '(' (open scope block)")
            } else if Self::Angle.open_context() == context {
                Ok("expecting '<' (open scope block)")
            } else if Self::Square.open_context() == context {
                Ok("expecting '[' (open scope block)")
            } else if Self::Curly.close_context() == context {
                Ok("expecting '}' (close scope block)")
            } else if Self::Parens.close_context() == context {
                Ok("expecting ')' (close scope block)")
            } else if Self::Angle.close_context() == context {
                Ok("expecting '>' (close scope block)")
            } else if Self::Square.close_context() == context {
                Ok("expecting ']' (close scope block)")
            } else if Self::Curly.unpaired_closing_scope() == context {
                Ok("closing scope without an opening scope")
            } else if Self::Parens.unpaired_closing_scope() == context {
                Ok("closing scope without an opening scope")
            } else if Self::Angle.unpaired_closing_scope() == context {
                Ok("closing scope without an opening scope")
            } else if Self::Square.unpaired_closing_scope() == context {
                Ok("closing scope without an opening scope")
            } else {
                Err(())
            }
        }

        pub fn context(&self) -> &'static str {
            match self {
                NestedBlockKind::Curly => "block:{}",
                NestedBlockKind::Parens => "block:()",
                NestedBlockKind::Square => "block:[]",
                NestedBlockKind::Angle => "block:<>",
            }
        }

        pub fn open_context(&self) -> &'static str {
            match self {
                NestedBlockKind::Curly => "block:open:{",
                NestedBlockKind::Parens => "block:open:(",
                NestedBlockKind::Square => "block:open:[",
                NestedBlockKind::Angle => "block:open:<",
            }
        }

        pub fn close_context(&self) -> &'static str {
            match self {
                NestedBlockKind::Curly => "block:close:}",
                NestedBlockKind::Parens => "block:close:)",
                NestedBlockKind::Square => "block:close:]",
                NestedBlockKind::Angle => "block:close:>",
            }
        }

        pub fn unpaired_closing_scope(&self) -> &'static str {
            match self {
                NestedBlockKind::Curly => "block:close-before-open:}",
                NestedBlockKind::Parens => "block:close-before-open:)",
                NestedBlockKind::Square => "block:close-before-open:]",
                NestedBlockKind::Angle => "block:close-before-open:>",
            }
        }

        pub fn open(&self) -> &'static str {
            match self {
                NestedBlockKind::Curly => "{",
                NestedBlockKind::Parens => "(",
                NestedBlockKind::Square => "[",
                NestedBlockKind::Angle => "<",
            }
        }

        pub fn close(&self) -> &'static str {
            match self {
                NestedBlockKind::Curly => "}",
                NestedBlockKind::Parens => ")",
                NestedBlockKind::Square => "]",
                NestedBlockKind::Angle => ">",
            }
        }

        pub fn open_as_char(&self) -> char {
            match self {
                NestedBlockKind::Curly => '{',
                NestedBlockKind::Parens => '(',
                NestedBlockKind::Square => '[',
                NestedBlockKind::Angle => '<',
            }
        }

        pub fn close_as_char(&self) -> char {
            match self {
                NestedBlockKind::Curly => '}',
                NestedBlockKind::Parens => ')',
                NestedBlockKind::Square => ']',
                NestedBlockKind::Angle => '>',
            }
        }
    }

    pub enum TextType<I> {
        Comment(I),
        NoComment(I),
    }

    impl<I: ToString> ToString for TextType<I> {
        fn to_string(&self) -> String {
            match self {
                TextType::Comment(i) => i.to_string(),
                TextType::NoComment(i) => i.to_string(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub enum Chunk<I> {
        Var(I),
        Text(I),
    }
    impl<I> Chunk<I>
    where
        I: Span,
    {
        pub fn stringify(self) -> Chunk<Tw<String>> {
            match self {
                Chunk::Var(var) => Chunk::Var(Tw::new(var.clone(), var.to_string())),
                Chunk::Text(text) => Chunk::Text(Tw::new(text.clone(), text.to_string())),
            }
        }
    }

    impl<I> Chunk<I> {
        pub fn span(&self) -> &I {
            match self {
                Chunk::Var(var) => var,
                Chunk::Text(text) => text,
            }
        }
    }

    impl<I: ToString> Chunk<I> {
        pub fn len(&self) -> usize {
            match self {
                Chunk::Var(var) => {
                    // account for ${}
                    var.to_string().len() + 3
                }
                Chunk::Text(text) => text.to_string().len(),
            }
        }
    }

    #[derive(Clone)]
    pub enum Var<O, P>
    where
        P: VarParser<O>,
    {
        Val(O),
        Var { name: String, parser: P },
    }

    pub trait VarParser<O> {
        fn parse<I: Span>(input: I) -> Result<O, ParseErrs>;
    }

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
    pub struct Subst<I> {
        pub chunks: Vec<Chunk<I>>,
        pub trace: Trace,
    }

    impl Subst<Tw<String>> {
        pub fn new(path: &str) -> Result<Self, ParseErrs> {
            let path = result(subst_path(new_span(path)))?;
            Ok(path.stringify())
        }
    }

    impl<I> Subst<I>
    where
        I: Span,
    {
        pub fn stringify(self) -> Subst<Tw<String>> {
            let chunks: Vec<Chunk<Tw<String>>> =
                self.chunks.into_iter().map(|c| c.stringify()).collect();
            Subst {
                chunks,
                trace: self.trace,
            }
        }
    }

    impl<I> ToString for Subst<I>
    where
        I: ToString,
    {
        fn to_string(&self) -> String {
            let mut rtn = String::new();
            for chunk in &self.chunks {
                match chunk {
                    Chunk::Var(var) => {
                        rtn.push_str(format!("${{{}}}", var.to_string()).as_str());
                    }
                    Chunk::Text(text) => {
                        rtn.push_str(text.to_string().as_str());
                    }
                }
            }
            rtn
        }
    }

    impl ToResolved<String> for Subst<Tw<String>> {
        fn to_resolved(self, env: &Env) -> Result<String, ParseErrs> {
            let mut rtn = String::new();
            let mut errs = vec![];
            for chunk in self.chunks {
                match chunk {
                    Chunk::Var(var) => match env.val(var.to_string().as_str()) {
                        Ok(val) => {
                            let val: String = val.clone().try_into()?;
                            rtn.push_str(val.as_str());
                        }
                        Err(err) => {
                            errs.push(ParseErrs::from_range(
                                format!("could not find variable: {}", var.to_string()).as_str(),
                                "not found",
                                var.trace.range,
                                var.trace.extra,
                            ));
                        }
                    },
                    Chunk::Text(text) => {
                        rtn.push_str(text.to_string().as_str());
                    }
                }
            }

            if errs.is_empty() {
                Ok(rtn)
            } else {
                let errs = ParseErrs::fold(errs);
                Err(errs.into())
            }
        }
    }
}

fn create_command<I: Span>(input: I) -> Res<I, CommandVar> {
    tuple((tag("create"), create))(input)
        .map(|(next, (_, create))| (next, CommandVar::Create(create)))
}

pub fn publish_command<I: Span>(input: I) -> Res<I, CommandVar> {
    tuple((tag("publish"), space1, publish))(input)
        .map(|(next, (_, _, create))| (next, CommandVar::Create(create)))
}

fn select_command<I: Span>(input: I) -> Res<I, CommandVar> {
    tuple((tag("select"), space1, select))(input)
        .map(|(next, (_, _, select))| (next, CommandVar::Select(select)))
}

fn set_command<I: Span>(input: I) -> Res<I, CommandVar> {
    tuple((tag("set"), space1, set))(input).map(|(next, (_, _, set))| (next, CommandVar::Set(set)))
}

fn get_command<I: Span>(input: I) -> Res<I, CommandVar> {
    tuple((tag("get"), space1, get))(input).map(|(next, (_, _, get))| (next, CommandVar::Get(get)))
}

pub fn command_strategy<I: Span>(input: I) -> Res<I, Strategy> {
    opt(tuple((tag("?"), multispace0)))(input).map(|(next, hint)| match hint {
        None => (next, Strategy::Commit),
        Some(_) => (next, Strategy::Ensure),
    })
}

pub fn command<I: Span>(input: I) -> Res<I, CommandVar> {
    context(
        "command",
        alt((
            create_command,
            publish_command,
            select_command,
            set_command,
            get_command,
            fail,
        )),
    )(input)
}

pub fn command_line<I: Span>(input: I) -> Res<I, CommandVar> {
    tuple((
        multispace0,
        command,
        multispace0,
        opt(tag(";")),
        multispace0,
    ))(input)
    .map(|(next, (_, command, _, _, _))| (next, command))
}

pub fn script_line<I: Span>(input: I) -> Res<I, CommandVar> {
    tuple((multispace0, command, multispace0, tag(";"), multispace0))(input)
        .map(|(next, (_, command, _, _, _))| (next, command))
}

pub fn script<I: Span>(input: I) -> Res<I, Vec<CommandVar>> {
    many0(script_line)(input)
}

pub fn consume_command_line<I: Span>(input: I) -> Res<I, CommandVar> {
    all_consuming(command_line)(input)
}

pub fn rec_script_line<I: Span>(input: I) -> Res<I, I> {
    recognize(script_line)(input)
}

pub fn layer<I: Span>(input: I) -> Res<I, Layer> {
    let (next, layer) = recognize(camel_case)(input.clone())?;
    match Layer::from_str(layer.to_string().as_str()) {
        Ok(layer) => Ok((next, layer)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            input,
            ErrorKind::Alpha,
        ))),
    }
}

fn topic_uuid<I: Span>(input: I) -> Res<I, Topic> {
    delimited(tag("Topic<Uuid>("), parse_uuid, tag(")"))(input)
        .map(|(next, uuid)| ((next, Topic::Uuid(uuid))))
}

fn topic_cli<I: Span>(input: I) -> Res<I, Topic> {
    value(Topic::Cli, tag("Topic<Cli>"))(input)
}

fn topic_path<I: Span>(input: I) -> Res<I, Topic> {
    delimited(tag("Topic<Path>("), many1(skewer_case), tag(")"))(input)
        .map(|(next, segments)| ((next, Topic::Path(segments))))
}

fn topic_any<I: Span>(input: I) -> Res<I, Topic> {
    context("Topic<*>", value(Topic::Any, tag("Topic<*>")))(input)
}

fn topic_not<I: Span>(input: I) -> Res<I, Topic> {
    context("Topic<Not>", value(Topic::Not, tag("Topic<!>")))(input)
}

pub fn topic_none<I: Span>(input: I) -> Res<I, Topic> {
    Ok((input, Topic::None))
}

pub fn topic<I: Span>(input: I) -> Res<I, Topic> {
    context(
        "topic",
        alt((topic_cli, topic_path, topic_uuid, topic_any, topic_not)),
    )(input)
}

pub fn topic_or_none<I: Span>(input: I) -> Res<I, Topic> {
    context("topic_or_none", alt((topic, topic_none)))(input)
}

pub fn plus_topic_or_none<I: Span>(input: I) -> Res<I, Topic> {
    context(
        "plus_topic_or_none",
        alt((preceded(tag("+"), topic), topic_none)),
    )(input)
}

pub fn port<I: Span>(input: I) -> Res<I, Surface> {
    let (next, (point, layer, topic)) = context(
        "port",
        tuple((
            terminated(tw(point_var), tag("@")),
            layer,
            plus_topic_or_none,
        )),
    )(input.clone())?;

    match point.w.collapse() {
        Ok(point) => Ok((next, Surface::new(point, layer, topic))),
        Err(err) => {
            let err = NomErr::from_error_kind(input.clone(), ErrorKind::Alpha);
            let loc = input.slice(point.trace.range);
            Err(nom::Err::Error(NomErr::add_context(
                loc,
                ErrCtx::ResolverNotAvailable,
                err,
            )))
        }
    }
}

pub type SurfaceSelectorVal =
    SurfaceSelectorDef<PointSegKindHop, VarVal<Topic>, VarVal<ValuePattern<Layer>>>;
pub type SurfaceSelectorCtx = SurfaceSelectorDef<PointSegKindHop, Topic, ValuePattern<Layer>>;
pub type SurfaceSelector = SurfaceSelectorDef<PointSegKindHop, Topic, ValuePattern<Layer>>;

pub struct SurfaceSelectorDef<Hop, Topic, Layer> {
    point: SelectorDef<Hop>,
    topic: Topic,
    layer: Layer,
}

pub struct KindLex {
    pub base: CamelCase,
    pub sub: Option<CamelCase>,
    pub specific: Option<Specific>,
}

impl TryInto<KindParts> for KindLex {
    type Error = ParseErrs;

    fn try_into(self) -> Result<KindParts, Self::Error> {
        Ok(KindParts {
            base: BaseKind::try_from(self.base)?,
            sub: self.sub,
            specific: self.specific,
        })
    }
}

pub fn expect<I, O, F>(mut f: F) -> impl FnMut(I) -> Res<I, O>
where
    F: FnMut(I) -> Res<I, O> + Copy,
{
    move |i: I| {
        f(i).map_err(|e| match e {
            Err::Incomplete(i) => Err::Incomplete(i),
            Err::Error(e) => Err::Failure(e),
            Err::Failure(e) => Err::Failure(e),
        })
    }
}

#[cfg(test)]
pub mod cmd_test {
    use core::str::FromStr;

    use crate::command::direct::create::KindTemplate;
    use crate::command::{Command, CommandVar};
    use crate::err::ParseErrs;
    use crate::kind::{BaseKind, Kind};
    use crate::parse::util::{new_span, result};
    use crate::point::{PointSeg, RouteSeg};
    use crate::selector::{PointHierarchy, PointKindSeg};
    use crate::util::ToResolved;

    use crate::parse::{
        command, create_command, point_selector, publish_command, script, upload_blocks, CamelCase,
    };
    /*
    #[mem]
    pub async fn test2() -> Result<(),Error>{
        let input = "? xreate localhost<Space>";
        let x: Result<CommandOp,VerboseError<&str>> = final_parser(command)(input);
        match x {
            Ok(_) => {}
            Err(err) => {
                println!("err: {}", err.to_string())
            }
        }


        Ok(())
    }

     */

    //    #[test]
    pub fn test() -> Result<(), ParseErrs> {
        let input = "xreate? localhost<Space>";
        match command(new_span(input)) {
            Ok(_) => {}
            Err(nom::Err::Error(e)) => {
                eprintln!("yikes!");
                return Err("could not find context".into());
            }
            Err(e) => {
                return Err("some err".into());
            }
        }
        Ok(())
    }

    #[test]
    pub fn test_kind() -> Result<(), ParseErrs> {
        let input = "create localhost:users<UserBase<Keycloak>>";
        let (_, command) = command(new_span(input))?;
        match command {
            CommandVar::Create(create) => {
                assert_eq!(
                    create.template.kind.sub,
                    Some(CamelCase::from_str("Keycloak").unwrap())
                );
            }
            _ => {
                panic!("expected create command")
            }
        }
        Ok(())
    }

    #[test]
    pub fn test_script() -> Result<(), ParseErrs> {
        let input = r#" create? localhost<Space>;
 Xcrete localhost:repo<Base<Repo>>;
 create? localhost:repo:tutorial<ArtifactBundleSeries>;
 publish? ^[ bundle.zip ]-> localhost:repo:tutorial:1.0.0;
 set localhost{ +bind=localhost:repo:tutorial:1.0.0:/bind/localhost.bind } ;
        "#;

        script(new_span(input))?;
        Ok(())
    }

    #[test]
    pub fn test_publish() -> Result<(), ParseErrs> {
        let input = r#"publish ^[ bundle.zip ]-> localhost:repo:tutorial:1.0.0"#;
        publish_command(new_span(input))?;
        Ok(())
    }

    #[test]
    pub fn test_upload_blocks() -> Result<(), ParseErrs> {
        let input = r#"publish ^[ bundle.zip ]-> localhost:repo:tutorial:1.0.0"#;
        let blocks = result(upload_blocks(new_span(input)))?;
        assert_eq!(1, blocks.len());
        let block = blocks.get(0).unwrap();
        assert_eq!("bundle.zip", block.name.as_str());

        // this should fail bcause it has multiple ^[
        let input = r#"publish ^[ ^[ bundle.zip ]-> localhost:repo:tutorial:1.0.0"#;
        let blocks = result(upload_blocks(new_span(input)))?;
        assert_eq!(0, blocks.len());

        // this should fail bcause it has no ^[
        let input = r#"publish localhost:repo:tutorial:1.0.0"#;
        let blocks = result(upload_blocks(new_span(input)))?;
        assert_eq!(0, blocks.len());

        Ok(())
    }

    #[test]
    pub fn test_create_kind() -> Result<(), ParseErrs> {
        let input = r#"create localhost:repo:tutorial:1.0.0<Repo>"#;
        let mut command = result(create_command(new_span(input)))?;
        let command = command.collapse()?;
        if let Command::Create(create) = command {
            let kind = KindTemplate {
                base: BaseKind::Repo,
                sub: None,
                specific: None,
            };
            assert_eq!(create.template.kind, kind);
        } else {
            assert!(false);
        }

        Ok(())
    }

    #[test]
    pub fn test_create_properties() -> Result<(), ParseErrs> {
        let input = r#"create localhost:repo:tutorial:1.0.0<Repo>{ +config=the:cool:property }"#;
        let mut command = result(create_command(new_span(input)))?;
        let command = command.collapse()?;
        if let Command::Create(create) = command {
            assert!(create.properties.get("config").is_some());
        } else {
            assert!(false);
        }

        Ok(())
    }

    #[test]
    pub fn test_selector() {
        let less = PointHierarchy::new(
            RouteSeg::Local,
            vec![PointKindSeg {
                segment: PointSeg::Base("less".to_string()),
                kind: Kind::Base,
            }],
        );

        let fae = PointHierarchy::new(
            RouteSeg::Local,
            vec![
                PointKindSeg {
                    segment: PointSeg::Base("fae".to_string()),
                    kind: Kind::Base,
                },
                PointKindSeg {
                    segment: PointSeg::Base("dra".to_string()),
                    kind: Kind::User,
                },
            ],
        );

        assert!(result(point_selector(new_span("less")))
            .unwrap()
            .matches_found(&less));
        assert!(result(point_selector(new_span("*")))
            .unwrap()
            .matches_found(&less));
        assert!(!result(point_selector(new_span("*")))
            .unwrap()
            .matches_found(&fae));
        assert!(result(point_selector(new_span("*:dra")))
            .unwrap()
            .matches_found(&fae));
        assert!(!result(point_selector(new_span("*:dra")))
            .unwrap()
            .matches_found(&less));
        assert!(result(point_selector(new_span("fae:*")))
            .unwrap()
            .matches_found(&fae));
        assert!(result(point_selector(new_span("**<User>")))
            .unwrap()
            .matches_found(&fae));
        assert!(!result(point_selector(new_span("**<User>")))
            .unwrap()
            .matches_found(&less));
        assert!(result(point_selector(new_span("**")))
            .unwrap()
            .matches_found(&less));
        assert!(result(point_selector(new_span("**")))
            .unwrap()
            .matches_found(&fae));
        assert!(!result(point_selector(new_span("**<Base>")))
            .unwrap()
            .matches_found(&fae));

        let less = result(point_selector(new_span("less"))).unwrap();
    }
}

fn inclusive_any_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    alt((tag("+*"), tag("ROOT+*")))(input).map(|(next, _)| (next, PointSegSelector::InclusiveAny))
}

fn inclusive_recursive_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    alt((tag("+**"), tag("ROOT+**")))(input)
        .map(|(next, _)| (next, PointSegSelector::InclusiveRecursive))
}

fn any_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    tag("*")(input).map(|(next, _)| (next, PointSegSelector::Any))
}

fn recursive_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    tag("**")(input).map(|(next, _)| (next, PointSegSelector::Recursive))
}

fn exact_space_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    point_segment_chars(input).map(|(next, segment)| {
        (
            next,
            PointSegSelector::Exact(ExactPointSeg::PointSeg(PointSeg::Space(
                segment.to_string(),
            ))),
        )
    })
}

fn exact_base_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    point_segment_chars(input).map(|(next, segment)| {
        (
            next,
            PointSegSelector::Exact(ExactPointSeg::PointSeg(PointSeg::Base(segment.to_string()))),
        )
    })
}

fn exact_file_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    file_chars(input).map(|(next, segment)| {
        (
            next,
            PointSegSelector::Exact(ExactPointSeg::PointSeg(PointSeg::File(segment.to_string()))),
        )
    })
}

fn exact_dir_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    file_chars(input).map(|(next, segment)| {
        (
            next,
            PointSegSelector::Exact(ExactPointSeg::PointSeg(PointSeg::Dir(segment.to_string()))),
        )
    })
}

pub fn parse_version_chars_str<I: Span, O: FromStr>(input: I) -> Res<I, O> {
    let (next, rtn) = recognize(version_chars)(input)?;
    match O::from_str(rtn.to_string().as_str()) {
        Ok(rtn) => Ok((next, rtn)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            next,
            ErrorKind::Fail,
        ))),
    }
}

fn exact_version_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    version_req(input).map(|(next, version_req)| (next, PointSegSelector::Version(version_req)))
}

fn version_req_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    delimited(tag("("), version_req, tag(")"))(input)
        .map(|(next, version_req)| (next, PointSegSelector::Version(version_req)))
}

pub fn point_segment_selector<I: Span>(input: I) -> Res<I, PointSegSelector> {
    alt((
        inclusive_recursive_segment,
        inclusive_any_segment,
        recursive_segment,
        any_segment,
        exact_space_segment,
    ))(input)
}

fn base_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    alt((recursive_segment, any_segment, exact_base_segment))(input)
}

fn file_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    alt((recursive_segment, any_segment, exact_file_segment))(input)
}

fn dir_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    terminated(
        alt((recursive_segment, any_segment, exact_dir_segment)),
        tag("/"),
    )(input)
}

fn dir_segment_meat<I: Span>(input: I) -> Res<I, PointSegSelector> {
    alt((recursive_segment, any_segment, exact_dir_segment))(input)
}

fn version_segment<I: Span>(input: I) -> Res<I, PointSegSelector> {
    alt((
        recursive_segment,
        any_segment,
        exact_version_segment,
        version_req_segment,
    ))(input)
}

pub fn parse_star_key<I: Span>(input: I) -> Res<I, StarKey> {
    let (next, (_, constelation, _, name, index)) = context(
        "star",
        tuple((
            tag("STAR::"),
            lowercase_alphanumeric,
            tag(":"),
            lowercase_alphanumeric,
            delimited(tag("["), digit1, tag("]")),
        )),
    )(input.clone())?;
    let constelation = constelation.to_string();
    let name = name.to_string();
    let index = match index.to_string().parse::<u16>() {
        Ok(index) => index,
        Err(err) => {
            return Err(nom::Err::Failure(NomErr::from_error_kind(
                input,
                ErrorKind::Digit,
            )))
        }
    };

    Ok((
        next,
        StarKey {
            constellation: constelation,
            name,
            index,
        },
    ))
}

pub fn pattern<I: Span, O, V>(mut value: V) -> impl FnMut(I) -> Res<I, Pattern<O>>+Copy
where
    V: Parser<I, O, NomErr<I>>+Copy,
{
    move |input: I| {
        let x: Res<I, I> = tag("*")(input.clone());
        match x {
            Ok((next, _)) => Ok((next, Pattern::Always)),
            Err(_) => {
                let (next, p) = value.parse(input)?;
                let pattern = Pattern::Exact(p);
                Ok((next, pattern))
            }
        }
    }
}

pub fn value_pattern<I: Span, O, F>(mut f: F) -> impl FnMut(I) -> Res<I, ValuePattern<O>>
where
    I: InputLength + InputTake + Compare<&'static str>,
    F: Parser<I, O, NomErr<I>>,
{
    move |input: I| match tag::<&'static str, I, NomErr<I>>("*")(input.clone()) {
        Ok((next, _)) => Ok((next, ValuePattern::Always)),
        Err(err) => f
            .parse(input.clone())
            .map(|(next, res)| (next, ValuePattern::Pattern(res))),
    }
}

pub fn version_req<I: Span>(input: I) -> Res<I, VersionReq> {
    let (next, version) = version_req_chars(input.clone())?;
    let version = version.to_string();
    let str_input = version.as_str();
    let rtn = semver::VersionReq::parse(str_input);

    match rtn {
        Ok(version) => Ok((next, VersionReq { version })),
        Err(err) => {
            let tree = Err::Error(NomErr::from_error_kind(input, ErrorKind::Fail));
            Err(tree)
        }
    }
}

fn rec_domain<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((
        many1(terminated(skewer_chars, tag("."))),
        skewer_chars,
    )))(input)
}

// can be a hostname or domain name
fn space<I: Span>(input: I) -> Res<I, I> {
    recognize(alt((skewer_chars, rec_domain)))(input)
}

pub fn specific_selector<I: Span>(input: I) -> Res<I, SpecificSelector> {
    tuple((
        pattern(domain),
        tag(":"),
        pattern(domain),
        tag(":"),
        pattern(skewer_case),
        tag(":"),
        pattern(skewer_case),
        tag(":"),
        delimited(tag("("), version_req, tag(")")),
    ))(input)
    .map(
        |(next, (provider, _, vendor, _, product, _, variant, _, version))| {
            let specific = SpecificSelector {
                provider,
                vendor,
                product,
                variant,
                version,
            };
            (next, specific)
        },
    )
}

pub fn rec_domain_pattern<I: Span>(input: I) -> Res<I, Pattern<I>> {
    pattern(rec_domain)(input)
}

pub fn rec_skewer_pattern<I: Span>(input: I) -> Res<I, Pattern<I>> {
    pattern(skewer_chars)(input)
}

pub fn specific_version_req<I: Span>(input: I) -> Res<I, VersionReq> {
    delimited(tag("("), version_req, tag(")"))(input)
}

pub fn kind<I: Span>(input: I) -> Res<I, Kind> {
    let (next, lex) = kind_lex(input)?;

    resolve_kind(lex)(next)
}

pub fn rec_kind<I: Span>(input: I) -> Res<I, I> {
    recognize(kind_parts)(input)
}

pub fn kind_lex<I: Span>(input: I) -> Res<I, KindLex> {
    tuple((
        camel_case,
        opt(delimited(
            tag("<"),
            tuple((camel_case, opt(delimited(tag("<"), specific, tag(">"))))),
            tag(">"),
        )),
    ))(input)
    .map(|(next, (kind, rest))| {
        let mut rtn = KindLex {
            base: kind,
            sub: Option::None,
            specific: Option::None,
        };

        match rest {
            Some((sub, specific)) => {
                rtn.sub = Option::Some(sub);
                match specific {
                    Some(specific) => {
                        rtn.specific = Option::Some(specific);
                    }
                    None => {}
                }
            }
            None => {}
        }

        (next, rtn)
    })
}

pub fn kind_parts<I: Span>(input: I) -> Res<I, KindParts> {
    tuple((
        base_kind,
        opt(delimited(
            tag("<"),
            tuple((camel_case, opt(delimited(tag("<"), specific, tag(">"))))),
            tag(">"),
        )),
    ))(input)
    .map(|(next, (base, rest))| {
        let mut rtn = KindParts {
            base,
            sub: Option::None,
            specific: Option::None,
        };

        match rest {
            Some((sub, specific)) => {
                rtn.sub = Option::Some(sub);
                match specific {
                    Some(specific) => {
                        rtn.specific = Option::Some(specific);
                    }
                    None => {}
                }
            }
            None => {}
        }

        (next, rtn)
    })
}

pub fn delim_kind<I: Span>(input: I) -> Res<I, Kind> {
    unwrap_block(BlockKind::Nested(NestedBlockKind::Angle), kind)(input)
}

pub fn delim_kind_lex<I: Span>(input: I) -> Res<I, KindLex> {
    delimited(tag("<"), kind_lex, tag(">"))(input)
}

pub fn delim_kind_parts<I: Span>(input: I) -> Res<I, KindParts> {
    delimited(tag("<"), kind_parts, tag(">"))(input)
}

pub fn consume_kind<I: Span>(input: I) -> Result<KindParts, ParseErrs> {
    let (_, kind_parts) = all_consuming(kind_parts)(input)?;

    Ok(kind_parts.try_into()?)
}

pub fn to_string<I: Span, F>(mut f: F) -> impl FnMut(I) -> Res<I, String>
where
    F: FnMut(I) -> Res<I, I> + Copy,
{
    move |input: I| {
        f.parse(input)
            .map(|(next, output)| (next, output.to_string()))
    }
}

pub fn sub_kind_selector<I: Span>(input: I) -> Res<I, SubKindSelector> {
    pattern(camel_case)(input).map(|(next, selector)| match selector {
        Pattern::Always => (next, SubKindSelector::Always),
        Pattern::Exact(sub) => (next, SubKindSelector::Exact(sub)),
    })
}

pub fn base_kind<I: Span>(input: I) -> Res<I, BaseKind> {
    let (next, kind) = context("kind-base", camel_case)(input.clone())?;

    match BaseKind::try_from(kind.clone()) {
        Ok(kind) => Ok((next, kind)),
        Err(err) => {
            let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
            Err(nom::Err::Error(NomErr::add_context(
                input,
                ErrCtx::InvalidBaseKind(kind.to_string()),
                err,
            )))
        }
    }
}

pub fn resolve_kind<I: Span>(lex: KindLex) -> impl FnMut(I) -> Res<I, Kind> {
    move |input: I| {
        let s = new_span(lex.base.as_str());

        let input2 = input.clone();
        let base = match BaseKind::try_from(lex.base.clone()) {
            Ok(base) => base,
            Err(err) => {
                let err = NomErr::from_error_kind(input2.clone(), ErrorKind::Fail);
                Err(nom::Err::Error(NomErr::add_context(
                    input2,
                    ErrCtx::InvalidBaseKind(lex.base.to_string()),
                    err,
                )))?
            }
        };

        match base {
            BaseKind::Root => Ok((input, Kind::Root)),
            BaseKind::Space => Ok((input, Kind::Space)),
            BaseKind::Base => Ok((input, Kind::Base)),
            BaseKind::User => Ok((input, Kind::User)),
            BaseKind::App => Ok((input, Kind::App)),
            BaseKind::Mechtron => Ok((input, Kind::Mechtron)),
            BaseKind::FileStore => Ok((input, Kind::FileStore)),
            BaseKind::BundleSeries => Ok((input, Kind::BundleSeries)),
            BaseKind::Bundle => Ok((input, Kind::Bundle)),
            BaseKind::Control => Ok((input, Kind::Control)),
            BaseKind::Portal => Ok((input, Kind::Portal)),
            BaseKind::Repo => Ok((input, Kind::Repo)),
            BaseKind::Driver => Ok((input, Kind::Driver)),
            BaseKind::Global => Ok((input, Kind::Global)),
            BaseKind::Host => Ok((input, Kind::Host)),
            BaseKind::Guest => Ok((input, Kind::Guest)),
            _ => {
                if lex.sub.is_none() {
                    let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                    Err(nom::Err::Error(NomErr::add_context(
                        input.clone(),
                        ErrCtx::InvalidSubKind(BaseKind::Database, "none".to_string()),
                        err,
                    )))?;
                }
                let sub = lex.sub.as_ref().unwrap().clone();
                match base {
                    BaseKind::Database => match sub.as_str() {
                        "Relational" => match lex.specific.as_ref() {
                            Some(specific) => Ok((
                                input,
                                Kind::Database(DatabaseSubKind::Relational(specific.clone())),
                            )),
                            None => {
                                let err =
                                    NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                                Err(nom::Err::Error(NomErr::add_context(
                                    input,
                                    ErrCtx::InvalidSubKind(BaseKind::Database, sub.to_string()),
                                    err,
                                )))
                            }
                        },
                        _ => {
                            let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                            Err(nom::Err::Error(NomErr::add_context(
                                input,
                                ErrCtx::InvalidSubKind(BaseKind::Database, sub.to_string()),
                                err,
                            )))
                        }
                    },
                    BaseKind::UserBase => match sub.as_str() {
                        "OAuth" => match lex.specific.as_ref() {
                            Some(specific) => {
                                return Ok((
                                    input,
                                    Kind::UserBase(UserBaseSubKind::OAuth(specific.clone())),
                                ));
                            }
                            None => {
                                let err =
                                    NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                                Err(nom::Err::Error(NomErr::add_context(
                                    input,
                                    ErrCtx::InvalidSubKind(BaseKind::UserBase, sub.to_string()),
                                    err,
                                )))?
                            }
                        },
                        _ => {
                            let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                            Err(nom::Err::Error(NomErr::add_context(
                                input,
                                ErrCtx::InvalidSubKind(BaseKind::UserBase, sub.to_string()),
                                err,
                            )))
                        }
                    },

                    BaseKind::Artifact => match ArtifactSubKind::from_str(sub.as_str()) {
                        Ok(sub) => Ok((input, Kind::Artifact(sub))),
                        Err(err) => {
                            let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                            Err(nom::Err::Error(NomErr::add_context(
                                input,
                                ErrCtx::InvalidSubKind(BaseKind::Artifact, sub.to_string()),
                                err,
                            )))
                        }
                    },
                    BaseKind::Star => match StarSub::from_str(sub.as_str()) {
                        Ok(sub) => Ok((input, Kind::Star(sub))),
                        Err(err) => {
                            let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                            Err(nom::Err::Error(NomErr::add_context(
                                input,
                                ErrCtx::InvalidSubKind(BaseKind::Star, sub.to_string()),
                                err,
                            )))
                        }
                    },
                    BaseKind::File => match FileSubKind::from_str(sub.as_str()) {
                        Ok(sub) => Ok((input, Kind::File(sub))),
                        Err(err) => {
                            let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                            Err(nom::Err::Error(NomErr::add_context(
                                input,
                                ErrCtx::InvalidSubKind(BaseKind::File, sub.to_string()),
                                err,
                            )))
                        }
                    },

                    _ => {
                        let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                        Err(nom::Err::Error(NomErr::add_context(
                            input,
                            ErrCtx::InvalidSubKind(BaseKind::File, sub.to_string()),
                            err,
                        )))
                    }
                }
            }
        }
    }
}

pub fn resolve_sub<I: Span>(base: BaseKind) -> impl FnMut(I) -> Res<I, Sub> {
    move |input: I| {
        let (next, sub) = context("kind-sub", camel_case)(input.clone())?;
        match &base {
            BaseKind::Database => match sub.as_str() {
                "Relational" => {
                    let (next, specific) =
                        context("specific", delimited(tag("<"), specific, tag(">")))(next)?;
                    Ok((next, Sub::Database(DatabaseSubKind::Relational(specific))))
                }
                _ => {
                    let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                    Err(nom::Err::Error(NomErr::add_context(
                        input,
                        ErrCtx::InvalidSubKind(BaseKind::Database, sub.to_string()),
                        err,
                    )))
                }
            },
            BaseKind::UserBase => match sub.as_str() {
                "OAuth" => {
                    let (next, specific) =
                        context("specific", delimited(tag("<"), specific, tag(">")))(next)?;
                    Ok((next, Sub::UserBase(UserBaseSubKind::OAuth(specific))))
                }
                _ => {
                    let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                    Err(nom::Err::Error(NomErr::add_context(
                        input,
                        ErrCtx::InvalidSubKind(BaseKind::UserBase, sub.to_string()),
                        err,
                    )))
                }
            },

            BaseKind::Artifact => match ArtifactSubKind::from_str(sub.as_str()) {
                Ok(sub) => Ok((next, Sub::Artifact(sub))),
                Err(err) => {
                    let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                    Err(nom::Err::Error(NomErr::add_context(
                        input,
                        ErrCtx::InvalidSubKind(BaseKind::Artifact, sub.to_string()),
                        err,
                    )))
                }
            },
            BaseKind::Star => match StarSub::from_str(sub.as_str()) {
                Ok(sub) => Ok((next, Sub::Star(sub))),
                Err(err) => {
                    let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                    Err(nom::Err::Error(NomErr::add_context(
                        input,
                        ErrCtx::InvalidSubKind(BaseKind::Star, sub.to_string()),
                        err,
                    )))
                }
            },
            BaseKind::File => match FileSubKind::from_str(sub.as_str()) {
                Ok(sub) => Ok((next, Sub::File(sub))),
                Err(err) => {
                    let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                    Err(nom::Err::Error(NomErr::add_context(
                        input,
                        ErrCtx::InvalidSubKind(BaseKind::File, sub.to_string()),
                        err,
                    )))
                }
            },
            k => {
                let kind = k.to_string();
                let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                Err(nom::Err::Error(NomErr::add_context(
                    input,
                    ErrCtx::InvalidSubKind(
                        k.clone(),
                        format!("Kind: `{}` does not have any associated SubKind", kind)
                            .to_string(),
                    ),
                    err,
                )))
            }
        }
    }
}

pub fn kind_base_selector<I: Span>(input: I) -> Res<I, KindBaseSelector> {
    value_pattern(base_kind)(input).map(|(next, pattern)| {
        (
            next,
            match pattern {
                ValuePattern::Always => KindBaseSelector::Always,
                ValuePattern::Never => KindBaseSelector::Never,
                ValuePattern::Pattern(kind) => KindBaseSelector::Exact(kind),
            },
        )
    })
}

pub fn kind_selector<I: Span>(input: I) -> Res<I, KindSelector> {
    delimited(
        tag("<"),
        tuple((
            kind_base_selector,
            opt(delimited(
                tag("<"),
                tuple((
                    sub_kind_selector,
                    opt(delimited(
                        tag("<"),
                        value_pattern(specific_selector),
                        tag(">"),
                    )),
                )),
                tag(">"),
            )),
        )),
        tag(">"),
    )(input)
    .map(|(next, (kind, sub_kind_and_specific))| {
        let (sub, specific): (SubKindSelector, ValuePattern<SpecificSelector>) =
            match sub_kind_and_specific {
                None => (SubKindSelector::Always, ValuePattern::Always),
                Some((sub, specific)) => {
                    let specific = match specific {
                        None => ValuePattern::Always,
                        Some(s) => s,
                    };

                    (sub, specific)
                }
            };

        let tks = KindSelector {
            base: kind,
            sub,
            specific,
        };

        (next, tks)
    })
}

fn space_hop<I: Span>(input: I) -> Res<I, PointSegKindHop> {
    tuple((point_segment_selector, opt(kind_selector), opt(tag("+"))))(input).map(
        |(next, (segment_selector, kind_selector, inclusive))| {
            let kind_selector = match kind_selector {
                None => KindSelector::any(),
                Some(kind_selector) => kind_selector,
            };
            let inclusive = inclusive.is_some();
            (
                next,
                PointSegKindHop {
                    inclusive,
                    segment_selector,
                    kind_selector,
                },
            )
        },
    )
}

fn base_hop<I: Span>(input: I) -> Res<I, PointSegKindHop> {
    tuple((base_segment, opt(kind_selector), opt(tag("+"))))(input).map(
        |(next, (segment, tks, inclusive))| {
            let tks = match tks {
                None => KindSelector::any(),
                Some(tks) => tks,
            };
            let inclusive = inclusive.is_some();
            (
                next,
                PointSegKindHop {
                    inclusive,
                    segment_selector: segment,
                    kind_selector: tks,
                },
            )
        },
    )
}

/*
fn file_hop<I: Span>(input: I) -> Res<I, PointSegKindHop> {
    tuple((file_segment, opt(tag("+"))))(input).map(|(next, (segment, inclusive))| {
        let tks = KindSelector {
            base: Pattern::Exact(BaseKind::File),
            sub: Pattern::Always,
            specific: ValuePattern::Always,
        };
        let inclusive = inclusive.is_some();
        (
            next,
            PointSegKindHop {
                inclusive,
                segment_selector: segment,
                kind_selector: ValuePattern::Pattern(tks),
            },
        )
    })
}

 */

fn file_hop<I: Span>(input: I) -> Res<I, PointSegKindHop> {
    tuple((file_segment, opt(tag("+"))))(input).map(|(next, (segment, inclusive))| {
        let tks = KindSelector {
            base: KindBaseSelector::Exact(BaseKind::File),
            sub: SubKindSelector::None,
            specific: ValuePattern::Always,
        };
        let inclusive = inclusive.is_some();
        (
            next,
            PointSegKindHop {
                inclusive,
                segment_selector: segment,
                kind_selector: tks,
            },
        )
    })
}

fn dir_hop<I: Span>(input: I) -> Res<I, PointSegKindHop> {
    tuple((dir_segment, opt(tag("+"))))(input).map(|(next, (segment, inclusive))| {
        let tks = KindSelector::any();
        let inclusive = inclusive.is_some();
        (
            next,
            PointSegKindHop {
                inclusive,
                segment_selector: segment,
                kind_selector: tks,
            },
        )
    })
}

fn version_hop<I: Span>(input: I) -> Res<I, PointSegKindHop> {
    tuple((version_segment, opt(kind_selector), opt(tag("+"))))(input).map(
        |(next, (segment, tks, inclusive))| {
            let tks = match tks {
                None => KindSelector::any(),
                Some(tks) => tks,
            };
            let inclusive = inclusive.is_some();
            (
                next,
                PointSegKindHop {
                    inclusive,
                    segment_selector: segment,
                    kind_selector: tks,
                },
            )
        },
    )
}

pub fn point_selector<I: Span>(input: I) -> Res<I, Selector> {
    context(
        "point_kind_pattern",
        tuple((
            space_hop,
            many0(preceded(tag(":"), base_hop)),
            opt(preceded(tag(":"), version_hop)),
            opt(preceded(tag(":/"), tuple((many0(dir_hop), opt(file_hop))))),
        )),
    )(input)
    .map(
        |(next, (space_hop, base_hops, version_hop, filesystem_hops))| {
            let mut hops = vec![];
            hops.push(space_hop);
            for base_hop in base_hops {
                hops.push(base_hop);
            }
            if let Option::Some(version_hop) = version_hop {
                hops.push(version_hop);
            }
            if let Some((dir_hops, file_hop)) = filesystem_hops {
                // first push the filesystem root
                hops.push(PointSegKindHop {
                    inclusive: false,
                    segment_selector: PointSegSelector::Exact(ExactPointSeg::PointSeg(
                        PointSeg::FsRootDir,
                    )),
                    kind_selector: KindSelector {
                        base: KindBaseSelector::Exact(BaseKind::File),
                        sub: SubKindSelector::Always,
                        specific: ValuePattern::Always,
                    },
                });
                for dir_hop in dir_hops {
                    hops.push(dir_hop);
                }
                if let Some(file_hop) = file_hop {
                    hops.push(file_hop);
                }
            }

            let rtn = Selector {
                hops,
                always: false,
            };

            (next, rtn)
        },
    )
}

pub fn point_and_kind<I: Span>(input: I) -> Res<I, PointKindVar> {
    tuple((point_var, kind))(input)
        .map(|(next, (point, kind))| (next, PointKindVar { point, kind }))
}

pub fn version<I: Span>(input: I) -> Res<I, Version> {
    let (next, version) = rec_version(input.clone())?;
    let version = version.to_string();
    let str_input = version.as_str();
    let rtn = semver::Version::parse(str_input);

    match rtn {
        Ok(version) => Ok((next, Version { version })),
        Err(err) => {
            let tree = Err::Error(NomErr::from_error_kind(input, ErrorKind::Fail));
            Err(tree)
        }
    }
}

pub fn specific<I>(input: I) -> Res<I, Specific> where I: Span {
    tuple((
        domain,
        tag(":"),
        domain,
        tag(":"),
        skewer_case,
        tag(":"),
        skewer_case,
        tag(":"),
        version,
    ))(input)
    .map(
        |(next, (provider, _, vendor, _, product, _, variant, _, version))| {
            let specific = Specific {
                provider,
                vendor,
                product,
                variant,
                version,
            };
            (next, specific)
        },
    )
}

pub fn args<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '"')
                && !(char_item == '_')
                && !(char_item == '{')
                && !(char_item == '}')
                && !(char_item == '(')
                && !(char_item == ')')
                && !(char_item == '[')
                && !(char_item == ']')
                && !(char_item == ' ')
                && !(char_item == '\n')
                && !(char_item == '\t')
                && !(char_item == '\r')
                && !(char_item == '\'')
                && !((char_item.is_alphanumeric()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn skewer<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn skewer_or_snake<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item == '_')
                && !((char_item.is_alpha() && char_item.is_lowercase()) || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn not_quote<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            (char_item == '"')
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn filename<T>(i: T) -> Res<T, T>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-') && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}

pub fn primitive_def<I: Span>(input: I) -> Res<I, PayloadType2Def<PointVar>> {
    tuple((
        payload,
        opt(preceded(tag("~"), opt(format))),
        opt(preceded(tag("~"), call_with_config)),
    ))(input)
    .map(|(next, (primitive, format, verifier))| {
        (
            next,
            PayloadType2Def {
                primitive,
                format: match format {
                    Some(Some(format)) => Some(format),
                    _ => Option::None,
                },
                verifier,
            },
        )
    })
}

pub fn payload<I: Span>(input: I) -> Res<I, SubstanceKind> {
    parse_camel_case_str(input)
}

pub fn consume_primitive_def<I: Span>(input: I) -> Res<I, PayloadType2Def<PointVar>> {
    all_consuming(primitive_def)(input)
}

pub fn call_with_config<I: Span>(input: I) -> Res<I, CallWithConfigVar> {
    tuple((call, opt(preceded(tag("+"), point_var))))(input)
        .map(|(next, (call, config))| (next, CallWithConfigVar { call, config }))
}

pub fn parse_alpha1_str<I: Span, O: FromStr>(input: I) -> Res<I, O> {
    let (next, rtn) = recognize(alpha1)(input)?;
    match O::from_str(rtn.to_string().as_str()) {
        Ok(rtn) => Ok((next, rtn)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            next,
            ErrorKind::Fail,
        ))),
    }
}

pub fn rc_command<I: Span>(input: I) -> Res<I, CmdKind> {
    parse_alpha1_str(input)
}

pub fn ext_call<I: Span>(input: I) -> Res<I, CallKind> {
    tuple((
        delimited(tag("Ext<"), ext_method, tag(">")),
        opt(subst_path),
    ))(input)
    .map(|(next, (method, path))| {
        let path = match path {
            None => subst(filepath_chars)(new_span("/")).unwrap().1.stringify(),
            Some(path) => path.stringify(),
        };
        (next, CallKind::Ext(ExtCall::new(method, path)))
    })
}

pub fn http_call<I: Span>(input: I) -> Res<I, CallKind> {
    tuple((
        delimited(tag("Http<"), http_method, tag(">")),
        opt(subst_path),
    ))(input)
    .map(|(next, (method, path))| {
        let path = match path {
            None => subst(filepath_chars)(new_span("/")).unwrap().1.stringify(),
            Some(path) => path.stringify(),
        };
        (next, CallKind::Http(HttpCall::new(method, path)))
    })
}

pub fn call_kind<I: Span>(input: I) -> Res<I, CallKind> {
    alt((ext_call, http_call))(input)
}

pub fn call<I: Span>(input: I) -> Res<I, CallVar> {
    tuple((point_var, preceded(tag("^"), call_kind)))(input)
        .map(|(next, (point, kind))| (next, CallVar { point, kind }))
}

pub fn consume_call<I: Span>(input: I) -> Res<I, CallVar> {
    all_consuming(call)(input)
}

pub fn labeled_primitive_def<I: Span>(input: I) -> Res<I, LabeledPrimitiveTypeDef<PointVar>> {
    tuple((skewer, delimited(tag("<"), primitive_def, tag(">"))))(input).map(
        |(next, (label, primitive_def))| {
            let labeled_def = LabeledPrimitiveTypeDef {
                label: label.to_string(),
                def: primitive_def,
            };
            (next, labeled_def)
        },
    )
}

pub fn digit_range<I: Span>(input: I) -> Res<I, NumRange> {
    tuple((digit1, tag("-"), digit1))(input).map(|(next, (min, _, max))| {
        let min: usize = usize::from_str(min.to_string().as_str()).expect("usize");
        let max: usize = usize::from_str(max.to_string().as_str()).expect("usize");
        let range = NumRange::MinMax { min, max };

        (next, range)
    })
}

pub fn exact_range<I: Span>(input: I) -> Res<I, NumRange> {
    digit1(input).map(|(next, exact)| {
        (
            next,
            NumRange::Exact(
                usize::from_str(exact.to_string().as_str())
                    .expect("expect to be able to change digit string into usize"),
            ),
        )
    })
}

pub fn range<I: Span>(input: I) -> Res<I, NumRange> {
    delimited(
        multispace0,
        opt(alt((digit_range, exact_range))),
        multispace0,
    )(input)
    .map(|(next, range)| {
        let range = match range {
            Some(range) => range,
            None => NumRange::Any,
        };
        (next, range)
    })
}

pub fn primitive_data_struct<I: Span>(input: I) -> Res<I, SubstanceTypePatternDef<PointVar>> {
    context("selector", payload)(input)
        .map(|(next, primitive)| (next, SubstanceTypePatternDef::Primitive(primitive)))
}

pub fn array_data_struct<I: Span>(input: I) -> Res<I, SubstanceTypePatternDef<PointVar>> {
    context(
        "selector",
        tuple((
            payload,
            context("array", delimited(tag("["), range, tag("]"))),
        )),
    )(input)
    .map(|(next, (primitive, range))| {
        (
            next,
            SubstanceTypePatternDef::List(ListPattern { primitive, range }),
        )
    })
}

pub fn map_entry_pattern_any<I: Span>(input: I) -> Res<I, ValuePattern<MapEntryPatternVar>> {
    delimited(multispace0, tag("*"), multispace0)(input)
        .map(|(next, _)| (next, ValuePattern::Always))
}

pub fn map_entry_pattern<I: Span>(input: I) -> Res<I, MapEntryPatternVar> {
    tuple((skewer, opt(delimited(tag("<"), payload_pattern, tag(">")))))(input).map(
        |(next, (key_con, payload_con))| {
            let payload_con = match payload_con {
                None => ValuePattern::Always,
                Some(payload_con) => payload_con,
            };

            let map_entry_con = MapEntryPatternVar {
                key: key_con.to_string(),
                payload: payload_con,
            };
            (next, map_entry_con)
        },
    )
}

pub fn map_entry_patterns<I: Span>(input: I) -> Res<I, Vec<MapEntryPatternVar>> {
    separated_list0(
        delimited(multispace0, tag(","), multispace0),
        map_entry_pattern,
    )(input)
}

pub fn consume_map_entry_pattern<I: Span>(input: I) -> Res<I, MapEntryPatternVar> {
    all_consuming(map_entry_pattern)(input)
}

pub fn required_map_entry_pattern<I: Span>(input: I) -> Res<I, Vec<MapEntryPatternVar>> {
    delimited(tag("["), map_entry_patterns, tag("]"))(input).map(|(next, params)| (next, params))
}

pub fn allowed_map_entry_pattern<I: Span>(input: I) -> Res<I, ValuePattern<SubstancePatternVar>> {
    payload_pattern(input).map(|(next, con)| (next, con))
}

//  [ required1<Bin>, required2<Text> ] *<Bin>
pub fn map_pattern_params<I: Span>(input: I) -> Res<I, MapPatternVar> {
    tuple((
        opt(map_entry_patterns),
        multispace0,
        opt(allowed_map_entry_pattern),
    ))(input)
    .map(|(next, (required, _, allowed))| {
        let mut required_map = HashMap::new();
        match required {
            Option::Some(required) => {
                for require in required {
                    required_map.insert(require.key, require.payload);
                }
            }
            Option::None => {}
        }

        let allowed = match allowed {
            Some(allowed) => allowed,
            None => ValuePattern::Never,
        };

        let con = MapPatternVar::new(required_map, allowed);

        (next, con)
    })
}

pub fn format<I: Span>(input: I) -> Res<I, SubstanceFormat> {
    let (next, format) = recognize(alpha1)(input)?;
    match SubstanceFormat::from_str(format.to_string().as_str()) {
        Ok(format) => Ok((next, format)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            next,
            ErrorKind::Fail,
        ))),
    }
}

enum MapConParam {
    Required(Vec<ValuePattern<MapEntryPattern>>),
    Allowed(ValuePattern<SubstancePattern>),
}

// EXAMPLE:
//  Map { [ required1<Bin>, required2<Text> ] *<Bin> }
pub fn map_pattern<I: Span>(input: I) -> Res<I, MapPatternVar> {
    tuple((
        delimited(multispace0, tag("Map"), multispace0),
        opt(delimited(
            tag("{"),
            delimited(multispace0, map_pattern_params, multispace0),
            tag("}"),
        )),
    ))(input)
    .map(|(next, (_, entries))| {
        let mut entries = entries;
        let con = match entries {
            None => MapPatternVar::any(),
            Some(con) => con,
        };

        (next, con)
    })
}

pub fn value_constrained_map_pattern<I: Span>(input: I) -> Res<I, ValuePattern<MapPatternVar>> {
    value_pattern(map_pattern)(input)
}

pub fn ext_action<I: Span>(input: I) -> Res<I, ValuePattern<StringMatcher>> {
    value_pattern(camel_case_to_string_matcher)(input)
}

pub fn parse_camel_case_str<I: Span, O: FromStr>(input: I) -> Res<I, O> {
    let (next, rtn) = recognize(camel_case_chars)(input)?;
    match O::from_str(rtn.to_string().as_str()) {
        Ok(rtn) => Ok((next, rtn)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            next,
            ErrorKind::Fail,
        ))),
    }
}

pub fn http_method<I: Span>(input: I) -> Res<I, HttpMethod> {
    context("http_method", parse_camel_case_str).parse(input)
}

pub fn http_method_pattern<I: Span>(input: I) -> Res<I, HttpMethodPattern> {
    context("@http_method_pattern", method_pattern(http_method))(input)
}

pub fn method_pattern<I: Clone, F>(mut f: F) -> impl FnMut(I) -> Res<I, HttpMethodPattern>
where
    I: InputLength + InputTake + Compare<&'static str>,
    F: Parser<I, HttpMethod, NomErr<I>>,
{
    move |input: I| match tag::<&'static str, I, NomErr<I>>("*")(input.clone()) {
        Ok((next, _)) => Ok((next, HttpMethodPattern::Always)),
        Err(err) => f
            .parse(input.clone())
            .map(|(next, res)| (next, HttpMethodPattern::Pattern(res))),
    }
}

pub fn ext_method<I: Span>(input: I) -> Res<I, ExtMethod> {
    let (next, ext_method) = camel_case_chars(input.clone())?;

    match ExtMethod::new(ext_method.to_string()) {
        Ok(method) => Ok((next, method)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            input,
            ErrorKind::Fail,
        ))),
    }
}

pub fn sys_method<I: Span>(input: I) -> Res<I, HypMethod> {
    let (next, sys_method) = camel_case_chars(input.clone())?;

    match HypMethod::from_str(sys_method.to_string().as_str()) {
        Ok(method) => Ok((next, method)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            input,
            ErrorKind::Fail,
        ))),
    }
}

pub fn cmd_method<I: Span>(input: I) -> Res<I, CmdMethod> {
    let (next, method) = camel_case_chars(input.clone())?;

    match CmdMethod::from_str(method.to_string().as_str()) {
        Ok(method) => Ok((next, method)),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            input,
            ErrorKind::Fail,
        ))),
    }
}

pub fn wrapped_ext_method<I: Span>(input: I) -> Res<I, Method> {
    let (next, ext_method) = ext_method(input.clone())?;

    match ExtMethod::new(ext_method.to_string()) {
        Ok(method) => Ok((next, Method::Ext(method))),
        Err(err) => Err(nom::Err::Error(NomErr::from_error_kind(
            input,
            ErrorKind::Fail,
        ))),
    }
}

pub fn wrapped_http_method<I: Span>(input: I) -> Res<I, Method> {
    http_method(input).map(|(next, method)| (next, Method::Http(method)))
}

pub fn wrapped_sys_method<I: Span>(input: I) -> Res<I, Method> {
    sys_method(input).map(|(next, method)| (next, Method::Hyp(method)))
}

pub fn wrapped_cmd_method<I: Span>(input: I) -> Res<I, Method> {
    cmd_method(input).map(|(next, method)| (next, Method::Cmd(method)))
}

pub fn rc_command_type<I: Span>(input: I) -> Res<I, CmdKind> {
    parse_alpha1_str(input)
}

pub fn map_pattern_payload_structure<I: Span>(
    input: I,
) -> Res<I, SubstanceTypePatternDef<PointVar>> {
    map_pattern(input).map(|(next, con)| (next, SubstanceTypePatternDef::Map(Box::new(con))))
}

pub fn payload_structure<I: Span>(input: I) -> Res<I, SubstanceTypePatternDef<PointVar>> {
    alt((
        array_data_struct,
        primitive_data_struct,
        map_pattern_payload_structure,
    ))(input)
}

pub fn payload_structure_with_validation<I: Span>(input: I) -> Res<I, SubstancePatternVar> {
    tuple((
        context("selector", payload_structure),
        opt(preceded(tag("~"), opt(format))),
        opt(preceded(tag("~"), call_with_config)),
    ))(input)
    .map(|(next, (data, format, verifier))| {
        (
            next,
            SubstancePatternVar {
                structure: data,
                format: match format {
                    Some(Some(format)) => Some(format),
                    _ => Option::None,
                },
                validator: verifier,
            },
        )
    })
}

pub fn consume_payload_structure<I: Span>(input: I) -> Res<I, SubstanceTypePatternVar> {
    all_consuming(payload_structure)(input)
}

pub fn consume_data_struct_def<I: Span>(input: I) -> Res<I, SubstancePatternVar> {
    all_consuming(payload_structure_with_validation)(input)
}

pub fn payload_pattern_any<I: Span>(input: I) -> Res<I, ValuePattern<SubstancePatternVar>> {
    tag("*")(input).map(|(next, _)| (next, ValuePattern::Always))
}

pub fn payload_pattern<I: Span>(input: I) -> Res<I, ValuePattern<SubstancePatternVar>> {
    context(
        "@payload-pattern",
        value_pattern(payload_structure_with_validation),
    )(input)
    .map(|(next, payload_pattern)| (next, payload_pattern))
}

pub fn payload_filter_block_empty<I: Span>(input: I) -> Res<I, PatternBlockVar> {
    multispace0(input.clone()).map(|(next, _)| (input, PatternBlockVar::Never))
}

pub fn payload_filter_block_any<I: Span>(input: I) -> Res<I, PatternBlockVar> {
    let (next, _) = delimited(multispace0, context("selector", tag("*")), multispace0)(input)?;

    Ok((next, PatternBlockVar::Always))
}

pub fn payload_filter_block_def<I: Span>(input: I) -> Res<I, PatternBlockVar> {
    payload_structure_with_validation(input)
        .map(|(next, pattern)| (next, PatternBlockVar::Pattern(pattern)))
}

fn insert_block_pattern<I: Span>(input: I) -> Res<I, UploadBlock> {
    delimited(multispace0, filename, multispace0)(input).map(|(next, filename)| {
        (
            next,
            UploadBlock {
                name: filename.to_string(),
            },
        )
    })
}

pub fn upload_payload_block<I: Span>(input: I) -> Res<I, UploadBlock> {
    delimited(multispace0, file_chars, multispace0)(input).map(|(next, filename)| {
        (
            next,
            UploadBlock {
                name: filename.to_string(),
            },
        )
    })
}

pub fn upload_block<I: Span>(input: I) -> Res<I, UploadBlock> {
    delimited(tag("^["), upload_payload_block, tag("]->"))(input)
}

pub fn upload_blocks<I: Span>(input: I) -> Res<I, Vec<UploadBlock>> {
    many0(pair(take_until("^["), upload_block))(input).map(|(next, blocks)| {
        let mut rtn = vec![];
        for (_, block) in blocks {
            rtn.push(block);
        }
        (next, rtn)
    })
}

pub fn request_payload_filter_block<I: Span>(input: I) -> Res<I, PayloadBlockVar> {
    tuple((
        multispace0,
        alt((
            payload_filter_block_any,
            payload_filter_block_def,
            payload_filter_block_empty,
        )),
        multispace0,
    ))(input)
    .map(|(next, (_, block, _))| (next, PayloadBlockVar::DirectPattern(block)))
}

pub fn response_payload_filter_block<I: Span>(input: I) -> Res<I, PayloadBlockVar> {
    context(
        "response-payload-filter-block",
        terminated(
            tuple((
                multispace0,
                alt((
                    payload_filter_block_any,
                    payload_filter_block_def,
                    payload_filter_block_empty,
                    fail,
                )),
                multispace0,
            )),
            tag("]"),
        ),
    )(input)
    .map(|(next, (_, block, _))| (next, PayloadBlockVar::ReflectPattern(block)))
}

pub fn rough_pipeline_step<I: Span>(input: I) -> Res<I, I> {
    recognize(tuple((
        many0(preceded(
            alt((tag("-"), tag("="), tag("+"))),
            any_surrounding_lex_block,
        )),
        alt((tag("->"), tag("=>"))),
    )))(input)
}

pub fn consume_pipeline_block<I: Span>(input: I) -> Res<I, PayloadBlockVar> {
    all_consuming(request_payload_filter_block)(input)
}

pub fn strip_comments<I: Span>(input: I) -> Res<I, String>
where
    I: InputTakeAtPosition + nom::InputLength + Clone + ToString,
    <I as InputTakeAtPosition>::Item: AsChar,
{
    many0(alt((no_comment, comment)))(input).map(|(next, texts)| {
        let mut rtn = String::new();
        for t in texts {
            match t {
                TextType::NoComment(span) => {
                    rtn.push_str(span.to_string().as_str());
                }
                TextType::Comment(span) => {
                    for i in 0..span.input_len() {
                        // replace with whitespace
                        rtn.push_str(" ");
                    }
                }
            }
        }

        // create with the new string, but use old string as reference
        //let span = LocatedSpan::new_extra(rtn.as_str(), input.extra.clone() );
        (next, rtn)
    })
}

pub fn no_comment<T: Span>(i: T) -> Res<T, TextType<T>>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            char_item == '#'
        },
        ErrorKind::AlphaNumeric,
    )
    .map(|(next, comment)| (next, TextType::NoComment(comment)))
}

pub fn comment<T: Span>(i: T) -> Res<T, TextType<T>>
where
    T: InputTakeAtPosition + nom::InputLength,
    <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            char_item == '\n'
        },
        ErrorKind::AlphaNumeric,
    )
    .map(|(next, comment)| (next, TextType::Comment(comment)))
}

pub fn bind_config(src: &str) -> Result<BindConfig, ParseErrs> {
    let document = doc(src)?;
    match document {
        Document::BindConfig(bind_config) => Ok(bind_config),
        _ => Err(ParseErrs::expected(
            "Document",
            DocKind::BindConfig.to_string(),
            document.kind().to_string(),
        )),
    }
}

pub fn mechtron_config(src: &str) -> Result<MechtronConfig, ParseErrs> {
    let document = doc(src)?;
    match document {
        Document::MechtronConfig(mechtron_config) => Ok(mechtron_config),
        _ => Err(ParseErrs::expected(
            "Document",
            &DocKind::MechtronConfig,
            &document.kind(),
        )),
    }
}

pub fn doc(src: &str) -> Result<Document, ParseErrs> {
    let src = src.to_string();
    let (next, stripped) = strip_comments(new_span(src.as_str()))?;
    let span = span_with_extra(stripped.as_str(), Arc::new(src.to_string()));
    let lex_root_scope = lex_root_scope(span.clone())?;
    let root_scope_selector = lex_root_scope.selector.clone().to_concrete()?;
    if root_scope_selector.name.as_str() == "Mechtron" {
        if root_scope_selector.version == Version::from_str("1.0.0")? {
            let mechtron = result(parse_mechtron_config(lex_root_scope.block.content.clone()))?;

            let mechtron = MechtronConfig::new(mechtron)?;
            return Ok(Document::MechtronConfig(mechtron));
        } else {
            let message = format!(
                "ConfigParser does not know how to process a Bind at version '{}'",
                root_scope_selector.version.to_string()
            );
            let mut builder = Report::build(ReportKind::Error, (), 0);
            let report = builder
                .with_message(message)
                .with_label(
                    Label::new(
                        lex_root_scope.selector.version.span.location_offset()
                            ..lex_root_scope.selector.version.span.location_offset()
                                + lex_root_scope.selector.version.span.len(),
                    )
                    .with_message("Unsupported Bind Config Version"),
                )
                .finish();
            Err(ParseErrs::from_report(report, lex_root_scope.block.content.extra.clone()).into())
        }
    } else if root_scope_selector.name.as_str() == "Bind" {
        if root_scope_selector.version == Version::from_str("1.0.0")? {
            let bind = parse_bind_config(lex_root_scope.block.content.clone())?;

            return Ok(Document::BindConfig(bind));
        } else {
            let message = format!(
                "ConfigParser does not know how to process a Bind at version '{}'",
                root_scope_selector.version.to_string()
            );
            let mut builder = Report::build(ReportKind::Error, (), 0);
            let report = builder
                .with_message(message)
                .with_label(
                    Label::new(
                        lex_root_scope.selector.version.span.location_offset()
                            ..lex_root_scope.selector.version.span.location_offset()
                                + lex_root_scope.selector.version.span.len(),
                    )
                    .with_message("Unsupported Bind Config Version"),
                )
                .finish();
            Err(ParseErrs::from_report(report, lex_root_scope.block.content.extra.clone()).into())
        }
    } else {
        let message = format!(
            "ConfigParser does not know how to process a '{}'",
            lex_root_scope.selector.name.to_string(),
        );
        let mut builder = Report::build(ReportKind::Error, (), 0);
        let report = builder
            .with_message(message)
            .with_label(
                Label::new(
                    lex_root_scope.selector.name.location_offset()
                        ..lex_root_scope.selector.name.location_offset()
                            + lex_root_scope.selector.name.len(),
                )
                .with_message("Unrecognized Config Kind"),
            )
            .finish();
        Err(ParseErrs::from_report(report, lex_root_scope.block.content.extra.clone()).into())
    }
}

fn parse_mechtron_config<I: Span>(input: I) -> Res<I, Vec<MechtronScope>> {
    let (next, (_, (_, (_, assignments)))) = pair(
        multispace0,
        context(
            "wasm",
            tuple((
                tag("Wasm"),
                alt((
                    tuple((
                        multispace0,
                        unwrap_block(BlockKind::Nested(NestedBlockKind::Curly), many0(assignment)),
                    )),
                    fail,
                )),
            )),
        ),
    )(input)?;
    Ok((next, vec![MechtronScope::WasmScope(assignments)]))
}

fn assignment<I>(input: I) -> Res<I, Assignment>
where
    I: Span,
{
    tuple((
        multispace0,
        context("assignment:plus", alt((tag("+"), fail))),
        context("assignment:key", alt((skewer, fail))),
        multispace0,
        context("assignment:equals", alt((tag("="), fail))),
        multispace0,
        context("assignment:value", alt((nospace1_nosemi, fail))),
        multispace0,
        opt(tag(";")),
        multispace0,
    ))(input)
    .map(|(next, (_, _, k, _, _, _, v, _, _, _))| {
        (
            next,
            Assignment {
                key: k.to_string(),
                value: v.to_string(),
            },
        )
    })
}

#[derive(Clone)]
pub struct Assignment {
    pub key: String,
    pub value: String,
}

fn semantic_mechtron_scope<I: Span>(scope: LexScope<I>) -> Result<MechtronScope, ParseErrs> {
    let selector_name = scope.selector.name.to_string();
    match selector_name.as_str() {
        "Wasm" => {
            let assignments = result(many0(assignment)(scope.block.content))?;
            Ok(MechtronScope::WasmScope(assignments))
        }
        what => {
            let mut builder = Report::build(ReportKind::Error, (), 0);
            let report = builder
                .with_message(format!(
                    "Unrecognized MechtronConfig selector: '{}'",
                    scope.selector.name.to_string()
                ))
                .with_label(
                    Label::new(
                        scope.selector.name.location_offset()
                            ..scope.selector.name.location_offset() + scope.selector.name.len(),
                    )
                    .with_message("Unrecognized Selector"),
                )
                .finish();
            Err(ParseErrs::from_report(report, scope.block.content.extra().clone()).into())
        }
    }
}

fn parse_bind_config<I: Span>(input: I) -> Result<BindConfig, ParseErrs> {
    let lex_scopes = lex_scopes(input)?;
    let mut scopes = vec![];
    let mut errors = vec![];

    for lex_scope in lex_scopes {
        match semantic_bind_scope(lex_scope) {
            Ok(scope) => {
                scopes.push(scope);
            }
            Err(err) => errors.push(err),
        }
    }

    if !errors.is_empty() {
        let errors = ParseErrs::fold(errors);
        return Err(errors.into());
    }

    let mut config = BindConfig::new(scopes);
    Ok(config)
}

fn semantic_bind_scope<I: Span>(scope: LexScope<I>) -> Result<BindScope, ParseErrs> {
    let selector_name = scope.selector.name.to_string();
    match selector_name.as_str() {
        "Route" => {
            let scope = lex_child_scopes(scope)?;
            let scope = RouteScope::try_from(scope)?;
            Ok(BindScope::RequestScope(scope))
        }
        what => {
            let mut builder = Report::build(ReportKind::Error, (), 0);
            let report = builder
                .with_message(format!(
                    "Unrecognized BindConfig selector: '{}'",
                    scope.selector.name.to_string()
                ))
                .with_label(
                    Label::new(
                        scope.selector.name.location_offset()
                            ..scope.selector.name.location_offset() + scope.selector.name.len(),
                    )
                    .with_message("Unrecognized Selector"),
                )
                .finish();
            Err(ParseErrs::from_report(report, scope.block.content.extra().clone()).into())
        }
    }
}

fn parse_bind_pipelines_scope<I: Span>(input: I) -> Result<Spanned<I, BindScopeKind>, ParseErrs> {
    unimplemented!()
    /*
    let (next, lex_scopes) = lex_scopes(input.clone())?;
    let mut errs = vec![];
    for lex_scope in lex_scopes {
        match lex_scope.selector.name.to_string().as_str() {
            "Ext" => {}
            "Http" => {}
            "Rc" => {}
            what => {
                let mut builder = Report::build(ReportKind::Error, (), 0);
                let report = builder
                    .with_message(format!("Unrecognized Pipeline scope: '{}'", what))
                    .with_label(
                        Label::new(input.location_offset()..input.location_offset())
                            .with_message("Unrecognized Selector"),
                    )
                    .finish();
                errs.push(ParseErrs::new(report, input.extra.clone()));
            }
        }
    }

    if !errs.is_empty() {
        Err(ParseErrs::fold(errs))
    } else {
        Ok(ElemSpan::new(BindBlock::Pipelines, input.clone()))
    }

     */
}

pub fn nospace0<I: Span>(input: I) -> Res<I, I> {
    recognize(many0(satisfy(|c| !c.is_whitespace())))(input)
}

pub fn nospace1<I: Span>(input: I) -> Res<I, I> {
    recognize(pair(
        satisfy(|c| !c.is_whitespace()),
        many0(satisfy(|c| !c.is_whitespace())),
    ))(input)
}

pub fn nospace1_nosemi<I: Span>(input: I) -> Res<I, I> {
    recognize(pair(
        satisfy(|c| !c.is_whitespace() && ';' != c),
        many0(satisfy(|c| !c.is_whitespace() && ';' != c)),
    ))(input)
}

pub fn no_space_with_blocks<I: Span>(input: I) -> Res<I, I> {
    recognize(many1(alt((recognize(any_block), nospace1))))(input)
}

pub fn pipeline_step_var<I: Span>(input: I) -> Res<I, PipelineStepVar> {
    context(
        "pipeline:step",
        tuple((
            alt((
                value(WaveDirection::Direct, tag("-")),
                value(WaveDirection::Reflect, tag("=")),
            )),
            opt(pair(
                delimited(
                    tag("["),
                    context("pipeline:step:exit", cut(request_payload_filter_block)),
                    tag("]"),
                ),
                context(
                    "pipeline:step:payload",
                    cut(alt((
                        value(WaveDirection::Direct, tag("-")),
                        value(WaveDirection::Reflect, tag("=")),
                    ))),
                ),
            )),
            context("pipeline:step:exit", cut(tag(">"))),
        )),
    )(input)
    .map(|(next, (entry, block_and_exit, _))| {
        let mut blocks = vec![];
        let exit = match block_and_exit {
            None => entry.clone(),
            Some((block, exit)) => {
                blocks.push(block);
                exit
            }
        };

        (
            next,
            PipelineStepVar {
                entry,
                exit,
                blocks,
            },
        )
    })
}

pub fn core_pipeline_stop<I: Span>(input: I) -> Res<I, PipelineStopVar> {
    context(
        "Core",
        delimited(
            tag("(("),
            delimited(multispace0, opt(tag("*")), multispace0),
            tag("))"),
        ),
    )(input)
    .map(|(next, _)| (next, PipelineStopVar::Core))
}

pub fn return_pipeline_stop<I: Span>(input: I) -> Res<I, PipelineStopVar> {
    tag("&")(input).map(|(next, _)| (next, PipelineStopVar::Reflect))
}

pub fn call_pipeline_stop<I: Span>(input: I) -> Res<I, PipelineStopVar> {
    context("Call", call)(input).map(|(next, call)| (next, PipelineStopVar::Call(call)))
}

pub fn point_pipeline_stop<I: Span>(input: I) -> Res<I, PipelineStopVar> {
    context("pipeline:stop:point", point_var)(input)
        .map(|(next, point)| (next, PipelineStopVar::Point(point)))
}

pub fn pipeline_stop_var<I: Span>(input: I) -> Res<I, PipelineStopVar> {
    context(
        "Stop",
        pair(
            context(
                "pipeline:stop:expecting",
                cut(peek(alt((tag("(("), tag("."), alpha1, tag("&"))))),
            ),
            alt((
                core_pipeline_stop,
                return_pipeline_stop,
                call_pipeline_stop,
                point_pipeline_stop,
            )),
        ),
    )(input)
    .map(|(next, (_, pipeline_stop))| (next, pipeline_stop))
}

pub fn consume_pipeline_step<I: Span>(input: I) -> Res<I, PipelineStepVar> {
    all_consuming(pipeline_step_var)(input)
}

pub fn consume_pipeline_stop<I: Span>(input: I) -> Res<I, PipelineStopVar> {
    all_consuming(pipeline_stop_var)(input)
}

pub fn pipeline_segment<I: Span>(input: I) -> Res<I, PipelineSegmentVar> {
    tuple((
        multispace0,
        pipeline_step_var,
        multispace0,
        pipeline_stop_var,
        multispace0,
    ))(input)
    .map(|(next, (_, step, _, stop, _))| (next, PipelineSegmentVar { step, stop }))
}

pub fn pipeline<I: Span>(input: I) -> Res<I, PipelineVar> {
    context(
        "pipeline",
        many0(delimited(multispace0, pipeline_segment, multispace0)),
    )(input)
    .map(|(next, segments)| (next, PipelineVar { segments }))
}

pub fn consume_pipeline<I: Span>(input: I) -> Res<I, PipelineVar> {
    all_consuming(pipeline)(input)
}

pub fn subst<I: Span, F>(f: F) -> impl FnMut(I) -> Res<I, Subst<I>>
where
    F: FnMut(I) -> Res<I, I> + Copy,
{
    move |input: I| {
        many1(chunk(f))(input.clone()).map(|(next, chunks)| {
            let len: usize = chunks.iter().map(|c| c.len()).sum();
            let span = input.slice(0..input.len() - next.len());
            let chunks = Subst {
                chunks,
                trace: span.trace(),
            };
            (next, chunks)
        })
    }
}

pub fn chunk<I: Span, F>(mut f: F) -> impl FnMut(I) -> Res<I, Chunk<I>> + Copy
where
    F: FnMut(I) -> Res<I, I> + Copy,
{
    move |input: I| alt((var_chunk, text_chunk(f)))(input)
}

pub fn text_chunk<I: Span, F>(mut f: F) -> impl FnMut(I) -> Res<I, Chunk<I>> + Copy
where
    F: FnMut(I) -> Res<I, I> + Copy,
{
    move |input: I| f(input).map(|(next, text)| (next, Chunk::Text(text)))
}

pub fn var_chunk<I: Span>(input: I) -> Res<I, Chunk<I>> {
    preceded(
        tag("$"),
        cut(delimited(
            cut(tag("{"))
                .context(BraceErrCtx::new(BraceKindErrCtx::Curly, BraceSideErrCtx::Open).into()),
            recognize(var_case),
            cut(tag("}"))
                .context(BraceErrCtx::new(BraceKindErrCtx::Curly, BraceSideErrCtx::Close).into()),
        )),
    )(input)
    .map(|(next, variable_name)| (next, Chunk::Var(variable_name)))
}

pub fn route_attribute(input: &str) -> Result<RouteSelector, ParseErrs> {
    let input = new_span(input);
    let (_, (_, lex_route)) = result(pair(
        tag("#"),
        unwrap_block(
            BlockKind::Nested(NestedBlockKind::Square),
            pair(
                tag("route"),
                unwrap_block(
                    BlockKind::Nested(NestedBlockKind::Parens),
                    unwrap_block(
                        BlockKind::Delimited(DelimitedBlockKind::DoubleQuotes),
                        nospace0,
                    ),
                ),
            ),
        ),
    )(input.clone()))?;

    route_selector(lex_route)
}

pub fn route_attribute_value(input: &str) -> Result<RouteSelector, ParseErrs> {
    let input = new_span(input);
    let lex_route = result(unwrap_block(
        BlockKind::Delimited(DelimitedBlockKind::DoubleQuotes),
        trim(nospace0),
    )(input.clone()))?;

    route_selector(lex_route)
}

pub fn route_selector<I: Span>(input: I) -> Result<RouteSelector, ParseErrs> {
    let (next, (topic, lex_route)) = match pair(
        opt(terminated(
            unwrap_block(
                BlockKind::Nested(NestedBlockKind::Square),
                value_pattern(topic),
            ),
            tag("::"),
        )),
        lex_route_selector,
    )(input.clone())
    {
        Ok((next, (topic, lex_route))) => (next, (topic, lex_route)),
        Err(err) => {
            return Err(err.into());
        }
    };

    if next.len() > 0 {
        return Err(ParseErrs::from_loc_span(
            "could not consume entire route selector",
            "extra",
            next,
        )
        .into());
    }

    let mut names = lex_route.names.clone();
    names.reverse();
    let method_kind_span = names
        .pop()
        .ok_or(ParseErrs::from_loc_span(
            "expecting MethodKind [ Http, Ext ]",
            "expecting MethodKind",
            input,
        ))?
        .clone();
    let method_kind = result(value_pattern(method_kind)(method_kind_span.clone()))?;
    let method = match &method_kind {
        ValuePattern::Always => ValuePattern::Always,
        ValuePattern::Never => ValuePattern::Never,
        ValuePattern::Pattern(method_kind) => match method_kind {
            MethodKind::Hyp => {
                let method = names.pop().ok_or(ParseErrs::from_loc_span(
                    "Hyp method requires a sub kind i.e. Hyp<Assign> or Ext<*>",
                    "sub kind required",
                    method_kind_span,
                ))?;
                let method = result(value_pattern(sys_method)(method))?;
                ValuePattern::Pattern(MethodPattern::Hyp(method))
            }
            MethodKind::Cmd => {
                let method = names.pop().ok_or(ParseErrs::from_loc_span(
                    "Cmd method requires a sub kind i.e. Cmd<Bounce>",
                    "sub kind required",
                    method_kind_span,
                ))?;
                let method = result(value_pattern(cmd_method)(method))?;
                ValuePattern::Pattern(MethodPattern::Cmd(method))
            }
            MethodKind::Ext => {
                let method = names.pop().ok_or(ParseErrs::from_loc_span(
                    "Ext method requires a sub kind i.e. Ext<SomeExt> or Ext<*>",
                    "sub kind required",
                    method_kind_span,
                ))?;
                let method = result(value_pattern(ext_method)(method))?;
                ValuePattern::Pattern(MethodPattern::Ext(method))
            }
            MethodKind::Http => {
                let method = names.pop().ok_or(ParseErrs::from_loc_span(
                    "Http method requires a sub kind i.e. Http<Get> or Http<*>",
                    "sub kind required",
                    method_kind_span,
                ))?;
                let method = result(value_pattern(http_method)(method))?;
                ValuePattern::Pattern(MethodPattern::Http(method))
            }
        },
    };

    if !names.is_empty() {
        let name = names.pop().unwrap();
        return Err(ParseErrs::from_loc_span("Too many SubKinds: only Http/Ext supported with one subkind i.e. Http<Get>, Ext<MyMethod>", "too many subkinds", name).into());
    }

    let path = match lex_route.path.as_ref() {
        None => Regex::new("/.*").unwrap(),
        Some(i) => match Regex::new(i.to_string().as_str()) {
            Ok(path) => path,
            Err(err) => {
                return Err(ParseErrs::from_loc_span(
                    format!("cannot parse Path regex: '{}'", err.to_string()).as_str(),
                    "path regex error",
                    i.clone(),
                )
                .into());
            }
        },
    };

    Ok(RouteSelector::new(
        topic,
        method,
        path,
        lex_route.filters.to_scope_filters(),
    ))
}

fn find_parse_err<I: Span>(_: &Err<NomErr<I>>) -> ParseErrs {
    todo!()
}
