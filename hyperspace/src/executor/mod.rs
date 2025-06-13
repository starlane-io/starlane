pub mod cli;
pub mod dialect;

use crate::executor::cli::os::CliOsExecutor;
use crate::host::err::HostErr;
use crate::host::Host;
use async_trait::async_trait;

#[async_trait]
pub trait Executor
where
    Self::Err: std::error::Error + 'static,
{
    type In;

    type Out;

    type Err;

    async fn execute(&self, args: Self::In) -> Result<Self::Out, Self::Err>;

    fn conf(&self) -> ExeConf;
}

#[derive(Clone)]
pub enum ExeConf {
    Host(Host),
}

impl ExeConf {
    pub fn with_env(&self, key: &str, value: &str) -> Self {
        match self {
            ExeConf::Host(host) => ExeConf::Host(host.with_env(key, value)),
        }
    }

    pub fn env(&self, key: &str) -> Option<&String> {
        match self {
            ExeConf::Host(host) => host.env(key),
        }
    }

    pub fn create<D>(&self) -> Result<D, HostErr>
    where
        D: TryFrom<CliOsExecutor, Error = HostErr>,
    {
        match self {
            ExeConf::Host(host) => Ok(host.create::<D>()?.try_into()?),
        }
    }
}
