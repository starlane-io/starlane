use std::collections::{HashSet, HashMap};
use std::sync::Arc;

use starlane_resources::data::{DataAspect, Binary, DataAspectKind};

use crate::error::Error;
use crate::starlane::api::StarlaneApi;
use std::convert::{TryInto, TryFrom};
use serde::{Serialize,Deserialize};
use crate::file_access::FileAccess;


#[derive(Clone)]
pub struct DataSetSrc<SRC> {
  pub map: HashMap<String,DataAspectSrc<SRC>>
}

impl <SRC> DataSetSrc<SRC>{
    pub fn new()->Self {
        Self {
            map: HashMap::new()
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct DataSetBlob {
    pub map: HashMap<String,Binary>
}



impl TryFrom<DataSetSrc<LocalBinSrc>> for DataSetBlob  {
    type Error = Error;

    fn try_from(value: DataSetSrc<LocalBinSrc>) -> Result<Self, Self::Error> {
        let mut map = HashMap::new();
        for (aspect, data_src) in value.map {
            map.insert(aspect,data_src.into()? );
        }
        Self{
            map: map
        }
    }
}

impl DataSetBlob{
    pub fn new() -> Self {
        Self{
            map: HashMap::new()
        }
    }
}

impl DataSetBlob {
    pub fn from_bin( bin: Binary ) -> Result<DataSetBlob,Error> {
        Ok(bincode::deserialize::<DataSetBlob>(bin.as_slice())?)
    }

    pub fn bin(&self) -> Result<Binary,Error> {
        Ok(bincode::serialize(self)?)
    }
}

impl TryInto<DataSetSrc<LocalBinSrc>> for DataSetBlob {
    type Error = Error;

    fn try_into(self) -> Result<DataSetSrc<LocalBinSrc>, Self::Error> {
        let mut map = HashMap::new();
        for (aspect, blob ) in  self.map {
            map.insert(aspect, LocalBinSrc::InMemory(blob) );
        }
        Ok(Self { map })
    }
}


impl TryInto<DataSetBlob> for DataSetSrc<Binary> {
    type Error = Error;

    fn try_into(self) -> Result<DataSetBlob, Self::Error> {
        let mut map:HashMap<String,Binary> = HashMap::new();
        for (aspect,bin) in self.map {
            map.insert( aspect, bin.into() )
        }
        Ok(DataSetBlob{
            map: map
        })
    }
}



#[derive(Clone)]
pub struct DataAspectSrc<SRC> {
    kind: DataAspectKind,
    src: SRC
}

impl Into<Binary> for DataAspectSrc<Binary> {
    fn into(self) -> Binary {
        self.src
    }
}

impl TryInto<DataAspect> for DataAspectSrc<LocalBinSrc> {
    type Error = Error;

    fn try_into(self) -> Result<DataAspect, Self::Error> {
        Ok(DataAspect::from_bin(self.src.get()?)?)
    }
}


impl TryInto<Binary> for LocalBinSrc {
    type Error = Error;

    fn try_into(self) -> Result<Binary, Self::Error> {
        match self {
            LocalBinSrc::InMemory(bin) => {
                Ok(bin)
            }
        }
    }
}


#[derive(Clone)]
pub enum BinSrc {
    Local(LocalBinSrc),
    Network(NetworkBinSrc)
}

impl BinSrc {
    pub async fn to_local(self, starlane_api: &StarlaneApi, file_access: &FileAccess ) -> Result<LocalBinSrc,Error> {
        match self {
            BinSrc::Local(local_bin_src) => {
                Ok(local_bin_src)
            }
            BinSrc::Network(network_bin_src) => {
                network_bin_src.to_local(starlane_api,file_access).await
            }
        }
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub enum NetworkBinSrc{
    InMemory(Binary)
}


impl NetworkBinSrc {
    pub async fn to_local( self, starlane_api: &StarlaneApi, file_access: &FileAccess ) -> Result<LocalBinSrc,Error> {
        match self {
            NetworkBinSrc::InMemory(bin) => Ok(LocalBinSrc::InMemory(bin))
        }
    }
}

#[derive(Clone)]
pub enum LocalBinSrc {
    InMemory(Binary)
}

impl LocalBinSrc {
    pub fn get(&self) -> Result<Binary,Error> {
        match self {
            LocalBinSrc::InMemory(binary) => Ok(binary.clone())
        }
    }
}

