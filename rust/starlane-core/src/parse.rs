use crate::star::StarKind;
use nom::sequence::delimited;
use nom::error::context;
use nom::character::complete::alpha1;
use starlane_resources::Res;
use nom::bytes::complete::{tag, take};
use crate::error::Error;
use crate::frame::StarPattern;
use std::str::FromStr;

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