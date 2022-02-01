use mesh_portal_versions::version::v0_0_1::parse::{create, publish, Res, select};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{multispace0, space1};
use nom::combinator::all_consuming;
use nom::sequence::tuple;
use crate::command::compose::CommandOp;

fn create_command(input: &str) -> Res<&str, CommandOp> {
    tuple((tag("create"),space1,create))(input).map( |(next,(_,_,create))|{
        (next, CommandOp::Create(create))
    })
}

fn publish_command(input: &str) -> Res<&str, CommandOp> {
    tuple((tag("publish"),space1,publish))(input).map( |(next,(_,_,create))|{
        (next, CommandOp::Publish(create))
    })
}

fn select_command(input: &str) -> Res<&str, CommandOp> {
    tuple((tag("select"),space1,select))(input).map( |(next,(_,_,select))|{
        (next, CommandOp::Select(select))
    })
}

pub fn command(input: &str) -> Res<&str, CommandOp> {
    alt( (create_command,publish_command,select_command) )(input)
}

pub fn command_line(input: &str) -> Res<&str, CommandOp> {
    all_consuming(tuple( (multispace0,command,multispace0)))(input).map(|(next,(_,command,_))|{
        (next,command)
    })
}