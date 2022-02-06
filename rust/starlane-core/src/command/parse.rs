use mesh_portal_versions::version::v0_0_1::entity::request::set::Set;
use mesh_portal_versions::version::v0_0_1::parse::{create, get, publish, Res, select, set};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{multispace0, space1};
use nom::combinator::{all_consuming, opt, recognize};
use nom::sequence::{terminated, tuple};
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

fn set_command(input: &str) -> Res<&str, CommandOp> {
    tuple((tag("set"),space1,set))(input).map( |(next,(_,_,set))|{
        (next, CommandOp::Set(set))
    })
}

fn get_command(input: &str) -> Res<&str, CommandOp> {
    tuple((tag("get"),space1,get))(input).map( |(next,(_,_,get))|{
        (next, CommandOp::Get(get))
    })
}


pub fn command(input: &str) -> Res<&str, CommandOp> {
    alt( (create_command,publish_command,select_command,set_command,get_command) )(input)
}

pub fn command_line(input: &str) -> Res<&str, CommandOp> {
    tuple( (multispace0,command,multispace0,opt(tag(";")),multispace0))(input).map(|(next,(_,command,_,_,_))|{
        (next,command)
    })
}

pub fn script_line(input: &str) -> Res<&str, CommandOp> {
    tuple( (multispace0,command,multispace0,tag(";"),multispace0))(input).map(|(next,(_,command,_,_,_))|{
        (next,command)
    })
}

pub fn consume_command_line(input: &str) -> Res<&str, CommandOp> {
    all_consuming(command_line)(input)
}

pub fn rec_script_line(input: &str) -> Res<&str, &str> {
    recognize(script_line)(input)
}