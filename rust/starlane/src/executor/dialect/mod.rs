use crate::err::ThisErr;
use crate::executor::cli::{CliIn, CliOut};
use crate::executor::cli::os::{CliOsExecutor, OsExeStub, OsStub};
use crate::executor::Executor;
use crate::host::{ExeStub, StrStub};

pub mod filestore;

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum ServiceDialect {
    FileStore
}


#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostDialect{
    Cli(HostRunner)
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostRunner {
   Os
}

impl HostDialect {
    pub fn create_cli<D>( &self, stub: &ExeStub) -> Result<Box<D>,ThisErr> where D: From<Box<CliOsExecutor>>{
        match self {
            HostDialect::Cli(host) => {
                host.create_cli(stub)
            }
        }
    }
}


impl HostRunner {
    pub fn create_cli<D>( &self, stub: &ExeStub) -> Result<Box<D>,ThisErr> where D: From<Box<CliOsExecutor>>{
        match self {
            HostRunner::Os => {
                Ok(Box::new(D::from( Box::new(CliOsExecutor::new(stub.clone())) )))
            }
        }
    }
}




