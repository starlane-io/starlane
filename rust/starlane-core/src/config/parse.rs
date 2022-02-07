use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
use mesh_portal_serde::version::latest::command::common::SetProperties;
use mesh_portal_versions::version::v0_0_1::parse::{camel_case, domain, Res, set_properties};
use mesh_portal_versions::version::v0_0_1::pattern::parse::kind;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_until};
use nom::character::complete::multispace0;
use nom::combinator::all_consuming;
use nom::multi::{many0, separated_list0};
use nom::sequence::{delimited, preceded, tuple};
use crate::artifact::ArtifactRef;
use crate::cache::Data;
use crate::command::compose::{Command, CommandOp};
use crate::command::parse::{script, script_line};
use crate::config::config::ResourceConfig;
use crate::error::Error;
use crate::resource::config::Parser;
use crate::resource::Kind;


pub struct ResourceConfigParser;

impl ResourceConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<ResourceConfig> for ResourceConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<ResourceConfig>, Error> {
        let raw = String::from_utf8(_data.to_vec() )?;
        let config = resource_config(raw.as_str(), artifact)?;
        Ok(Arc::new(config))
    }
}


pub fn resource_config(input: &str, artifact_ref: ArtifactRef  ) -> Result<ResourceConfig,Error> {
    let (next,(_,(kind,(_,sections)),_)) = all_consuming(tuple( (multispace0, tuple((kind, tuple((multispace0,delimited(tag("{"),sections, tag("}")))))),multispace0)) )(input)?;

    let kind: Kind = TryFrom::try_from(kind)?;

    let mut config = ResourceConfig {
        artifact_ref,
        kind,
        properties: SetProperties::new(),
        install: vec![]
    };

    for section in sections {
        match section {
            Section::SetProperties(properties) => {config.properties = properties}
            Section::Install(ops) => {
               config.install=ops}
        }
    }

    Ok(config)
}

fn sections( input: &str ) -> Res<&str,Vec<Section>> {
    many0(section)(input)
}

fn section( input: &str) -> Res<&str,Section> {
    alt( (properties_section,install_section) )(input)
}

fn properties_section( input: &str) -> Res<&str,Section> {
    tuple( (multispace0, preceded(tag("Set"), tuple((multispace0,delimited(tag("{"),set_properties, tag("}"))))),multispace0) )(input).map( |(next,(_,(_,properties),_))| {
        (next, Section::SetProperties(properties))
    })
}

fn rec_command_lines( input: &str ) -> Res<&str,Vec<String>> {
    separated_list0(tag(";"), take_until(";") )(input).map( |(next,lines) | {
        let lines : Vec<String> = lines.into_iter().map(|line| line.to_string() ).collect();
        (next,lines)
    })
}

fn install_section( input: &str) -> Res<&str,Section> {
   let (next,(_,(_,ops),_)) = tuple( (multispace0, preceded(tag("Install"), tuple((multispace0,delimited(tag("{"),rec_command_lines, tag("}"))))),multispace0) )(input)?;

    Ok((next,Section::Install(ops)))
}

pub enum Section {
    SetProperties(SetProperties),
    Install(Vec<String>)
}



pub mod replace {
    use std::collections::HashMap;
    use mesh_portal_versions::version::v0_0_1::parse::{domain, Res};
    use nom::branch::alt;
    use nom::bytes::complete::{tag, take_until};
    use nom::character::complete::anychar;
    use nom::combinator::{opt, recognize};
    use nom::multi::many1;
    use nom::sequence::delimited;
    use crate::error::Error;

    fn config_chunk(input: &str) -> Res<&str,&str> {
        alt(( take_until("$("),recognize(many1(anychar))))(input)
    }

    fn replace_token(input: &str) -> Res<&str,&str> {
        delimited(tag("$("), domain ,tag(")") )(input)
    }

    pub fn substitute(input:&str, map: &HashMap<String,String>) -> Result<String,Error> {
        let mut rtn = String::new();
        let mut next = input;
        let mut chunk = Option::None;
        while !next.is_empty() {
            (next,chunk) = opt(config_chunk)(next)?;
            if let Some(chunk) = chunk {
                rtn.push_str(chunk);
            }

            (next,chunk) = opt(replace_token)(next)?;
            if let Some(chunk) = chunk {
                let replacement = map.get(&chunk.to_string()).ok_or(format!("could not find substitution for '{}'", chunk))?;
                rtn.push_str(replacement.as_str());
            }
    }
        Ok(rtn)
    }


}


pub mod test {
    use std::collections::HashMap;
    use std::str::FromStr;
    use mesh_portal_serde::version::latest::id::Address;
    use crate::artifact::ArtifactRef;
    use crate::config::parse::{resource_config, properties_section};
    use crate::config::parse::replace::substitute;
    use crate::error::Error;
    use crate::resource::ArtifactKind;

    #[test]
    pub fn test_replace() -> Result<(),Error>{
        let config_src = r#"App {

  Set {
    +wasm.src=$(self.config.bundle):/wasm/my-app.wasm,
    +wasm.name=my-app,
    +bind=$(self.config.bundle):/bind/app.bind
  }

  Install {
    create $(self):users<Base<User>>;
    create $(self):files<FileSystem>;
  }

}"#;
        let mut map = HashMap::new();
        map.insert( "self".to_string(), "localhost:app".to_string());
        map.insert( "self.config.bundle".to_string(), "localhost:repo:site:1.0.0".to_string());

        let rtn = substitute(config_src, map )?;

        println!("{}",rtn);

        let artifact_ref = ArtifactRef {
            address: Address::from_str("localhost:app")?,
            kind: ArtifactKind::ResourceConfig
        };
        let config = resource_config(rtn.as_str(), artifact_ref )?;

        Ok(())
    }


    #[test]
    pub fn test_set() -> Result<(),Error>{
        let config_src = r#"

  Set {
    +wasm.src=localhost:files:/wasm/my-app.wasm
    +wasm.name=my-app
    +bind=localhost:files:/bind/app.bind
  }

"#;

        let (_,section) = properties_section(config_src)?;

        Ok(())
    }

}