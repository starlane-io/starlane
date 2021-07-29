use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{Path, ResourcePathSegment};
use crate::error::Error;

use std::ops::Deref;
use std::convert::TryInto;
use std::ops::DerefMut;
use bincode::deserialize;

pub type Meta = MetaDeref<HashMap<String,String>>;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct MetaDeref<T>{
    map: T
}

impl<T> Deref for MetaDeref<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<T> DerefMut for MetaDeref<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.map
    }
}

impl Meta {
    pub fn new()->Self{
        Self{
            map: HashMap::new()
        }
    }

    pub fn single(key: &str, value: &str ) -> Self {
        let mut meta = Self::new();
        meta.insert(key.to_string(),value.to_string());
        meta
    }

    pub fn bin(&self) -> Result<Vec<u8>,Error>{
        Ok(bincode::serialize(self)?)
    }

    pub fn from_bin<'a>(data: &'a [u8]) -> Result<Self,Error>{
        Ok(bincode::deserialize::<Self>(data)?)
    }
}

impl TryInto<Vec<u8>> for Meta {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.bin()?)
    }
}