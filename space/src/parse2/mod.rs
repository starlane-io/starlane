use ariadne::{Label, Report, ReportKind, Source};
use futures::TryStreamExt;
use crate::parse::util::{preceded, Span};
use nom::character::complete::alpha1;
use nom::combinator::all_consuming;
use nom::error::{convert_error, ErrorKind, FromExternalError, ParseError, VerboseError, VerboseErrorKind};
use nom::multi::separated_list1;
use nom::sequence::pair;
use nom::{Finish, IResult, Offset, Parser};
use nom::bytes::complete::tag;
use nom_locate::LocatedSpan;
use nom_supreme::context::ContextError;
use nom_supreme::error::{ErrorTree, GenericErrorTree};
use strum_macros::{Display, EnumString};
use crate::err::ParseErrs;
use nom_supreme::parser_ext::ParserExt;
//use nom_supreme::tag::complete::tag;
use nom_supreme::context;
use nom_supreme::tag::TagError;
use thiserror::Error;

type Input<'a> = LocatedSpan<&'a str,&'a str>;

pub fn new_input<'a>(src: &'a str) -> Input<'a> {
    Input::new_extra(src,src)
}

pub type Res<'a,O> = IResult<Input<'a>, O,VerboseError<Input<'a>>>;
pub type StupidRes<'a,O> = IResult<Input<'a>, O,StupidErr<'a>>;



#[derive(Debug)]
struct StupidErr<'a> {
    pub errors: Vec<(Input<'a>, ErrKind)>,
}

#[derive(Debug,Eq,PartialEq,Hash,Clone,Error)]
pub enum ErrKind {
    #[error("unexpected: nom::ErrorKind lacks 'Display'")]
    Nom(ErrorKind),
    #[error("not expecting: '{0}'")]
    Char(char),
    #[error("{0}")]
    Context(ParseCtx),
}

#[derive(Debug,Eq,PartialEq,Hash,Clone,Display,EnumString,Error)]
pub enum ParseCtx {
    Yuk
}
impl <'a> ParseError<Input<'a>> for StupidErr<'a> {
    fn from_error_kind(input: Input<'a>, kind: ErrorKind) -> Self {
        Self{
            errors: vec![(input, ErrKind::Nom(kind))],
        }
    }

    fn append(input: Input<'a>, kind: ErrorKind, mut other: Self) -> Self {
        other.errors.push((input, ErrKind::Nom(kind)));
        other
    }

    fn from_char(input: Input<'a>, c: char) -> Self {
        Self{
            errors: vec![(input, ErrKind::Char(c))],
        }
    }
}
impl <'a> ContextError<Input<'a>, ParseCtx> for StupidErr<'a> {
    fn add_context(input: Input<'a>, err: ParseCtx, mut other: Self) -> Self {
        other.errors.push((input,ErrKind::Context(err)));
        other
    }
}

impl <'a> TagError<Input<'a>, ParseCtx> for StupidErr<'a> {
    fn from_tag(input: Input<'a>, tag: ParseCtx) -> Self {
        todo!()
    }
}


pub fn segments(i: Input) -> StupidRes<Vec<Input>> {

    let mut parser = pair(separated_list1(tag(":"),alpha1::<Input,StupidErr>),preceded(tag("^"),alpha1)).context(ParseCtx::Yuk);

    parser.parse(i).map(|(next,(segments,extra))|(next,segments))

    //pair(context("segments",separated_list1(tag(":"),alpha1)),context("yikes",preceded(tag("^"),alpha1)))(i).map(|(next,(segments,_))|(next,segments))
//    Err(nom::Err::Failure(NomErr::from_error_kind(i,ErrorKind::Alpha)))
}



/*
pub fn convert<'a>(e: StupidErr<'a>) -> VerboseError<&'a str> {
   let errors = e.errors.iter().map(|(input,kind)|{

       (*input.fragment(),kind.clone())}).collect();
    VerboseError { errors }
}

 */

/*
pub fn convert(e: VerboseError<Input>) -> VerboseError<&str> {
    let errors = e.errors.iter().map(|(input,kind)|{


        println!("Kind: {:?}",kind);


        (*input.fragment(),kind.clone())}).collect();
    VerboseError { errors }
}*/

#[test]
fn test() {
    let input = new_input("you:are:^awesome");
    let error = all_consuming(segments)(input.clone()).finish().unwrap_err();
    log(error);
}

fn log<'a>(err: StupidErr<'a>) {
    for (input,err) in err.errors {

            Report::build(ReportKind::Error, 0..input.extra.len())
                .with_message(err.to_string())
                .with_label(Label::new(input.location_offset()..input.len()).with_message("This is of type Nat"))
                .finish()
                .print(Source::from(input.extra))
                .unwrap();
    }
}



/*
pub fn convert_error<'a,I: >(
    input: I,
    e: VerboseError<I>,
) -> String {
    use std::fmt::Write;
    use nom::Offset;

    let mut result = String::new();

    for (i, (substring, kind)) in e.errors.iter().enumerate() {
        let offset = input.offset(substring);

        if input.is_empty() {
            match kind {
                VerboseErrorKind::Char(c) => {
                    write!(&mut result, "{}: expected '{}', got empty input\n\n", i, c)
                }
                VerboseErrorKind::Context(s) => write!(&mut result, "{}: in {}, got empty input\n\n", i, s),
                VerboseErrorKind::Nom(e) => write!(&mut result, "{}: in {:?}, got empty input\n\n", i, e),
            }
        } else {
            let prefix = &input.as_bytes()[..offset];

            // Count the number of newlines in the first `offset` bytes of input
            let line_number = prefix.iter().filter(|&&b| b == b'\n').count() + 1;

            // Find the line that includes the subslice:
            // Find the *last* newline before the substring starts
            let line_begin = prefix
                .iter()
                .rev()
                .position(|&b| b == b'\n')
                .map(|pos| offset - pos)
                .unwrap_or(0);

            // Find the full line after that newline
            let line = input[line_begin..]
                .lines()
                .next()
                .unwrap_or(&input[line_begin..])
                .trim_end();

            // The (1-indexed) column number is the offset of our substring into that line
            let column_number = line.offset(substring) + 1;

            match kind {
                VerboseErrorKind::Char(c) => {
                    if let Some(actual) = substring.chars().next() {
                        write!(
                            &mut result,
                            "{i}: at line {line_number}:\n\
               {line}\n\
               {caret:>column$}\n\
               expected '{expected}', found {actual}\n\n",
                            i = i,
                            line_number = line_number,
                            line = line,
                            caret = '^',
                            column = column_number,
                            expected = c,
                            actual = actual,
                        )
                    } else {
                        write!(
                            &mut result,
                            "{i}: at line {line_number}:\n\
               {line}\n\
               {caret:>column$}\n\
               expected '{expected}', got end of input\n\n",
                            i = i,
                            line_number = line_number,
                            line = line,
                            caret = '^',
                            column = column_number,
                            expected = c,
                        )
                    }
                }
                VerboseErrorKind::Context(s) => write!(
                    &mut result,
                    "{i}: at line {line_number}, in {context}:\n\
             {line}\n\
             {caret:>column$}\n\n",
                    i = i,
                    line_number = line_number,
                    context = s,
                    line = line,
                    caret = '^',
                    column = column_number,
                ),
                VerboseErrorKind::Nom(e) => write!(
                    &mut result,
                    "{i}: at line {line_number}, in {nom_err:?}:\n\
             {line}\n\
             {caret:>column$}\n\n",
                    i = i,
                    line_number = line_number,
                    nom_err = e,
                    line = line,
                    caret = '^',
                    column = column_number,
                ),
            }
        }
            // Because `write!` to a `String` is infallible, this `unwrap` is fine.
            .unwrap();
    }

    result
}

 */

