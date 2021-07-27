use std::collections::HashSet;
use std::sync::Arc;
use crate::starlane::api::StarlaneApi;
use crate::error::Error;

pub type DataSetSrc = HashSet<String,DataAspectSrc>;
pub type Meta = HashSet<String,String>;
pub type Binary = Arc<Vec<u8>>;

pub enum DataAspectType{
    Meta,
    Binary
}

#[derive(Clone)]
pub enum DataAspect{
    Meta(Meta),
    Binary(Binary)
}

#[derive(Clone)]
pub enum DataAspectSrc {
    Local(Box<dyn LocalDataAspectSrc>),
    System(Box<dyn SystemDataAspectSrc>),
    External(Option<String>)
}

impl DataAspectSrc {
    pub async fn get(self, starlane_api: StarlaneApi ) -> Result<DataAspect, Error> {
        match self {
            DataAspectSrc::Local(local) => {
                local.get()
            }
            DataAspectSrc::System(system) => {
                system.get(starlane_api).await
            }
            DataAspectSrc::External(_) => {
                Err("cannot get data for an external src")
            }
        }
    }
}

pub trait LocalDataAspectSrc: Clone {
  fn get(self) -> Result<DataAspect,Error>;
}

#[async_trait]
pub trait SystemDataAspectSrc: Clone {
    async fn get(&self, starlane_api: StarlaneApi ) -> Result<DataAspect,Error>;
}

#[derive(Clone)]
pub struct InMemoryDataAspectSrc {
  pub data: DataAspect
}

#[async_trait]
impl LocalDataAspectSrc for InMemoryDataAspectSrc{
    fn get(self) -> Result<DataAspect, Error> {
       self.data
    }
}
