use std::collections::{HashSet, HashMap};
use std::sync::Arc;

use crate::{Path, ResourcePathSegment};
use crate::error::Error;
use serde::{Serialize,Deserialize};


pub type Binary = Arc<Vec<u8>>;
pub type DataSchema = DataAspectKind;
pub type DataSet = HashMap<String,DataAspect>;

#[derive(Clone,Serialize,Deserialize)]
pub struct Meta {
  pub hash: HashMap<String,String>
}


pub enum DataAspectKind {
    Meta,
    Binary
}

impl DataAspectKind{


}

#[derive(Clone,Serialize,Deserialize)]
pub enum DataAspect{
    Meta(Meta),
    Binary(Binary)
}

impl DataAspect {

    pub fn from_bin( bin: Binary ) -> Result<DataAspect,Error> {
        Ok(bincode::deserialize::<DataAspect>(bin.as_slice())?)
    }

    pub fn bin(&self) -> Result<Binary,Error> {
        match self {
            DataAspect::Meta( meta ) => {
                Ok(Arc::new(bincode::serialize(self)?))
            }
            DataAspect::Binary(binary) => {
                Ok(binary.clone())
            }
        }
    }



}

