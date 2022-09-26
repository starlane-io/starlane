use core::str::FromStr;
use serde::de::Unexpected::Option;
use crate::parse::model::MechtronScope;
use crate::loc::Point;
use crate::{Bin, UniErr};
use crate::parse::mechtron_config;

#[derive(Clone)]
pub struct MechtronConfig {
    pub bin: Point,
    pub name: String
}

impl MechtronConfig {
    pub fn new( scopes: Vec<MechtronScope>) -> Result<Self, UniErr> {
        let mut bin  = None;
        let mut name = None;
        for scope in scopes {
            match scope {
                MechtronScope::WasmScope(assigns) => {
                    for assign in assigns {
                        if assign.key.as_str() ==  "bin" {
                            bin.replace(Point::from_str(assign.value.as_str())?);
                        } else if assign.key.as_str() == "name" {
                            name.replace(assign.value);
                        }
                    }

                }
            }
        }
        if bin.is_some() && name.is_some() {
            Ok(Self {
                bin: bin.unwrap(),
                name: name.unwrap()
            })
        } else {
            Err("required `bin` and `name` in Wasm scope".into())
        }
    }
}

impl TryFrom<Vec<u8>> for MechtronConfig {
    type Error = UniErr;

    fn try_from(doc: Vec<u8>) -> Result<Self, Self::Error> {
        let doc = String::from_utf8(doc)?;
        mechtron_config(doc.as_str())
    }
}

impl TryFrom<Bin> for MechtronConfig {
    type Error = UniErr;

    fn try_from(doc: Bin) -> Result<Self, Self::Error> {
        let doc = String::from_utf8((*doc).clone() )?;
        mechtron_config(doc.as_str())
    }
}
