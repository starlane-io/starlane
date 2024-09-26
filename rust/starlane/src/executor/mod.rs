pub mod cli;
pub mod dialect;

use crate::err::ThisErr;
use crate::executor::cli::os::CliOsExecutor;
use crate::host::Host;

#[async_trait]
pub trait Executor
{
    type In;

    type Out;

    async fn execute(&self, args: Self::In ) -> Result<Self::Out,ThisErr>;

    fn conf(&self) -> ExeConf;
}

pub enum ExeConf {
    Host(Host)
}

impl ExeConf {


   pub fn with_env( &self, key: &str, value: &str) -> Self {
      match self {
          ExeConf::Host(host) => ExeConf::Host(host.with_env(key,value))
      }
   }

    pub fn env( &self, key: &str ) -> Option<&String> {
        match self {
            ExeConf::Host(host) => host.env(key)
        }
    }

    pub fn create<D>(&self) -> Result<D, ThisErr>
    where
        D: TryFrom<CliOsExecutor, Error = ThisErr>,
    {
        match self {
            ExeConf::Host(host) => host.create()
        }
    }
}







