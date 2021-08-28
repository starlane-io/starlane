use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::ops::Deref;
use std::ops::DerefMut;
use std::sync::Arc;

use bincode::deserialize;
use serde::{Deserialize, Serialize};

use crate::{Path, ResourcePathSegment};
use crate::error::Error;

pub type Meta = MetaDeref<HashMap<String, String>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaDeref<T> {
    map: T,
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
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn single(key: &str, value: &str) -> Self {
        let mut meta = Self::new();
        meta.insert(key.to_string(), value.to_string());
        meta
    }

    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        Ok(bincode::serialize(self)?)
    }

    pub fn from_bin<'a>(data: &'a [u8]) -> Result<Self, Error> {
        Ok(bincode::deserialize::<Self>(data)?)
    }
}

impl TryInto<Vec<u8>> for Meta {
    type Error = Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.bin()?)
    }
}

pub type Binary = Arc<Vec<u8>>;
pub type DataSet<B> = HashMap<String, B>;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BinSrc {
    Memory(Binary),
}

impl BinSrc {
    pub fn new(bin: Binary) -> Self {
        Self::Memory(bin)
    }
}

pub trait BinContext: Sync + Send {


}


impl BinSrc {

    pub fn to_bin(&self, ctx: Arc<dyn BinContext>) -> Result<Binary, Error> {
        match self {
            BinSrc::Memory(bin) => Ok(bin.clone()),
        }
    }
}

impl TryFrom<Meta> for BinSrc {
    type Error = Error;

    fn try_from(meta: Meta) -> Result<Self, Self::Error> {
        Ok(BinSrc::Memory(Arc::new(bincode::serialize(&meta)?)))
    }
}
