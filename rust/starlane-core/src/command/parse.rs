use mesh_portal_versions::version::v0_0_1::entity::request::set::Set;
use mesh_portal_versions::version::v0_0_1::parse::{create, get, publish, Res, select, set};
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{multispace0, space1};
use nom::combinator::{all_consuming, fail, opt, recognize};
use nom::error::context;
use nom::multi::many0;
use nom::sequence::{terminated, tuple};
use crate::command::compose::{CommandOp, Strategy};

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

pub fn command_strategy(input: &str) -> Res<&str, Strategy> {
    opt( tuple((tag("?"),multispace0)) )(input).map( |(next,hint)| {
        match hint {
            None => (next, Strategy::Commit),
            Some(_) => (next, Strategy::Ensure)
        }
    } )

}

pub fn command(input: &str) -> Res<&str, CommandOp> {
    context("command", alt( (create_command, publish_command, select_command, set_command, get_command,fail) ))(input)
}

pub fn command_mutation(input: &str) -> Res<&str, CommandOp> {
    context("command_mutation", tuple((command_strategy, command)))(input).map( |(next,(strategy,mut command)),| {
        command.set_strategy(strategy);
        (next, command)
    })
}

pub fn command_line(input: &str) -> Res<&str, CommandOp> {
    tuple( (multispace0,command_mutation,multispace0,opt(tag(";")),multispace0))(input).map(|(next,(_,command,_,_,_))|{
        (next,command)
    })
}

pub fn script_line(input: &str) -> Res<&str, CommandOp> {
    tuple( (multispace0,command_mutation,multispace0,tag(";"),multispace0))(input).map(|(next,(_,command,_,_,_))|{
        (next,command)
    })
}

pub fn script(input: &str) -> Res<&str,Vec<CommandOp>> {
    many0(script_line)(input)
}

pub fn consume_command_line(input: &str) -> Res<&str, CommandOp> {
    all_consuming(command_line)(input)
}

pub fn rec_script_line(input: &str) -> Res<&str, &str> {
    recognize(script_line)(input)
}

#[cfg(test)]
pub mod test {
    use mesh_portal_versions::version::v0_0_1::parse::Res;
    use nom::error::{VerboseError, VerboseErrorKind};
    use nom_supreme::final_parser::{ExtractContext, final_parser};
    use crate::command::compose::CommandOp;
    use crate::command::parse::{command, command_mutation, script};
    use crate::error::Error;

    /*
    #[test]
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

    #[test]
    pub async fn test() -> Result<(),Error>{
        let input = "? xreate localhost<Space>";
        match command_mutation(input) {
            Ok(_) => {}
            Err(nom::Err::Error(e)) => {
                eprintln!("{}",e.to_string());
                return Err("could not find context".into());
            }
            Err(e) => {
                return Err("some err".into());
            }
        }
        Ok(())
    }

    #[test]
    pub async fn test_kind() -> Result<(),Error>{
        let input = "create localhost:users<UserBase<Keycloak>>";
        let (_, command) = command(input)?;
        match command {
            CommandOp::Create(create) => {
                assert_eq!(create.template.kind.sub_kind, Some("Keycloak".to_string()));
            }
            _ => {
                panic!("expected create command")
            }
        }
        Ok(())
    }


    #[test]
    pub async fn test_script() -> Result<(),Error>{
        let input = r#" ? create localhost<Space>;
 Xcrete localhost:repo<Base<Repo>>;
? create localhost:repo:tutorial<ArtifactBundleSeries>;
? publish ^[ bundle.zip ]-> localhost:repo:tutorial:1.0.0;
set localhost{ +bind=localhost:repo:tutorial:1.0.0:/bind/localhost.bind };
        "#;

        script(input)?;
        Ok(())
    }


}