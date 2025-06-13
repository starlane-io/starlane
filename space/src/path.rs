use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::parse::util::new_span;

use crate::err::ParseErrs;
use crate::parse::consume_path;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct Path {
    string: String,
}

impl Path {
    fn new(string: &str) -> Self {
        Path {
            string: string.to_string(),
        }
    }

    pub fn make_absolute(string: &str) -> Result<Self, ParseErrs> {
        if string.starts_with("/") {
            Path::from_str(string)
        } else {
            Path::from_str(format!("/{}", string).as_str())
        }
    }

    pub fn bin(&self) -> Result<Vec<u8>, ParseErrs> {
        let bin = bincode::serialize(self).map_err(ParseErrs::from)?;
        Ok(bin)
    }

    pub fn is_absolute(&self) -> bool {
        self.string.starts_with("/")
    }

    pub fn cat(&self, path: &Path) -> Result<Self, ParseErrs> {
        if self.string.ends_with("/") {
            Path::from_str(format!("{}{}", self.string.as_str(), path.string.as_str()).as_str())
        } else {
            Path::from_str(format!("{}/{}", self.string.as_str(), path.string.as_str()).as_str())
        }
    }

    pub fn parent(&self) -> Option<Path> {
        let s = self.to_string();
        let parent = std::path::Path::new(s.as_str()).parent();
        match parent {
            None => Option::None,
            Some(path) => match path.to_str() {
                None => Option::None,
                Some(some) => match Self::from_str(some) {
                    Ok(parent) => Option::Some(parent),
                    Err(error) => Option::None,
                },
            },
        }
    }

    pub fn last_segment(&self) -> Option<String> {
        let split = self.string.split("/");
        match split.last() {
            None => Option::None,
            Some(last) => Option::Some(last.to_string()),
        }
    }

    pub fn to_relative(&self) -> String {
        let mut rtn = self.string.clone();
        rtn.remove(0);
        rtn
    }
}

impl FromStr for Path {
    type Err = ParseErrs;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (_, path) = consume_path(new_span(s))?;
        Ok(Self {
            string: path.to_string(),
        })
    }
}

impl ToString for Path {
    fn to_string(&self) -> String {
        self.string.clone()
    }
}
