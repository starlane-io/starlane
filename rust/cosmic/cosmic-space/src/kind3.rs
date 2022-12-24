use core::str::FromStr;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use cosmic_nom::new_span;
use crate::err::SpaceErr;
use crate::kind3::parse::kind;
use crate::kind::Specific;
use crate::parse::CamelCase;
use crate::particle::property::PropertiesConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash )]
pub struct Kind {
    pub segments: Vec<String>,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash )]
pub struct ExcactKind {
    pub kind: Kind,
    pub specific: Specific
}


impl ToString for Kind {
    fn to_string(&self) -> String {
        let mut s = String::new();
        for seg in self.segments {
            s.push_str(seg.as_str());
            s.push_str(":");
        }
        s.push_str(self.name.as_str());
        s
    }
}

impl FromStr for Kind {
    type Err = SpaceErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
       kind(new_span(s))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash )]
pub struct KindConfig {
    pub properties: PropertiesConfig,
    pub specific: Specific
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash )]
pub struct KindDefs {
    map: HashMap<Kind, KindConfig>
}

pub trait KindDefsFactory {
    fn create(&self) -> KindDefs;
}

pub enum DriverKind {
    BuiltIn,
    Wasi(String)
}

pub mod parse {
    use nom::bytes::complete::tag;
    use nom::multi::many1;
    use nom::sequence::{pair, terminated};
    use cosmic_nom::Span;
    use crate::err::SpaceErr;
    use crate::kind3::Kind;
    use crate::parse::{camel_case, skewer};
    use crate::parse::error::result;

    pub fn kind<I>( i: I ) -> Result<Kind,SpaceErr> where I: Span{
        result(pair(many1(terminated(skewer, tag(":"))),camel_case)(i).map(|(next,(segments,name))|{
            let segments : Vec<String> = segments.into_iter().map(|i|i.to_string()).collect();
            let name = name.to_string();

            (next,Kind {
                segments,
                name
            })
        }))
    }
}


