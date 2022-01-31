use mesh_portal_versions::version::v0_0_1::parse::{create, publish, Res, select};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{multispace0, space1};
use nom::sequence::tuple;
use crate::command::compose::ProtoCommand;

fn create_command(input: &str) -> Res<&str, ProtoCommand> {
    tuple((tag("create"),space1,create))(input).map( |(next,(_,_,create))|{
        (next, ProtoCommand::Create(create))
    })
}

fn publish_command(input: &str) -> Res<&str, ProtoCommand> {
    tuple((tag("publish"),space1,publish))(input).map( |(next,(_,_,create))|{
        (next, ProtoCommand::Publish(create))
    })
}

fn select_command(input: &str) -> Res<&str, ProtoCommand> {
    tuple((tag("select"),space1,select))(input).map( |(next,(_,_,select))|{
        (next, ProtoCommand::Select(select))
    })
}

pub fn command(input: &str) -> Res<&str, ProtoCommand> {
    alt( (create_command) )(input)
}

pub fn command_line(input: &str) -> Res<&str, ProtoCommand> {
    tuple( (multispace0,command,multispace0))(input).map(|(next,(_,command,_))|{
        (next,command)
    })
}