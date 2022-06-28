use crate::artifact::ArtifactRef;
use crate::config::config::ParticleConfig;
use crate::error::Error;
use crate::particle::config::Parser;
use mesh_portal::version::latest::bin::Bin;
use mesh_portal::version::latest::command::common::SetProperties;
use mesh_portal_versions::version::v0_0_1::parse::{camel_case_chars, domain, kind, script, script_line, set_properties};
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_until};
use nom::character::complete::multispace0;
use nom::combinator::{all_consuming, recognize};
use nom::error::context;
use nom::multi::{many0, separated_list0};
use nom::sequence::{delimited, preceded, terminated, tuple};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;
use cosmic_nom::{new_span, Res, Span};
use mesh_portal_versions::version::v0_0_1::id::id::Kind;

pub struct ParticleConfigParser;

impl ParticleConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<ParticleConfig> for ParticleConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Bin) -> Result<Arc<ParticleConfig>, Error> {
        let raw = String::from_utf8(_data.to_vec())?;
        let config = particle_config(new_span(raw.as_str()), artifact)?;
        Ok(Arc::new(config))
    }
}

pub fn particle_config<I: Span>(
    input: I,
    artifact_ref: ArtifactRef,
) -> Result<ParticleConfig, Error> {
    let (next, (_, (kind, (_, sections)), _)) = all_consuming(tuple((
        multispace0,
        tuple((
            kind,
            tuple((multispace0, delimited(tag("{"), sections, tag("}")))),
        )),
        multispace0,
    )))(input)?;


    let mut config = ParticleConfig {
        artifact_ref,
        kind,
        properties: SetProperties::new(),
        install: vec![],
    };

    for section in sections {
        match section {
            Section::SetProperties(properties) => config.properties = properties,
            Section::Install(ops) => config.install = ops,
        }
    }

    Ok(config)
}

fn sections<I: Span>(input: I) -> Res<I, Vec<Section>> {
    many0(section)(input)
}

fn section<I: Span>(input: I) -> Res<I, Section> {
    alt((properties_section, install_section))(input)
}

fn properties_section<I: Span>(input: I) -> Res<I, Section> {
    tuple((
        multispace0,
        preceded(
            tag("Set"),
            tuple((multispace0, delimited(tag("{"), set_properties, tag("}")))),
        ),
        multispace0,
    ))(input)
    .map(|(next, (_, (_, properties), _))| (next, Section::SetProperties(properties)))
}

fn rec_command_line<I: Span>(input: I) -> Res<I, I> {
    terminated(tuple((multispace0, take_until(";"), multispace0)), tag(";"))(input)
        .map(|(next, (_, line, _))| (next, line))
}

fn rec_command_lines<I: Span>(input: I) -> Res<I, Vec<I>> {
    tuple((many0(rec_command_line), multispace0))(input).map(|(next, (lines, _))| (next, lines))
}

fn install_section<I: Span>(input: I) -> Res<I, Section> {
    let (next, (_, (_, ops), _)) = context(
        "Install Section",
        tuple((
            multispace0,
            preceded(
                tag("Install"),
                tuple((
                    multispace0,
                    delimited(tag("{"), rec_command_lines, tag("}")),
                )),
            ),
            multispace0,
        )),
    )(input)?;

    //    Ok((next,Section::Install(ops)))
    Ok((next, Section::Install(vec![])))
}

pub enum Section {
    SetProperties(SetProperties),
    Install(Vec<String>),
}

#[cfg(test)]
pub mod test {
    use crate::artifact::ArtifactRef;
    use crate::config::parse::{
        properties_section, rec_command_line, rec_command_lines, particle_config,
    };
    use crate::error::Error;
    use mesh_portal_versions::version::v0_0_1::id::ArtifactSubKind;
    use mesh_portal::version::latest::command::common::PropertyMod;
    use mesh_portal::version::latest::id::Point;
    use mesh_portal_versions::version::v0_0_1::parse::{
        property_mod, property_value, property_value_not_space_or_comma, set_properties,
    };
    use mesh_portal_versions::version::v0_0_1::span::new_span;
    use nom::combinator::{all_consuming, recognize};
    use std::collections::HashMap;
    use std::str::FromStr;
    use cosmic_nom::new_span;
    use cosmic_nom::Span;

    #[test]
    pub async fn test_set() -> Result<(), Error> {
        let config_src = r#"Set {
    +wasm.src=localhost:files:/wasm/my-app.wasm,
    +wasm.name=my-app,
    +bind=localhost:files:/bind/app.bind
  }"#;

        let (_, section) = properties_section(new_span(config_src))?;

        Ok(())
    }

    #[test]
    pub async fn test_set_properties() -> Result<(), Error> {
        let config_src = r#"
    +wasm.src=localhost:files:/wasm/my-app.wasm,
    +wasm.name=my-app,
    +bind=localhost:files:/bind/app.bind
  "#;

        let (_, section) = set_properties(new_span(config_src))?;

        Ok(())
    }

    #[test]
    pub async fn test_property_valu3() -> Result<(), Error> {
        let config_src = "+some=blah,";

        let (next, property) = property_mod(new_span(config_src))?;

        match property.clone() {
            PropertyMod::Set { key, value, lock } => {
                assert_eq!(key, "some".to_string());
                assert_eq!(value, "blah".to_string());
            }
            PropertyMod::UnSet(_) => {
                assert!(false);
            }
        }
        assert_eq!(next.to_string(), ",".to_string());

        Ok(())
    }

    #[test]
    pub async fn test_rec_command_line() -> Result<(), Error> {
        let (_, line) = all_consuming(rec_command_line)(new_span("create $(self):users<Base<User>>;"))?;
        let (_, line) =
            all_consuming(rec_command_line)(new_span("        create $(self):users<Base<User>>;"))?;

        Ok(())
    }

    #[test]
    pub async fn test_rec_command_lines() -> Result<(), Error> {
        let config_src = r#"

    create $(self):users<Base<User>>;
    create $(self):files<FileSystem>;

"#;

        let (_, section) = all_consuming(rec_command_lines)(new_span(config_src))?;

        assert_eq!(section.len(), 2);
        Ok(())
    }
}
