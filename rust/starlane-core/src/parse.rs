use crate::star::StarKind;
use nom::sequence::{delimited, terminated};
use nom::error::{context, ErrorKind};
use nom::character::complete::alpha1;
use starlane_resources::Res;
use nom::bytes::complete::{tag, take};
use crate::error::Error;
use crate::frame::StarPattern;
use std::str::FromStr;
use starlane_resources::parse::parse_domain;
use crate::resource::DomainCase;
use nom::{InputTakeAtPosition, AsChar};

pub fn parse_star_kind(input: &str) -> Res<&str, Result<StarKind,Error>> {
    context(

        "star_kind",
        delimited(tag("<"),alpha1,tag(">"))
    )(input)
        .map(|(input_next,kind)| {
            match StarKind::from_str(kind) {
                Ok(kind) => {

                    (input_next,Ok(kind))
                }
                Err(error) => {
                    (input_next,Err(error.into()))
                }

            }
        })
}


pub fn parse_star_pattern(input: &str) -> Res<&str, Result<StarPattern,Error>> {
    context(

        "star_pattern",
        parse_star_kind
    )(input)
        .map(|(input_next,kind)| {
            match kind {
                Ok(kind) => {
                    (input_next,Ok(StarPattern::StarKind(kind)))
                }
                Err(error) => {
                    (input_next,Err(error.into()))
                }
            }
        })
}

fn alpha1_hyphen<T>(i: T) -> Res<T, T>
    where
        T: InputTakeAtPosition,
        <T as InputTakeAtPosition>::Item: AsChar,
{
    i.split_at_position1_complete(
        |item| {
            let char_item = item.as_char();
            !(char_item == '-')
                && !(char_item.is_alpha() || char_item.is_dec_digit())
        },
        ErrorKind::AlphaNumeric,
    )
}


pub fn parse_host( input: &str ) -> Res<&str, &str> {
   context(
       "parse_host",
       terminated( alpha1_hyphen, tag(":") )
   )(input)
}
