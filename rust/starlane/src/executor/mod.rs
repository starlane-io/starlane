pub mod cli;
pub mod dialect;

use crate::executor::cli::{CliExecutor, Env};
use crate::executor::cli::os::CliOsExecutor;
use crate::host::err::HostErr;
use crate::host::CommandHost;
#[async_trait]
pub trait Executor where Self::Err: std::error::Error + 'static
{
    type In;

    type Out;

    type Err;

    async fn execute(&self, args: Self::In ) -> Result<Self::Out, Self::Err>;

    fn conf(&self) -> ExecutorRunner;
}

#[derive(Clone)]
pub enum ExecutorRunner {
    Cli(CliExecutor),
    Shell(CommandHost)
}

impl ExecutorRunner {

   pub fn with_env( &self, key: &str, value: &str) -> Self {
      match self {
          ExecutorRunner::Shell(host) => ExecutorRunner::Shell(host.with_env(key, value))
      }
   }

    pub fn env( &self, key: &str ) -> Option<&String> {
        match self {
            ExecutorRunner::Shell(host) => host.env(key)
        }
    }

    pub fn create<D>(&self) -> Result<D, HostErr>
    where
        D: TryFrom<CliOsExecutor, Error =HostErr>,
    {
        match self {
            ExecutorRunner::Shell(host) => Ok(host.create::<D>()?.try_into()?)
        }
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExecutorConfig {
    /// identifier meaning differs based on the type of executor.  For a ShellProcess
    /// it's the command or filename that the Executor invokes
    pub identifier: String,
    pub env: Env,
    pub args: Vec<String>,
}

impl ExecutorConfig {
    pub fn new(indentifier: String, env: Env) -> Self {
        let args = vec![];
        Self::new_with_args(indentifier, env, args)
    }

    pub fn new_with_args(identifier: String, env: Env, args: Vec<String>) -> Self {
        Self { identifier, env, args }
    }
}