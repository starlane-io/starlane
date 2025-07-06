use crate::parse::util::recognize;
use crate::parse::{CamelCase, SkewerCase, SnakeCase};
use crate::parse2::chars::recognize::{
    lower1, lower_alphanumeric_plus_dash0, lower_alphanumeric_plus_underscore0, upper1,
};
use crate::parse2::token::Ident;
use crate::parse2::{Ctx, Input, Res};
use nom::branch::alt;
use nom::character::complete::alphanumeric0;
use nom::character::streaming::alphanumeric1;
use nom::combinator::{into, value};
use nom::multi::many1;
use nom::sequence::pair;

pub fn camel(input: Input) -> Res<CamelCase> {
    recognize::camel(input).map(|(next, rtn)| (next, CamelCase::new(rtn.to_string())))
}

pub fn skewer(input: Input) -> Res<SkewerCase> {
    recognize::skewer(input).map(|(next, rtn)| (next, SkewerCase::new(rtn.to_string())))
}

pub fn snake(input: Input) -> Res<SnakeCase> {
    recognize::camel(input).map(|(next, rtn)| (next, SnakeCase::new(rtn.to_string())))
}

fn undefined(input: Input) -> Res<Input> {
    use recognize::*;
    recognize(many1(alt((alphanumeric1, dash, underscore))))(input)
}

pub fn ident(input: Input) -> Res<Ident> {
    alt((into(camel), into(skewer), into(snake)))(input)
}

mod recognize {
    use crate::parse::util::recognize;
    use crate::parse2::{Ctx, Input, Res};
    use nom::branch::alt;
    use nom::bytes::complete::{is_a, tag};
    use nom::character::complete::{alpha1, alphanumeric0, alphanumeric1};
    use nom::combinator::opt;
    use nom::sequence::pair;
    use nom_supreme::ParserExt;

    pub fn dash(input: Input) -> Res<Input> {
        recognize(tag("-"))(input)
    }

    pub fn newline(input: Input) -> Res<Input> {
        recognize(tag("\n"))(input)
    }

    pub fn underscore(input: Input) -> Res<Input> {
        recognize(tag("_"))(input)
    }

    pub fn upper1(input: Input) -> Res<Input> {
        recognize(is_a("ABCDEFGHIJKLMNOPQRSTUVWXYZ"))(input)
    }

    pub fn lower1(input: Input) -> Res<Input> {
        recognize(is_a("abcdefghijklmnopqrstuvwxyz"))(input)
    }

    pub fn lower_alphanumeric_plus_dash0(input: Input) -> Res<Input> {
        recognize(opt(alt((lower1, tag("-")))))(input)
    }

    pub fn lower_alphanumeric_plus_dash1(input: Input) -> Res<Input> {
        recognize(alt((lower1, tag("-"))))(input)
    }

    pub fn lower_alphanumeric_plus_underscore0(input: Input) -> Res<Input> {
        recognize(opt(alt((lower1, tag("_")))))(input)
    }

    pub fn lower_alphanumeric_plus_underscore1(input: Input) -> Res<Input> {
        recognize(alt((lower1, tag("_"))))(input)
    }

    pub fn camel(input: Input) -> Res<Input> {
        recognize(pair(upper1, alphanumeric0).context(Ctx::CamelCase))(input)
    }

    pub fn skewer(input: Input) -> Res<Input> {
        recognize(pair(lower1, lower_alphanumeric_plus_dash0).context(Ctx::SkewerCase))(input)
    }

    pub fn snake(input: Input) -> Res<Input> {
        recognize(pair(lower1, lower_alphanumeric_plus_underscore0).context(Ctx::SkewerCase))(input)
    }
}
