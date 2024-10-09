use core::str::FromStr;
use std::ops::Deref;

use serde::{Deserialize, Serialize};
use starlane_primitive_macros::Autobox;
use crate::space::config::mechtron::MechtronConfig;
use crate::space::particle::{Details, Stub};
use crate::space::point::Point;
use crate::BindConfig;

pub mod bind;
pub mod mechtron;

use crate::space::err::{ParseErrs, SpaceErr};
use crate::space::parse::doc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PortalKind {
    Mechtron,
    Portal,
}

impl ToString for PortalKind {
    fn to_string(&self) -> String {
        match self {
            PortalKind::Mechtron => "Mechtron".to_string(),
            PortalKind::Portal => "Portal".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub stub: Stub,
    pub kind: PortalKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalConfig {
    pub max_payload_size: u32,
    pub init_timeout: u64,
    pub frame_timeout: u64,
    pub response_timeout: u64,
}

impl Default for PortalConfig {
    fn default() -> Self {
        Self {
            max_payload_size: 128 * 1024,
            init_timeout: 30,
            frame_timeout: 5,
            response_timeout: 15,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PointConfig<Body> {
    pub point: Point,
    pub body: Body,
}

impl<Body> Deref for PointConfig<Body> {
    type Target = Body;

    fn deref(&self) -> &Self::Target {
        &self.body
    }
}

#[derive(Autobox)]
pub enum Document {
    BindConfig(BindConfig),
    MechtronConfig(MechtronConfig),
}

impl Document {
   pub fn kind(&self) -> DocKind {
       match self {
           Document::BindConfig(_) => DocKind::BindConfig,
           Document::MechtronConfig(_) => DocKind::MechtronConfig,
       }
   }
}



impl FromStr for Document {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        doc(s)
    }
}

#[derive(Clone,Hash,Eq,PartialEq,strum_macros::Display,strum_macros::EnumString)]
pub enum DocKind {
    BindConfig,
    MechtronConfig
}

impl AsRef<str> for DocKind {
    fn as_ref(&self) -> &str {
        self.as_ref()
    }
}



#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ParticleConfigBody {
    pub details: Details,
}
