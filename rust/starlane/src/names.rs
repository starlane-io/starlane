use std::fmt;
use std::str::{Split, FromStr};

use crate::actor::ActorKind;
use crate::actor::ActorSpecific;
use crate::app::AppSpecific;
use crate::artifact::{Artifact, SubSpaceName};
use crate::artifact::ArtifactLocation;
use crate::artifact::ArtifactKind;
use crate::error::Error;

use serde::{Deserialize, Serialize, Serializer};

lazy_static!
{
    pub static ref TEST_APP_SPEC: AppSpecific = AppSpecific::from("starlane.io:starlane:core:/test/test_app").unwrap();
    pub static ref TEST_ACTOR_SPEC: ActorSpecific = ActorSpecific::from("starlane.io:starlane:core:/test/test_actor").unwrap();

    pub static ref TEST_APP_CONFIG_ARTIFACT: Artifact = Artifact
    {
                    location: ArtifactLocation::from_str("starlane.io:starlane:core:test:1.0.0:/test/test_app.yaml").unwrap(),
                    kind: ArtifactKind::AppConfig,
                    specific: Option::Some(AppSpecific::from_str("starlane.io:starlane:core:test:/test/test_app").unwrap())
    };

    pub static ref TEST_ACTOR_CONFIG_ARTIFACT: Artifact = Artifact
    {
                    location: ArtifactLocation::from_str("starlane.io:starlane:core:test:1.0.0:/test/test_actor.yaml").unwrap(),
                    kind: ArtifactKind::ActorConfig,
                    specific:Option::Some(ActorSpecific::from_str("starlane.io:starlane:core:test:/test/test_actor").unwrap())
    };
}





#[derive(Clone,Eq,PartialEq,Hash,Serialize,Deserialize)]
pub struct Name
{
    pub sub_space: SubSpaceName,
    pub path: String,
}


impl Name
{
    pub fn more(string: &str) -> Result<(Self,Split<&str>),Error>
    {
        let (sub_space,mut parts) = SubSpaceName::more(string)?;

        Ok((Name
            {
                sub_space: sub_space,
                path: parts.next().ok_or("path")?.to_string(),
            },parts))
    }

    pub fn from(string: &str) -> Result<Self,Error>
    {
        let (name,_) = Name::more(string)?;
        Ok(name)
    }

    pub fn to(&self) -> String {
        let mut rtn= String::new();
        rtn.push_str(self.sub_space.to().as_str()); rtn.push_str(":");
        rtn.push_str(self.path.as_str());
        return rtn;
    }

    pub fn as_name(&self)->Self
    {
        self.clone()
    }
}


impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}", self.to()) }
}

impl FromStr for Name
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name,_) = Name::more(s)?;
        Ok(name)
    }
}

pub type Specific=Name;
