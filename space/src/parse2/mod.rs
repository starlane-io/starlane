use std::mem::offset_of;
use std::ops::Deref;
use anyhow::__private::kind::TraitKind;
use itertools::Itertools;
use nom::bytes::complete::tag;
use nom::character::complete::alpha1;
use nom::{ErrorConvert, Finish, IResult, Offset};
use nom::combinator::all_consuming;
use nom::error::{context, convert_error, VerboseError, VerboseErrorKind};
use nom::multi::separated_list1;
use nom::sequence::pair;
use nom_locate::LocatedSpan;
use nom_supreme::ParserExt;
use thiserror::__private::AsDynError;
use crate::parse::{NomErr};
use crate::parse::util::{preceded, Span};

type Input<'a> = LocatedSpan<&'a str>;

pub type Res<'a,O> = IResult<Input<'a>, O,VerboseError<Input<'a>>>;


pub fn segments(i: Input) -> Res<Vec<Input>> {
   pair(context("segments",separated_list1(tag(":"),alpha1)),preceded(context("zoinks!",tag("^")),alpha1))(i).map(|(next,(segments,_))|(next,segments))
}

pub fn convert(e: VerboseError<Input>) -> VerboseError<&str> {
   let errors = e.errors.iter().map(|(input,kind)|(*input.fragment(),kind.clone())).collect();
    VerboseError { errors }
}

#[test]
fn test() {
    let input = Input::new("you:are:^awesome");
    let error = all_consuming(segments)(input.clone()).finish().unwrap_err();
    let msg = convert_error(*input.fragment(), convert(error));
    println!("{}", msg );
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

