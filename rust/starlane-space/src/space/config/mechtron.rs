use crate::space::parse::mechtron_config;
use crate::space::parse::model::MechtronScope;
use crate::space::point::Point;
use crate::SpaceErr;
use core::str::FromStr;
use serde::{Deserialize, Serialize};
use crate::space::err::ParseErrs;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct MechtronConfig {
    pub wasm: Point,
    pub name: String,
}

impl MechtronConfig {
    pub fn new(scopes: Vec<MechtronScope>) -> Result<Self, SpaceErr> {
        let mut wasm = None;
        let mut name = None;
        for scope in scopes {
            match scope {
                MechtronScope::WasmScope(assigns) => {
                    for assign in assigns {
                        if assign.key.as_str() == "bin" {
                            wasm.replace(Point::from_str(assign.value.as_str())?);
                        } else if assign.key.as_str() == "name" {
                            name.replace(assign.value);
                        }
                    }
                }
            }
        }
        if wasm.is_some() && name.is_some() {
            Ok(Self {
                wasm: wasm.unwrap(),
                name: name.unwrap(),
            })
        } else {
            Err("required `bin` and `name` in Wasm scope".into())
        }
    }
}

impl FromStr for MechtronConfig {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        mechtron_config(s)
    }
}

impl TryFrom<Vec<u8>> for MechtronConfig {
    type Error = ParseErrs;

    fn try_from(doc: Vec<u8>) -> Result<Self, Self::Error> {
        let doc = String::from_utf8(doc)?;
        mechtron_config(doc.as_str())
    }
}
