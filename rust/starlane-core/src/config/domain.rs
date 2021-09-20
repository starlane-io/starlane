use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactRef;
use crate::cache::{Cacheable, Data};
use crate::error::Error;
use crate::resource::config::{Parser, ResourceConfig};
use crate::resource::ArtifactKind;
use crate::resource::{DomainKey, ResourceAddress, ResourceKind};
use starlane_resources::ResourcePath;

pub struct Domain {
    key: DomainKey,
    address: ResourceAddress,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DomainState {}

impl DomainState {
    pub fn new() -> Self {
        DomainState {}
    }
}

impl TryInto<Vec<u8>> for DomainState {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(bincode::serialize(&self)?)
    }
}

impl TryInto<Arc<Vec<u8>>> for DomainState {
    type Error = Error;

    fn try_into(self) -> Result<Arc<Vec<u8>>, Self::Error> {
        Ok(Arc::new(bincode::serialize(&self)?))
    }
}

impl TryFrom<Arc<Vec<u8>>> for DomainState {
    type Error = Error;

    fn try_from(value: Arc<Vec<u8>>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<DomainState>(value.as_slice())?)
    }
}

impl TryFrom<Vec<u8>> for DomainState {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize::<DomainState>(value.as_slice())?)
    }
}

pub struct HttpResourceSelector {}

pub struct DomainConfig {
    artifact: ResourcePath,
    routes: HashMap<String, HttpResourceSelector>,
}

impl Cacheable for DomainConfig {
    fn artifact(&self) -> ArtifactRef {
        ArtifactRef {
            path: self.artifact.clone(),
            kind: ArtifactKind::DomainConfig,
        }
    }

    fn references(&self) -> Vec<ArtifactRef> {
        vec![]
    }
}

impl ResourceConfig for DomainConfigParser {
    fn kind(&self) -> ResourceKind {
        ResourceKind::Domain
    }
}

pub struct DomainConfigParser;

impl DomainConfigParser {
    pub fn new() -> Self {
        Self {}
    }
}

impl Parser<DomainConfig> for DomainConfigParser {
    fn parse(&self, artifact: ArtifactRef, _data: Data) -> Result<Arc<DomainConfig>, Error> {
        Ok(Arc::new(DomainConfig {
            artifact: artifact.path,
            routes: HashMap::new(),
        }))
    }
}

#[cfg(test)]
mod test {
    use nom::bytes::complete::{take_while, take_while1};
    use nom::character::complete::{alpha0, alphanumeric0, digit0, newline, one_of, space0};
    use nom::character::is_alphanumeric;
    use nom::combinator::{opt, recognize};
    use nom::error::{Error, ErrorKind};
    use nom::sequence::{pair, preceded};
    use nom::{branch::alt, bytes::complete::tag, Err, IResult};

    fn is_path_char(c: char) -> bool {
        '-' == c || c.is_alphanumeric() || '_' == c || '/' == c
    }

    fn path_pattern(input: &str) -> IResult<&str, &str> {
        recognize(pair(tag("/"), take_while1(is_path_char)))(input)
    }

    fn path_assignment(input: &str) -> IResult<&str, &str> {
        recognize(pair(tag("/"), take_while1(is_path_char)))(input)
    }

    fn line(input: &str) -> IResult<&str, &str> {
        recognize(newline)(input)
    }

    #[test]
    pub fn test2() {
        match line(
            r"/proxy/ => starlane-core.io:default:[some-proxy]::<Proxy>
/filesystem/ => starlane-core.io:default:*:[some-filesystem-tag]::<FileSystem>

        ",
        ) {
            Ok((input, path)) => {
                println!("path: {}", path);
                println!("..input: {}", input);
            }
            Err(error) => match error {
                Err::Incomplete(incomple) => {
                    println!("error: INCOMPLETE!");
                }
                Err::Error(error) => {
                    println!("error: ERROR! {}", error.code.description());
                }
                Err::Failure(failure) => {
                    println!("error: FAILURE!");
                }
            },
        }
    }
}
