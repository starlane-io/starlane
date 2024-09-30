use crate::space::command::direct::CmdKind;
use crate::space::command::CommandVar;
use crate::space::config::bind::{PipelineStepVar, PipelineStopVar, RouteSelector, WaveDirection};
use crate::space::config::mechtron::MechtronConfig;
use crate::space::config::Document;
use crate::space::err::report::{Label, Report, ReportKind};
use crate::space::err::{ParseErrs, SpaceErr};
use crate::space::kind::{ArtifactSubKind, BaseKind, DatabaseSubKind, FileSubKind, Kind, KindParts, NativeSub, Specific, StarSub, UserBaseSubKind};
use crate::space::loc::{Layer, Surface, Topic, VarVal, Version};
use crate::space::point::{PointSeg, PointVar};
use crate::space::selector::{ExactPointSeg, Hop, KindBaseSelector, KindSelector, LabeledPrimitiveTypeDef, MapEntryPatternVar, Pattern, PatternBlockVar, PayloadBlockVar, PayloadType2Def, PointSegSelector, Selector, SelectorDef, SpecificSelector, SubKindSelector, UploadBlock, VersionReq};
use crate::space::substance::{CallKind, CallVar, CallWithConfigVar, ExtCall, HttpCall, ListPattern, MapPatternVar, NumRange, SubstanceKind, SubstancePatternVar, SubstanceTypePatternDef};
use crate::space::util::{HttpMethodPattern, StringMatcher, ValuePattern};
use crate::space::wave::core::http2::HttpMethod;
use crate::space::wave::core::{MethodKind, MethodPattern};
use core::str::FromStr;
use model::{BlockKind, Chunk, DelimitedBlockKind, MechtronScope, NestedBlockKind, PipelineSegmentVar, PipelineVar, Subst, TextType};
//    use ariadne::Report;
//    use ariadne::{Label, ReportKind, Source};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{alpha1, digit1, multispace0, satisfy};
use nom::combinator::{all_consuming, cut, fail, opt, recognize, value};
use nom::error::{ErrorKind, ParseError};
use nom::multi::{many0, many1, separated_list0};
use nom::sequence::{delimited, pair, preceded, terminated, tuple};
use nom::{AsChar, Compare, Err, InputLength, InputTake, InputTakeAtPosition, Parser};
use nom_supreme::context::ContextError;
use regex::Regex;
use util::{new_span, span_with_extra, Span};
use {any_block, any_soround_lex_block, camel_case, camel_case_chars, camel_case_to_string_matcher, context, domain, filepath_chars, lex_root_scope, lex_route_selector, method_kind, parse_uuid, point_segment_chars, point_var, skewer_case, skewer_chars, subst_path, unwrap_block, variable_name, version_chars, CamelCase, ErrCtx, ParseTree, Res, SubstParser};
use starlane_space::space::parse;
use starlane_space::space::parse::nospace1;



/*
pub fn just_msg<R, E: From<String>>(
    result: Result<(Span, R), Err<ErrorTree<Span>>>,
) -> Result<R, E> {
    match result {
        Ok((_, e)) => Ok(e),
        Err(err) => match find(&err) {
            Ok((message, _)) => Err(E::from(message)),
            Err(err) => Err(E::from(err)),
        },
    }
}

 */

fn create_err_report<I: Span>(context: &str, loc: I) -> SpaceErr {
    todo!()
}
/*(    fn create_err_report<I: Span>(context: &str, loc: I) -> SpaceErr {

        let mut builder = Report::build(ReportKind::Error, (), 23);

        match NestedBlockKind::error_message(&loc, context) {
            Ok(message) => {
                let builder = builder.with_message(message).with_label(
                    Label::new(loc.location_offset()..loc.location_offset()).with_message(message),
                );
                return ParseErrs::from_report(builder.finish(), loc.extra()).into();
            }
            Err(_) => {}
        }

        let builder = match context {
                "var" => {
                    let f = |input| {preceded(tag("$"),many0(alt((tag("{"),alphanumeric1,tag("-"),tag("_"),multispace1))))(input)};
                    let len = len(f)(loc.clone())+1;
                    builder.with_message("Variables should be lowercase skewer with a leading alphabet character and surrounded by ${} i.e.:'${var-name}' ").with_label(Label::new(loc.location_offset()..loc.location_offset()+len).with_message("Bad Variable Substitution"))
                },
                "assignment:plus" => {
                    builder.with_message("Expecting a preceding '+' (create variable operator)").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("expected '+'"))
                }
                "assignment:equals" => {
                    builder.with_message("Expecting a preceding '=' for assignment").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("expecting '='"))
                }
                "assignment:value" => {
                    builder.with_message("Expecting a value").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("expecting value"))
                }
                "capture-path" => {
                    builder.with_message("Invalid capture path. Legal characters are filesystem characters plus captures $(var=.*) i.e. /users/$(user=.*)").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Illegal capture path"))

                }
                "point" => {
                        builder.with_message("Invalid Point").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Invalid Point"))
                    },

                "resolver-not-available" => {
                    builder.with_message("Var & Working Point resolution are not available in this context").with_label(Label::new(loc.location_offset()..loc.location_offset()+loc.len()).with_message("resolution not available"))
                }
                "var-resolver-not-available" => {
                    builder.with_message("Variable resolution is not available in this context").with_label(Label::new(loc.location_offset()..loc.location_offset()+loc.len()).with_message("var resolution not available"))
                }
                "ctx-resolver-not-available" => {
                    builder.with_message("WorkingPoint resolution is not available in this context").with_label(Label::new(loc.location_offset()..loc.location_offset()+loc.len()).with_message("working point resolution not available"))
                }

                "regex" => {
                    let span = result(nospace1(loc.clone()));
                            match span {
                                Ok(span) => {
                                    match Regex::new(loc.to_string().as_str()) {
                                        Ok(_) => {
                                            builder.with_message("internal parse error: regex error in this expression")
                                        }
                                        Err(err) => {
                                            match err {
                                                Error::Syntax(syntax) => {
                                                    builder.with_message(format!("Regex Syntax Error: '{}'",syntax)).with_label(Label::new(span.location_offset()..span.location_offset()+span.len()).with_message("regex syntax error"))
                                                }
                                                Error::CompiledTooBig(size) => {
                                                    builder.with_message("Regex compiled too big").with_label(Label::new(span.location_offset()..span.location_offset()+span.len()).with_message("regex compiled too big"))
                                                }
                                                _ => {

                                                    builder.with_message("Regex is nonexhaustive").with_label(Label::new(span.location_offset()..span.location_offset()+span.len()).with_message("non-exhaustive regex"))

                                                }
                                            }
                                        }
                                    }
                                }
                        Err(_) => {
                            builder.with_message("internal parse error: could not identify regex")
                        }
                    }
                },
                "expect-camel-case" => { builder.with_message("expecting a CamelCase expression").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("expecting CamelCase"))},
                "expect-skewer-case" => { builder.with_message("expecting a skewer-case expression").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("expecting skewer-case"))},
                "parsed-scopes" => { builder.with_message("expecting a properly formed scope").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("not a scope"))},
                "scope" => { builder.with_message("expecting a properly formed scope").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("not a scope"))},
                "root-scope:block" => { builder.with_message("expecting root scope block {}").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Expecting Scope Block"))},
                "pipeline:stop:expecting" =>{ builder.with_message("expecting a pipeline stop: point, call, or return ('&')").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Expecting Pipeline Stop"))},
                "pipeline:step" =>{ builder.with_message("expecting a pipeline step ('->', '=>', '-[ Bin ]->', etc...)").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Expecting Pipeline Step"))},
                "pipeline:step:entry" =>{ builder.with_message("expecting a pipeline step entry ('-' or '=') to form a pipeline step i.e. '->' or '=>'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Expecting Pipeline Entry"))},
                "pipeline:step:exit" =>{ builder.with_message("expecting a pipeline step exit i.e. '->' or '=>'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Expecting Pipeline Exit"))},
                "pipeline:step:payload" =>{ builder.with_message("Invalid payload filter").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("invalid payload filter"))},
                "scope:expect-space-after-pipeline-step" =>{ builder.with_message("expecting a space after selection pipeline step (->)").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Expecting Space"))},
                "scope-selector-name:expect-alphanumeric-leading" => { builder.with_message("expecting a valid scope selector name starting with an alphabetic character").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Expecting Alpha Char"))},
                "scope-selector-name:expect-termination" => { builder.with_message("expecting scope selector to be followed by a space, a filter declaration: '(filter)->' or a sub scope selector: '<SubScope> or subscope terminator '>' '").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Bad Scope Selector Termination"))},
                "scope-selector-version-closing-tag" =>{ builder.with_message("expecting a closing parenthesis for the root version declaration (no spaces allowed) -> i.e. Bind(version=1.0.0)->").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("missing closing parenthesis"))}
                "scope-selector-version-missing-kazing"=> { builder.with_message("The version declaration needs a little style.  Try adding a '->' to it.  Make sure there are no spaces between the parenthesis and the -> i.e. Bind(version=1.0.0)->").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("missing stylish arrow"))}
                "scope-selector-version" => { builder.with_message("Root config selector requires a version declaration with NO SPACES between the name and the version filter example: Bind(version=1.0.0)->").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("bad version declaration"))}
                "scope-selector-name" => { builder.with_message("Expecting an alphanumeric scope selector name. example: Pipeline").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("expecting scope selector"))}
                "root-scope-selector-name" => { builder.with_message("Expecting an alphanumeric root scope selector name and version. example: Bind(version=1.0.0)->").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("expecting scope selector"))}
                "consume" => { builder.with_message("Expected to be able to consume the entire String")}
                "point:space_segment:dot_dupes" => { builder.with_message("Space Segment cannot have consecutive dots i.e. '..'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Consecutive dots not allowed"))}
                "point:version:root_not_trailing" =>{ builder.with_message("Root filesystem is the only segment allowed to follow a bundle version i.e. 'space:base:2.0.0-version:/dir/somefile.txt'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Only root file segment ':/' allowed here"))}
                "point:space_segment_leading" => {builder.with_message("The leading character of a Space segment must be a lowercase letter").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Invalid Leading Character"))}
                "point:space_segment" => {builder.with_message("A Point Space Segment must be all lowercase, alphanumeric with dashes and dots.  It follows Host and Domain name rules i.e. 'localhost', 'mechtron.io'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Invalid Space Segment"))}
                "point:bad_leading" => {builder.with_message("The leading character must be a lowercase letter (for Base Segments) or a digit (for Version Segments)").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Invalid Leading Character"))}
                "point:base_segment" => {builder.with_message("A Point Base Segment must be 'skewer-case': all lowercase alphanumeric with dashes. The leading character must be a letter.").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Invalid Base Segment Character"))}
                "point:dir_pop" => {builder.with_message("A Point Directory Pop '..'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Something is Wrong"))}
                "point:dir_segment" => {builder.with_message("A Point Dir Segment follows legal filesystem characters and must end in a '/'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Illegal Character"))}
                "point:root_filesystem_segment" => {builder.with_message("Root FileSystem ':/'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Illegal Character"))}
                "point:file_segment" => {builder.with_message("A Point File Segment follows legal filesystem characters").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Illegal Character"))}
                "point:file_or_directory"=> {builder.with_message("A Point File Segment (Files & Directories) follows legal filesystem characters").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Illegal Character"))}
                "point:version_segment" => {builder.with_message("A Version Segment allows all legal SemVer characters").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Illegal Character"))}
                "filter-name" => {builder.with_message("Filter name must be skewer case with leading character").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Invalid filter name"))}
                "kind-base" => {builder.with_message("Invalid Base Kind").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Base Kind not recognized"))}
            "command" => {builder.with_message("Unrecognized Command").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Invalid"))}
                "parsed-scope-selector-kazing" => {builder.with_message("Selector needs some style with the '->' operator either right after the Selector i.e.: 'Pipeline ->' or as part of the filter declaration i.e. 'Pipeline(auth)->'").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Missing or Invalid Kazing Operator( -> )"))}
                "variable" => {
                        builder.with_message("variable name must be alphanumeric lowercase, dashes and dots.  Variables are preceded by the '$' operator and must be sorounded by curly brackets ${env.valid-variable-name}")
                    },
                "variable:close" => {
                    builder.with_message("variable name must be alphanumeric lowercase, dashes and dots.  Variables are preceded by the '$' operator and must be sorounded by curly brackets with no spaces ${env.valid-variable-name}").with_label(Label::new(loc.location_offset()..loc.location_offset()).with_message("Bad Variable Substitution"))
                },

                "child_perms" => {
                        builder.with_message("expecting child permissions form csd (Create, Select, Delete) uppercase indicates set permission (CSD==full permission, csd==no permission)")
                    },
                    "particle_perms" => {
                        builder.with_message("expecting particle permissions form rwx (Read, Write, Execute) uppercase indicates set permission (RWX==full permission, rwx==no permission)")
                    },
                    "permissions" => {
                        builder.with_message("expecting permissions form 'csd-rwx' (Create,Select,Delete)-(Read,Write,Execute) uppercase indicates set permission (CSD-RWX==full permission, csd-rwx==no permission)")
                    }
                    "permissions_mask" => {
                        builder.with_message("expecting permissions mask symbol '+' for 'Or' mask and '&' for 'And' mask. Example:  &csd-RwX removes ----R-X from current permission")
                    }
                    "privilege" => {
                        builder.with_message("privilege name must be '*' for 'full' privileges or an alphanumeric lowercase, dashes and colons i.e. 'props:email:read'")
                    },
                    "access_grant:perm" => {
                        builder.with_message("expecting permissions mask symbol '+' for 'Or' mask and '&' for 'And' mask. Example:  &csd-RwX removes ----R-X from current permission")
                    },
                    "access_grant:priv" => {
                        builder.with_message("privilege name must be '*' for 'full' privileges or an alphanumeric lowercase, dashes and colons i.e. 'props:email:read'")
                    },
                    "access_grant:on" => {
                        builder.with_message("expecting grant 'on' i.e.: 'grant perm +cSd+RwX on localhost:app:** to localhost:app:users:**<User>'")
                    },
                    "access_grant:to" => {
                        builder.with_message("expecting grant 'to' i.e.: 'grant perm +cSd+RwX on localhost:app:** to localhost:app:users:**<User>'")
                    },
                    "point-subst-brute-force" => {
                        builder.with_message("not expecting variables or working point context '.'/'..' in this point")
                    },
                    "access_grant_kind" => {
                        builder.with_message("expecting access grant kind ['super','perm','priv']")
                    },

                    what => {
                        builder.with_message(format!("internal parser error: cannot determine an error message for parse context: {}",what))
                    }
                };

        //            let source = String::from_utf8(loc.get_line_beginning().to_vec() ).unwrap_or("could not parse utf8 of original source".to_string() );
        ParseErrs::from_report(builder.finish(), loc.extra()).into()
    }
    pub fn find_parse_err<I: Span>(err: &Err<ErrorTree<I>>) -> SpaceErr {
        match err {
            Err::Incomplete(_) => "internal parser error: Incomplete".into(),
            Err::Error(err) => find_tree(err),
            Err::Failure(err) => find_tree(err),
        }
    }

    pub enum ErrFind {
        Context(String),
        Message(String),
    }

    pub fn find_tree<I: Span>(err: &ErrorTree<I>) -> SpaceErr {
        match err {
            ErrorTree::Stack { base, contexts } => {
                let (span, context) = contexts.first().unwrap();
                match context {
                        StackContext::Context(context) => {
                            create_err_report(*context, span.clone())
                        }
                        _ => "internal parser error: could not find a parse context in order to generate a useful error message".into()
                    }
            }
            ErrorTree::Base { location, kind } => create_err_report("eof", location.clone()),
            ErrorTree::Alt(alts) => {
                for alt in alts {
                    return find_tree(alt);
                }

                "internal parser error: ErrorTree::Alt could not find a suitable context error in the various alts".into()
            }
        }
    }

    pub fn first_context<I: Span>(
        orig: Err<ErrorTree<I>>,
    ) -> Result<(String, Err<ErrorTree<I>>), ()> {
        match &orig {
            Err::Error(err) => match err {
                ErrorTree::Stack { base, contexts } => {
                    let (_, context) = contexts.first().unwrap();
                    match context {
                        StackContext::Context(context) => Ok((context.to_string(), orig)),
                        _ => Err(()),
                    }
                }
                _ => Err(()),
            },
            _ => Err(()),
        }
    }
}

 */


pub fn pattern<'r, O, E: ParseError<&'r str>, V>(
    mut value: V,
) -> impl FnMut(&'r str) -> IResult<&str, Pattern<O>, E>
where
    V: Parser<&'r str, O, E>,
{
    move |input: &str| {
        let x: Res<Span, Span> = tag("*")(input);
        match x {
            Ok((next, _)) => Ok((next, Pattern::Any)),
            Err(_) => {
                let (next, p) = value.parse(input)?;
                let pattern = Pattern::Exact(p);
                Ok((next, pattern))
            }
        }
    }
}

 */

/*
pub fn context<I: Clone, E: ContextError<I>, F, O>(
    context: &'static str,
    mut f: F,
) -> impl FnMut(I) -> IResult<I, O, E>
    where
        F: Parser<I, O, E>,
{
    move |i: I| match f.parse(i.clone()) {
        Ok(o) => Ok(o),
        Err(Err::Incomplete(i)) => Err(Err::Incomplete(i)),
        Err(Err::Error(e)) => Err(Err::Error(E::add_context(i, context, e))),
        Err(Err::Failure(e)) => Err(Err::Failure(E::add_context(i, context, e))),
    }
}

 */
/*
pub fn value_pattern<E,F,O>(
    mut f: F
) -> impl Fn(&str) -> IResult<&str, ValuePattern<O>, E>
where F: Parser<&'static str,O,E>, E: ContextError<&'static str> {
    move |input: &str| match tag::<&str,&'static str,ErrorTree<&'static str>>("*")(input) {
        Ok((next, _)) => Ok((next, ValuePattern::Any)),
        Err(err) => {
            match f.parse(input.clone()) {
                Ok((input,output)) => {Ok((input,ValuePattern::Pattern(output)))}
                Err(Err::Incomplete(i)) => Err(Err::Incomplete(i)),
                Err(Err::Error(e)) => Err(Err::Error(E::add_context(input.clone(), "value_pattern", e))),
                Err(Err::Failure(e)) => Err(Err::Failure(E::add_context(input.clone(), "value_pattern", e))),
            }
        }
    }
}

 */

/*
pub fn value_pattern<P>(
    parse: fn<I:Span>(input: Span) -> Res<Span, P>,
) -> impl Fn(&str) -> Res<Span, ValuePattern<P>> {
    move |input: &str| match tag::<&str, &str, VerboseError<&str>>("*")(input) {
        Ok((next, _)) => Ok((next, ValuePattern::Any)),
        Err(_) => {
            let (next, p) = parse(input)?;
            let pattern = ValuePattern::Pattern(p);
            Ok((next, pattern))
        }
    }
}
 */

#[derive(Clone)]
pub struct SkewerPatternParser();
impl SubstParser<Pattern<String>> for SkewerPatternParser {
    fn parse_span<I: Span>(&self, span: I) -> Res<I, Pattern<String>> {
        let (next, pattern) = parse::rec_skewer_pattern(span)?;
        let pattern = pattern.to_string_version();
        Ok((next, pattern))
    }
}

#[derive(Clone)]
pub struct DomainPatternParser();
impl SubstParser<Pattern<String>> for DomainPatternParser {
    fn parse_span<I: Span>(&self, span: I) -> Res<I, Pattern<String>> {
        let (next, pattern) = parse::rec_domain_pattern(span)?;
        let pattern = pattern.to_string_version();
        Ok((next, pattern))
    }
}

/*
fn version_req<I:Span>(input: Span) -> Res<Span, VersionReq> {
    let str_input = *input.fragment();
    let rtn:IResult<&str,VersionReq,ErrorTree<&str>> = parse_from_str(version_req_chars).parse(str_input);

    match rtn {
        Ok((next,version_req)) => {
            Ok((span(next), version_req))
        }
        Err(err) => {
            let tree = Err::Error(ErrorTree::from_error_kind(input, ErrorKind::Fail));
            Err(tree)
        }
    }
}

 */

//}

/*
pub fn text_payload_block<I:Span>(input: Span) -> Res<Span, PayloadBlock> {
    delimited(
        tag("+["),
        tuple((
            multispace0,
            delimited(tag("\""), not_quote, tag("\"")),
            multispace0,
        )),
        tag("]"),
    )(input)
    .map(|(next, (_, text, _))| {
        (
            next,
            PayloadBlock::CreatePayload(Payload::Text(text.to_string())),
        )
    })
}*/

/*
pub fn remove_comments_from_span( span: Span )-> Res<Span,Span> {
    let (next,no_comments) = remove_comments(span.clone())?;
    let new = LocatedSpan::new_extra(no_comments.as_str(), span.extra.clone() );
    Ok((next,new))
}
 */

/*
pub fn strip<I:Span>(input: Span) -> Result<Span, ExtErr>
{
    let (_, stripped) = strip_comments(input.clone())?;
    let span = LocatedSpan::new_extra(stripped.as_str().clone(), Arc::new(input.to_string()));
    Ok(span)
}

 */

/*
pub fn entity_selectors<I:Span>(input: Span) -> Res<Span, Vec<Selector<PipelineSelector>>> {
    many0(delimited(multispace0, entity_selector, multispace0))(input)
}

pub fn entity_selector<I:Span>(input: Span) -> Res<Span, Selector<PipelineSelector>> {
    tuple((entity_pattern, multispace0, pipeline, tag(";")))(input)
        .map(|(next, (pattern, _, pipeline, _))| (next, Selector::new(pattern, pipeline)))
}

pub fn msg_selector<I:Span>(input: Span) -> Res<Span, Selector<ExtPipelineSelector>> {
    tuple((msg_pattern_scoped, multispace0, pipeline, tag(";")))(input)
        .map(|(next, (pattern, _, pipeline, _))| (next, Selector::new(pattern, pipeline)))
}

pub fn http_pipeline<I:Span>(input: Span) -> Res<Span, Selector<HttpPipelineSelector>> {
    tuple((http_pattern_scoped, multispace0, pipeline, tag(";")))(input)
        .map(|(next, (pattern, _, pipeline, _))| (next, Selector::new(pattern, pipeline)))
}

pub fn rc_selector<I:Span>(input: Span) -> Res<Span, Selector<RcPipelineSelector>> {
    tuple((rc_pattern_scoped, multispace0, pipeline, tag(";")))(input)
        .map(|(next, (pattern, _, pipeline, _))| (next, Selector::new(pattern, pipeline)))
}

pub fn consume_selector<I:Span>(input: Span) -> Res<Span, Selector<PipelineSelector>> {
    all_consuming(entity_selector)(input)
}

 */

/*
pub fn unwrap_route_selector(input: &str ) -> Result<RouteSelector,ExtErr> {
    let input = new_span(input);
    let input = result(unwrap_block( BlockKind::Nested(NestedBlockKind::Parens),input))?;
}

 */

/*
pub fn topic<I: Span>(input: I) -> Res<I, ValuePattern<Topic>> {
    context(
        "topic",
        delimited(tag("["), value_pattern(skewer_case_chars), tag("]::")),
    )(input)
    .map(|(next, topic)| {
        let topic = match topic {
            ValuePattern::Any => ValuePattern::Any,
            ValuePattern::None => ValuePattern::None,
            ValuePattern::Pattern(topic) => ValuePattern::Pattern(Topic::Tag(topic.to_string())),
        };
        (next, topic)
    })
}

 */

#[cfg(test)]
pub mod test {
    use std::str::FromStr;
    use std::sync::Arc;

    use nom::bytes::complete::escaped;
    use nom::character::complete::{alpha1, anychar, multispace0};
    use nom::combinator::{all_consuming, peek, recognize};
    use nom::error::context;
    use nom::multi::many0;
    use nom::sequence::{delimited, tuple};

    use crate::space::command::direct::create::{
        PointSegTemplate, PointTemplate, Template,
    };
    use crate::space::command::Command;
    use crate::space::config::Document;
    use crate::space::err::SpaceErr;
    use crate::space::point::{Point, PointCtx, PointSegVar, RouteSegVar};
    use crate::space::substance::Substance;
    use crate::space::util;
    use crate::space::util::{log, ToResolved};
    use error::{assignment, doc, pipeline, pipeline_step_var, result, route_attribute};
    use model::{
        BlockKind, DelimitedBlockKind, NestedBlockKind, TerminatedBlockKind,
    };
    use util::{new_span, span_with_extra, Span};
    use *;

    #[test]
    pub fn test_assignment() {
        let config = "+bin=some:bin:somewhere;";
        let assign = log(result(assignment(new_span(config)))).unwrap();
        assert_eq!(assign.key.as_str(), "bin");
        assert_eq!(assign.value.as_str(), "some:bin:somewhere");

        let config = "    +bin   =    some:bin:somewhere;";
        log(result(assignment(new_span(config)))).unwrap();

        let config = "    noplus =    some:bin:somewhere;";
        assert!(log(result(assignment(new_span(config)))).is_err());
        let config = "   +nothing ";
        assert!(log(result(assignment(new_span(config)))).is_err());
        let config = "   +nothing  = ";
        assert!(log(result(assignment(new_span(config)))).is_err());
    }

    #[test]
    pub fn test_mechtron_config() {
        let config = r#"

Mechtron(version=1.0.0) {
    Wasm {
      +bin=repo:1.0.0:/wasm/blah.wasm;
      +name=my-mechtron;
    }
}

         "#;

        let doc = log(doc(config)).unwrap();

        if let Document::MechtronConfig(_) = doc {
        } else {
            assert!(false)
        }
    }

    #[test]
    pub fn test_bad_mechtron_config() {
        let config = r#"

Mechtron(version=1.0.0) {
    Wasm
    varool
      +bin=repo:1.0.0:/wasm/blah.wasm;
      +name=my-mechtron;
    }
}

         "#;

        let doc = log(doc(config)).is_err();
    }

    #[test]
    pub fn test_message_selector() {
        let route =
            util::log(route_attribute("#[route(\"[Topic<*>]::Ext<NewSession>\")]")).unwrap();
        let route = util::log(route_attribute("#[route(\"Hyp<Assign>\")]")).unwrap();

        println!("path: {}", route.path.to_string());
        //println!("filters: {}", route.filters.first().unwrap().name)
    }

    #[test]
    pub fn test_create_command() -> Result<(), SpaceErr> {
        let command = util::log(result(command_line(new_span("create localhost<Space>"))))?;
        let env = Env::new(Point::root());
        let command: Command = util::log(command.to_resolved(&env))?;
        Ok(())
    }

    //    #[test]
    pub fn test_command_line_err() -> Result<(), SpaceErr> {
        let command = util::log(result(command_line(new_span("create localhost<bad>"))))?;
        let env = Env::new(Point::root());
        let command: Command = util::log(command.to_resolved(&env))?;
        Ok(())
    }

    #[test]
    pub fn test_template() -> Result<(), SpaceErr> {
        let t = util::log(result(all_consuming(template)(new_span(
            "localhost<Space>",
        ))))?;
        let env = Env::new(Point::root());
        let t: Template = util::log(t.to_resolved(&env))?;

        let t = util::log(result(base_point_segment(new_span(
            "localhost:base<Space>",
        ))))?;

        let (space, bases): (PointSegVar, Vec<PointSegVar>) = util::log(result(tuple((
            var_seg(root_ctx_seg(space_point_segment)),
            many0(base_seg(var_seg(pop(base_point_segment)))),
        ))(
            new_span("localhost:base:nopo<Space>"),
        )))?;
        println!("space: {}", space.to_string());
        for base in bases {
            println!("\tbase: {}", base.to_string());
        }
        //let t= util::log(result(all_consuming(template)(new_span("localhost:base<Space>"))))?;
        //        let env = Env::new(Point::root());
        //       let t: Template = util::log(t.to_resolved(&env))?;

        Ok(())
    }

    #[test]
    pub fn test_point_template() -> Result<(), SpaceErr> {
        assert!(mesh_eos(new_span(":")).is_ok());
        assert!(mesh_eos(new_span("%")).is_ok());
        assert!(mesh_eos(new_span("x")).is_err());

        assert!(point_var(new_span("localhost:some-%")).is_ok());

        util::log(result(all_consuming(point_template)(new_span("localhost"))))?;

        let template = util::log(result(point_template(new_span("localhost:other:some-%"))))?;
        let template: PointTemplate = util::log(template.collapse())?;
        if let PointSegTemplate::Pattern(child) = template.child_segment_template {
            assert_eq!(child.as_str(), "some-%")
        }

        util::log(result(point_template(new_span("my-domain.com"))))?;
        util::log(result(point_template(new_span("ROOT"))))?;
        Ok(())
    }

    //    #[test]
    pub fn test_point_var() -> Result<(), SpaceErr> {
        util::log(result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:${name}:base",
        ))))?;
        util::log(result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:1.0.0:/dorko/x/",
        ))))?;
        util::log(result(all_consuming(point_var)(new_span(
            "[hub]::my-domain.com:1.0.0:/dorko/${x}/",
        ))))?;
        util::log(result(all_consuming(point_var)(new_span(
            "[hub]::.:1.0.0:/dorko/${x}/",
        ))))?;
        util::log(result(all_consuming(point_var)(new_span(
            "[hub]::..:1.0.0:/dorko/${x}/",
        ))))?;
        let point = util::log(result(point_var(new_span(
            "[hub]::my-domain.com:1.0.0:/dorko/${x}/file.txt",
        ))))?;
        if let Some(PointSegVar::Var(var)) = point.segments.get(4) {
            assert_eq!("x", var.name.as_str());
        } else {
            assert!(false);
        }

        if let Some(PointSegVar::File(file)) = point.segments.get(5) {
            assert_eq!("file.txt", file.as_str());
        } else {
            assert!(false);
        }

        let point = util::log(result(point_var(new_span(
            "${route}::my-domain.com:${name}:base",
        ))))?;

        // this one SHOULD fail and an appropriate error should be located at BAD
        util::log(result(point_var(new_span(
            "${route of routes}::my-domain.com:${BAD}:base",
        ))));

        if let RouteSegVar::Var(ref var) = point.route {
            assert_eq!("route", var.name.as_str());
        } else {
            assert!(false);
        }

        if let Some(PointSegVar::Space(space)) = point.segments.get(0) {
            assert_eq!("my-domain.com", space.as_str());
        } else {
            assert!(false);
        }

        if let Some(PointSegVar::Var(var)) = point.segments.get(1) {
            assert_eq!("name", var.name.as_str());
        } else {
            assert!(false);
        }

        if let Some(PointSegVar::Base(base)) = point.segments.get(2) {
            assert_eq!("base", base.as_str());
        } else {
            assert!(false);
        }

        let mut env = Env::new(Point::from_str("my-domain.com")?);
        env.set_var("route", Substance::Text("[hub]".to_string()));
        env.set_var("name", Substance::Text("zophis".to_string()));
        let point: Point = point.to_resolved(&env)?;
        println!("point.to_string(): {}", point.to_string());

        util::log(
            util::log(result(all_consuming(point_var)(new_span(
                "[hub]::my-domain.com:1.0.0:/dorko/x/",
            ))))?
                .to_point(),
        );
        util::log(
            util::log(result(all_consuming(point_var)(new_span(
                "[hub]::my-domain.com:1.0.0:/${dorko}/x/",
            ))))?
                .to_point(),
        );
        util::log(
            util::log(result(all_consuming(point_var)(new_span(
                "${not-supported}::my-domain.com:1.0.0:/${dorko}/x/",
            ))))?
                .to_point(),
        );

        let point = util::log(result(point_var(new_span("${route}::${root}:base1"))))?;
        let mut env = Env::new(Point::from_str("my-domain.com:blah")?);
        env.set_var("route", Substance::Text("[hub]".to_string()));
        env.set_var("root", Substance::Text("..".to_string()));

        let point: PointCtx = util::log(point.to_resolved(&env))?;

        /*
                let resolver = Env::new(Point::from_str("my-domain.com:under:over")?);
                let point = log(consume_point_var("../../hello") )?;
        //        let point: Point = log(point.to_resolved(&resolver))?;
          //      println!("point.to_string(): {}", point.to_string());
                let _: Result<Point, ExtErr> = log(log(result(all_consuming(point_var)(new_span(
                    "${not-supported}::my-domain.com:1.0.0:/${dorko}/x/",
                )))?
                    .to_resolved(&env)));

                 */
        Ok(())
    }

    #[test]
    pub fn test_point() -> Result<(), SpaceErr> {
        util::log(
            result(all_consuming(point_var)(new_span(
                "[hub]::my-domain.com:name:base",
            )))?
                .to_point(),
        )?;
        util::log(
            result(all_consuming(point_var)(new_span(
                "[hub]::my-domain.com:1.0.0:/dorko/x/",
            )))?
                .to_point(),
        )?;
        util::log(
            result(all_consuming(point_var)(new_span(
                "[hub]::my-domain.com:1.0.0:/dorko/xyz/",
            )))?
                .to_point(),
        )?;

        Ok(())
    }

    #[test]
    pub fn test_simple_point_var() -> Result<(), SpaceErr> {
        /*
        let point = util::log(result(point_var(new_span("localhost:base"))))?;
        println!("point '{}'", point.to_string());
        let point :Point = point.collapse()?;
        assert_eq!("localhost:base", point.to_string().as_str());
        let point = util::log(result(point_var(new_span("localhost:base<Kind>"))))?;
        let point :Point = point.collapse()?;
        assert_eq!("localhost:base", point.to_string().as_str());

        let point = util::log(result(point_var(new_span("localhost:base:3.0.0<Kind>"))))?;
        let point :Point = point.collapse()?;
        assert_eq!("localhost:base:3.0.0", point.to_string().as_str());
        let point = util::log(result(point_var(new_span("localhost:base:3.0.0:/some/file.txt<Kind>"))))?;
        assert_eq!("localhost:base:3.0.0:/some/file.txt", point.to_string().as_str());
        let point :Point = point.collapse()?;
        println!("point: '{}'",point.to_string());

        for seg in &point.segments {
            println!("\tseg: '{}'",seg.to_string());
        }
        assert_eq!("some/",point.segments.get(4).unwrap().to_string().as_str());

         */

        let point = util::log(result(point_var(new_span(
            "localhost:base:/fs/file.txt<Kind>",
        ))))?;
        let point: Point = point.collapse()?;
        assert_eq!("localhost:base:/fs/file.txt", point.to_string().as_str());

        Ok(())
    }

    #[test]
    pub fn test_lex_block() -> Result<(), SpaceErr> {
        let esc = result(escaped(anychar, '\\', anychar)(new_span("\\}")))?;
        //println!("esc: {}", esc);
        util::log(result(all_consuming(lex_block(BlockKind::Nested(
            NestedBlockKind::Curly,
        )))(new_span("{}"))))?;
        util::log(result(all_consuming(lex_block(BlockKind::Nested(
            NestedBlockKind::Curly,
        )))(new_span("{x}"))))?;
        util::log(result(all_consuming(lex_block(BlockKind::Nested(
            NestedBlockKind::Curly,
        )))(new_span("{\\}}"))))?;
        util::log(result(all_consuming(lex_block(BlockKind::Delimited(
            DelimitedBlockKind::SingleQuotes,
        )))(new_span("'hello'"))))?;
        util::log(result(all_consuming(lex_block(BlockKind::Delimited(
            DelimitedBlockKind::SingleQuotes,
        )))(new_span("'ain\\'t it cool?'"))))?;

        //assert!(log(result(all_consuming(lex_block( BlockKind::Nested(NestedBlockKind::Curly)))(create_span("{ }}")))).is_err());
        Ok(())
    }
    #[test]
    pub fn test_path_regex2() -> Result<(), SpaceErr> {
        util::log(result(path_regex(new_span("/xyz"))))?;
        Ok(())
    }
    #[test]
    pub fn test_bind_config() -> Result<(), SpaceErr> {
        let bind_config_str = r#"Bind(version=1.0.0)  { Route<Http> -> { <Get> -> ((*)) => &; } }
        "#;

        util::log(doc(bind_config_str))?;
        if let Document::BindConfig(bind) = util::log(doc(bind_config_str))? {
            assert_eq!(bind.route_scopes().len(), 1);
            let mut pipelines = bind.route_scopes();
            let pipeline_scope = pipelines.pop().unwrap();
            assert_eq!(pipeline_scope.selector.selector.name.as_str(), "Route");
            let message_scope = pipeline_scope.block.first().unwrap();
            assert_eq!(
                message_scope.selector.selector.name.to_string().as_str(),
                "Http"
            );
            let method_scope = message_scope.block.first().unwrap();
            assert_eq!(
                method_scope.selector.selector.name.to_string().as_str(),
                "Http<Get>"
            );
        } else {
            assert!(false);
        }

        let bind_config_str = r#"Bind(version=1.0.0)  {
              Route<Ext<Create>> -> localhost:app => &;
           }"#;

        if let Document::BindConfig(bind) = util::log(doc(bind_config_str))? {
            assert_eq!(bind.route_scopes().len(), 1);
            let mut pipelines = bind.route_scopes();
            let pipeline_scope = pipelines.pop().unwrap();
            assert_eq!(pipeline_scope.selector.selector.name.as_str(), "Route");
            let message_scope = pipeline_scope.block.first().unwrap();
            assert_eq!(
                message_scope.selector.selector.name.to_string().as_str(),
                "Ext"
            );
            let action_scope = message_scope.block.first().unwrap();
            assert_eq!(
                action_scope.selector.selector.name.to_string().as_str(),
                "Ext<Create>"
            );
        } else {
            assert!(false);
        }

        let bind_config_str = r#"  Bind(version=1.0.0) {
              Route -> {
                 <*> -> {
                    <Get>/users/(?P<user>)/.* -> localhost:users:${user} => &;
                 }
              }
           }

           "#;
        util::log(doc(bind_config_str))?;

        let bind_config_str = r#"  Bind(version=1.0.0) {
              Route -> {
                 <Http<*>>/users -> localhost:users => &;
              }
           }

           "#;
        util::log(doc(bind_config_str))?;

        let bind_config_str = r#"  Bind(version=1.0.0) {
              * -> { // This should fail since Route needs to be defined
                 <*> -> {
                    <Get>/users -> localhost:users => &;
                 }
              }
           }

           "#;
        assert!(util::log(doc(bind_config_str)).is_err());
        let bind_config_str = r#"  Bind(version=1.0.0) {
              Route<Rc> -> {
                Create ; Bok;
                  }
           }

           "#;
        assert!(util::log(doc(bind_config_str)).is_err());
        //   assert!(log(config(bind_config_str)).is_err());

        Ok(())
    }

    #[test]
    pub fn test_pipeline_segment() -> Result<(), SpaceErr> {
        util::log(result(pipeline_segment(new_span("-> localhost"))))?;
        assert!(util::log(result(pipeline_segment(new_span("->")))).is_err());
        assert!(util::log(result(pipeline_segment(new_span("localhost")))).is_err());
        Ok(())
    }

    #[test]
    pub fn test_pipeline_stop() -> Result<(), SpaceErr> {
        util::log(result(space_chars(new_span("localhost"))))?;
        util::log(result(space_no_dupe_dots(new_span("localhost"))))?;

        util::log(result(mesh_eos(new_span(""))))?;
        util::log(result(mesh_eos(new_span(":"))))?;

        util::log(result(recognize(tuple((
            context("point:space_segment_leading", peek(alpha1)),
            space_no_dupe_dots,
            space_chars,
        )))(new_span("localhost"))))?;
        util::log(result(space_point_segment(new_span("localhost.com"))))?;

        util::log(result(point_var(new_span("mechtron.io:app:hello")))?.to_point())?;
        util::log(result(pipeline_stop_var(new_span("localhost:app:hello"))))?;
        Ok(())
    }

    #[test]
    pub fn test_pipeline() -> Result<(), SpaceErr> {
        util::log(result(pipeline(new_span("-> localhost => &"))))?;
        Ok(())
    }

    #[test]
    pub fn test_pipeline_step() -> Result<(), SpaceErr> {
        util::log(result(pipeline_step_var(new_span("->"))))?;
        util::log(result(pipeline_step_var(new_span("-[ Text ]->"))))?;
        util::log(result(pipeline_step_var(new_span("-[ Text ]=>"))))?;
        util::log(result(pipeline_step_var(new_span("=[ Text ]=>"))))?;

        assert!(util::log(result(pipeline_step_var(new_span("=")))).is_err());
        assert!(util::log(result(pipeline_step_var(new_span("-[ Bin ]=")))).is_err());
        assert!(util::log(result(pipeline_step_var(new_span("[ Bin ]=>")))).is_err());
        Ok(())
    }

    #[test]
    pub fn test_rough_bind_config() -> Result<(), SpaceErr> {
        let unknown_config_kind = r#"
Unknown(version=1.0.0) # mem unknown config kind
{
    Route{
    }
}"#;
        let unsupported_bind_version = r#"
Bind(version=100.0.0) # mem unsupported version
{
    Route{
    }
}"#;
        let multiple_unknown_sub_selectors = r#"
Bind(version=1.0.0)
{
    Whatever -> { # Someone doesn't care what sub selectors he creates
    }

    Dude(filter $(value)) -> {}  # he doesn't care one bit!

}"#;

        let now_we_got_rows_to_parse = r#"
Bind(version=1.0.0)
{
    Route(auth) -> {
       Http {
          <$(method=.*)>/users/$(user=.*)/$(path=.*)-> localhost:app:users:$(user)^Http<$(method)>/$(path) => &;
          <Get>/logout -> localhost:app:mechtrons:logout-handler => &;
       }
    }

    Route -> {
       Ext<FullStop> -> localhost:apps:
       * -> localhost:app:bad-page => &;
    }


}"#;
        util::log(doc(unknown_config_kind));
        util::log(doc(unsupported_bind_version));
        util::log(doc(multiple_unknown_sub_selectors));
        util::log(doc(now_we_got_rows_to_parse));

        Ok(())
    }

    #[test]
    pub fn test_remove_comments() -> Result<(), SpaceErr> {
        let bind_str = r#"
# this is a mem of comments
Bind(version=1.0.0)->
{
  # let's see if it works a couple of spaces in.
  Route(auth)-> {  # and if it works on teh same line as something we wan to keep

  }

  # looky!  I deliberatly put an error here (space between the filter and the kazing -> )
  # My hope is that we will get a an appropriate error message WITH COMMENTS INTACT
  Route(noauth)-> # look!  I made a boo boo
  {
     # nothign to see here
  }
}"#;

        match doc(bind_str) {
            Ok(_) => {}
            Err(err) => {
                err.print();
            }
        }

        Ok(())
    }

    #[test]
    pub fn test_version() -> Result<(), SpaceErr> {
        rec_version(new_span("1.0.0"))?;
        rec_version(new_span("1.0.0-alpha"))?;
        version(new_span("1.0.0-alpha"))?;

        Ok(())
    }
    #[test]
    pub fn test_rough_block() -> Result<(), SpaceErr> {
        result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
            new_span("{  }"),
        ))?;
        result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
            new_span("{ {} }"),
        ))?;
        assert!(
            result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
                new_span("{ } }")
            ))
                .is_err()
        );
        // this is allowed by rough_block
        result(all_consuming(lex_nested_block(NestedBlockKind::Curly))(
            new_span("{ ] }"),
        ))?;

        result(lex_nested_block(NestedBlockKind::Curly)(new_span(
            r#"x blah


Hello my friend


        }"#,
        )))
            .err()
            .unwrap()
            .print();

        result(lex_nested_block(NestedBlockKind::Curly)(new_span(
            r#"{

Hello my friend


        "#,
        )))
            .err()
            .unwrap()
            .print();
        Ok(())
    }

    #[test]
    pub fn test_block() -> Result<(), SpaceErr> {
        util::log(result(lex_nested_block(NestedBlockKind::Curly)(new_span(
            "{ <Get> -> localhost; }    ",
        ))))?;
        if true {
            return Ok(());
        }
        all_consuming(nested_block(NestedBlockKind::Curly))(new_span("{  }"))?;
        all_consuming(nested_block(NestedBlockKind::Curly))(new_span("{ {} }"))?;
        util::log(result(nested_block(NestedBlockKind::Curly)(new_span(
            "{ [] }",
        ))))?;
        assert!(
            expected_block_terminator_or_non_terminator(NestedBlockKind::Curly)(new_span("}"))
                .is_ok()
        );
        assert!(
            expected_block_terminator_or_non_terminator(NestedBlockKind::Curly)(new_span("]"))
                .is_err()
        );
        assert!(
            expected_block_terminator_or_non_terminator(NestedBlockKind::Square)(new_span("x"))
                .is_ok()
        );
        assert!(nested_block(NestedBlockKind::Curly)(new_span("{ ] }")).is_err());
        result(nested_block(NestedBlockKind::Curly)(new_span(
            r#"{



        ]


        }"#,
        )))
            .err()
            .unwrap()
            .print();
        Ok(())
    }

    //#[test]
    pub fn test_root_scope_selector() -> Result<(), SpaceErr> {
        assert!(
            (result(root_scope_selector(new_span(
                r#"

            Bind(version=1.0.0)->"#,
            )))
                .is_ok())
        );

        assert!(
            (result(root_scope_selector(new_span(
                r#"

            Bind(version=1.0.0-alpha)->"#,
            )))
                .is_ok())
        );

        result(root_scope_selector(new_span(
            r#"

            Bind(version=1.0.0) ->"#,
        )))
            .err()
            .unwrap()
            .print();

        result(root_scope_selector(new_span(
            r#"

        Bind   x"#,
        )))
            .err()
            .unwrap()
            .print();

        result(root_scope_selector(new_span(
            r#"

        (Bind(version=3.2.0)   "#,
        )))
            .err()
            .unwrap()
            .print();

        Ok(())
    }

    //    #[test]
    pub fn test_scope_filter() -> Result<(), SpaceErr> {
        result(scope_filter(new_span("(auth)")))?;
        result(scope_filter(new_span("(auth )")))?;
        result(scope_filter(new_span("(auth hello)")))?;
        result(scope_filter(new_span("(auth +hello)")))?;
        result(scope_filters(new_span("(auth +hello)->")))?;
        result(scope_filters(new_span("(auth +hello)-(filter2)->")))?;
        result(scope_filters(new_span("(3auth +hello)-(filter2)->")))
            .err()
            .unwrap()
            .print();
        result(scope_filters(new_span("(a?th +hello)-(filter2)->")))
            .err()
            .unwrap()
            .print();
        result(scope_filters(new_span("(auth +hello)-(filter2) {}")))
            .err()
            .unwrap()
            .print();

        assert!(skewer_case_chars(new_span("3x")).is_err());

        Ok(())
    }
    #[test]
    pub fn test_next_selector() {
        assert_eq!(
            "Http",
            next_stacked_name(new_span("Http"))
                .unwrap()
                .1
                .0
                .to_string()
                .as_str()
        );
        assert_eq!(
            "Http",
            next_stacked_name(new_span("<Http>"))
                .unwrap()
                .1
                .0
                .to_string()
                .as_str()
        );
        assert_eq!(
            "Http",
            next_stacked_name(new_span("Http<Ext>"))
                .unwrap()
                .1
                .0
                .to_string()
                .as_str()
        );
        assert_eq!(
            "Http",
            next_stacked_name(new_span("<Http<Ext>>"))
                .unwrap()
                .1
                .0
                .to_string()
                .as_str()
        );

        assert_eq!(
            "*",
            next_stacked_name(new_span("<*<Ext>>"))
                .unwrap()
                .1
                .0
                .to_string()
                .as_str()
        );

        assert_eq!(
            "*",
            next_stacked_name(new_span("*"))
                .unwrap()
                .1
                .0
                .to_string()
                .as_str()
        );

        assert!(next_stacked_name(new_span("<*x<Ext>>")).is_err());
    }
    #[test]
    pub fn test_lex_scope2() -> Result<(), SpaceErr> {
        /*        let scope = log(result(lex_scopes(create_span(
                   "  Get -> {}\n\nPut -> {}   ",
               ))))?;

        */
        util::log(result(many0(delimited(
            multispace0,
            lex_scope,
            multispace0,
        ))(new_span(""))))?;
        util::log(result(path_regex(new_span("/root/$(subst)"))))?;
        util::log(result(path_regex(new_span("/users/$(user=.*)"))))?;

        Ok(())
    }

    #[test]
    pub fn test_lex_scope() -> Result<(), SpaceErr> {
        let pipes = util::log(result(lex_scope(new_span("Pipes -> {}")))).unwrap();

        //        let pipes = log(result(lex_scope(create_span("Pipes {}"))));

        assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
        assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
        assert_eq!(pipes.block.content.len(), 0);
        assert!(pipes.selector.filters.is_empty());
        assert!(pipes.pipeline_step.is_some());

        assert!(util::log(result(lex_scope(new_span("Pipes {}")))).is_err());

        let pipes = util::log(result(lex_scope(new_span("Pipes -> 12345;"))))?;
        assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
        assert_eq!(pipes.block.content.to_string().as_str(), "-> 12345");
        assert_eq!(
            pipes.block.kind,
            BlockKind::Terminated(TerminatedBlockKind::Semicolon)
        );
        assert_eq!(pipes.selector.filters.len(), 0);
        assert!(pipes.pipeline_step.is_none());
        let pipes = util::log(result(lex_scope(new_span(
            //This time adding a space before the 12345... there should be one space in the content, not two
            r#"Pipes ->  12345;"#,
        ))))?;
        assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
        assert_eq!(pipes.block.content.to_string().as_str(), "->  12345");
        assert_eq!(
            pipes.block.kind,
            BlockKind::Terminated(TerminatedBlockKind::Semicolon)
        );
        assert_eq!(pipes.selector.filters.len(), 0);
        assert!(pipes.pipeline_step.is_none());

        let pipes = util::log(result(lex_scope(new_span("Pipes(auth) -> {}"))))?;

        assert_eq!(pipes.selector.name.to_string().as_str(), "Pipes");
        assert_eq!(pipes.block.content.len(), 0);
        assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
        assert_eq!(pipes.selector.filters.len(), 1);
        assert!(pipes.pipeline_step.is_some());

        let pipes = util::log(result(lex_scope(new_span("Route<Ext> -> {}"))))?;

        assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
        assert_eq!(
            Some(
                pipes
                    .selector
                    .children
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .as_str()
            ),
            Some("<Ext>")
        );

        assert_eq!(pipes.block.content.to_string().as_str(), "");
        assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
        assert_eq!(pipes.selector.filters.len(), 0);
        assert!(pipes.pipeline_step.is_some());

        let pipes = util::log(result(lex_scope(new_span(
            "Route<Http>(noauth) -> {zoink!{}}",
        ))))?;
        assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
        assert_eq!(
            Some(
                pipes
                    .selector
                    .children
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .as_str()
            ),
            Some("<Http>")
        );
        assert_eq!(pipes.block.content.to_string().as_str(), "zoink!{}");
        assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
        assert_eq!(pipes.selector.filters.len(), 1);
        //        assert_eq!(Some(pipes.pipeline_step.unwrap().to_string().as_str()),Some("->") );

        let msg = "Hello my future friend";
        let parseme = format!("<Http<Get>> -> {};", msg);
        let pipes = util::log(result(lex_scope(new_span(parseme.as_str()))))?;

        assert_eq!(pipes.selector.name.to_string().as_str(), "Http");
        assert_eq!(
            pipes.block.content.to_string().as_str(),
            format!("-> {}", msg)
        );
        assert_eq!(
            pipes.block.kind,
            BlockKind::Terminated(TerminatedBlockKind::Semicolon)
        );
        assert_eq!(pipes.selector.filters.len(), 0);
        assert!(pipes.pipeline_step.is_none());

        assert_eq!(
            lex_scope_selector(new_span("<Route<Http>>/users/",))
                .unwrap()
                .0
                .len(),
            0
        );

        util::log(result(lex_scope_selector(new_span(
            "Route<Http<Get>>/users/",
        ))))
            .unwrap();

        let pipes = util::log(result(lex_scope(new_span(
            "Route<Http<Get>>/blah -[Text ]-> {}",
        ))))
            .unwrap();
        assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
        assert_eq!(
            Some(
                pipes
                    .selector
                    .children
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .as_str()
            ),
            Some("<Http<Get>>")
        );
        assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
        assert_eq!(pipes.selector.filters.len(), 0);
        assert_eq!(
            pipes.pipeline_step.as_ref().unwrap().to_string().as_str(),
            "-[Text ]->"
        );

        let pipes = util::log(result(lex_scope(new_span(
            "Route<Http<Get>>(auth)/users/ -[Text ]-> {}",
        ))))?;
        assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
        assert_eq!(
            Some(
                pipes
                    .selector
                    .children
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .as_str()
            ),
            Some("<Http<Get>>")
        );
        assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
        assert_eq!(pipes.selector.filters.len(), 1);
        assert_eq!(
            pipes.pipeline_step.as_ref().unwrap().to_string().as_str(),
            "-[Text ]->"
        );

        let pipes = util::log(result(lex_scope(new_span(
            "Route<Http<Get>>(auth)-(blah xyz)/users/ -[Text ]-> {}",
        ))))?;
        assert_eq!(pipes.selector.name.to_string().as_str(), "Route");
        assert_eq!(
            Some(
                pipes
                    .selector
                    .children
                    .as_ref()
                    .unwrap()
                    .to_string()
                    .as_str()
            ),
            Some("<Http<Get>>")
        );
        assert_eq!(pipes.block.kind, BlockKind::Nested(NestedBlockKind::Curly));
        assert_eq!(pipes.selector.filters.len(), 2);
        assert_eq!(
            pipes.pipeline_step.as_ref().unwrap().to_string().as_str(),
            "-[Text ]->"
        );

        let (next, stripped) = strip_comments(new_span(
            r#"Route<Http>(auth)-(blah xyz)/users/ -[Text]-> {

            Get -> {}
            <Put>(superuser) -> localhost:app => &;
            Post/users/scott -> localhost:app^Ext<SuperScott> => &;

        }"#,
        ))?;
        let span = span_with_extra(stripped.as_str(), Arc::new(stripped.to_string()));
        let pipes = util::log(result(lex_scope(span)))?;

        let pipes = util::log(result(lex_scope(new_span("* -> {}"))))?;

        /* let pipes = log(result(lex_scope(create_span(
            "* -> {}",
        ))))?;

        */
        Ok(())
    }

    pub fn test_nesting_bind() {
        let pipes = util::log(result(lex_scope(new_span(
            r#"


            Route<Http>/auth/.*(auth) -> {

                   <Get>/auth/more ->

            }"#,
        ))))
            .unwrap();
    }

    //#[test]
    pub fn test_root_and_subscope_phases() -> Result<(), SpaceErr> {
        let config = r#"
Bind(version=1.2.3)-> {
   Route -> {
   }

   Route(auth)-> {
   }
}

        "#;

        let root = result(root_scope(new_span(config)))?;

        util::log(lex_scopes(root.block.content.clone()));
        let sub_scopes = lex_scopes(root.block.content.clone())?;

        assert_eq!(sub_scopes.len(), 2);

        Ok(())
    }
    #[test]
    pub fn test_variable_name() -> Result<(), SpaceErr> {
        assert_eq!(
            "v".to_string(),
            util::log(result(lowercase1(new_span("v"))))?.to_string()
        );
        assert_eq!(
            "var".to_string(),
            util::log(result(skewer_dot(new_span("var"))))?.to_string()
        );

        util::log(result(variable_name(new_span("var"))))?;
        Ok(())
    }

    //#[test]
    pub fn test_subst() -> Result<(), SpaceErr> {
        /*
        #[derive(Clone)]
        pub struct SomeParser();
        impl SubstParser<String> for SomeParser {
            fn parse_span<'a>(&self, span: I) -> Res<I, String> {
                recognize(terminated(
                    recognize(many0(pair(peek(not(eof)), recognize(anychar)))),
                    eof,
                ))(span)
                .map(|(next, span)| (next, span.to_string()))
            }
        }

        let chunks = log(result(subst(SomeParser())(create_span("123[]:${var}:abc"))))?;
        assert_eq!(chunks.chunks.len(), 3);
        let mut resolver = MapResolver::new();
        resolver.insert("var", "hello");
        let resolved = log(chunks.resolve_vars(&resolver))?;

        let chunks = log(result(subst(SomeParser())(create_span(
            "123[]:\\${var}:abc",
        ))))?;
        let resolved = log(chunks.resolve_vars(&resolver))?;

        let r = log(result(subst(SomeParser())(create_span(
            "123[    ]:${var}:abc",
        ))))?;
        println!("{}", r.to_string());
        log(result(subst(SomeParser())(create_span("123[]:${vAr}:abc"))));
        log(result(subst(SomeParser())(create_span(
            "123[]:${vAr }:abc",
        ))));

        Ok(())

         */
        unimplemented!()
    }
}