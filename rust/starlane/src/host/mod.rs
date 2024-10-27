use crate::executor::cli::os::CliOsExecutor;
use crate::executor::{Executor, ExecutorConfig};
use itertools::Itertools;
use nom::AsBytes;
use starlane::space::wave::exchange::asynch::DirectedHandler;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::ops::{Deref, DerefMut};
use tokio::io::AsyncWriteExt;
use crate::host::err::HostErr;
pub mod err;


pub trait ShellExecutor {
    type StdOut;
    type StdErr;
    type StdIn;
    fn stderr(&self) -> Option<&Self::StdErr>;
    fn stdout(&self) -> Option<&Self::StdOut>;
    fn stdin(&mut self) -> Option<&Self::StdIn>;
}


pub(crate) fn stringify_args(args: Vec<&str>) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}




#[derive(Clone, Hash, Eq, PartialEq)]
pub enum CommandHost {
    Cli(HostCli),
}

impl CommandHost {

    pub fn env( &self, key: &str ) -> Option<&String> {
        match self {
            CommandHost::Cli(cli) => cli.env(key)
        }
    }
    pub fn create<D>(&self) -> Result<D, HostErr>
    where
        D: TryFrom<CliOsExecutor, Error =HostErr>,
    {
        match self {
            CommandHost::Cli(host) => host.create(),
        }
    }

    pub fn with_env( &self, key: &str, value: &str) -> Self {
        match self {
            CommandHost::Cli(cli) => CommandHost::Cli(cli.with_env(key, value))
        }
    }

}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostCli {
    Os(ExecutorConfig),
}

impl HostCli {

    pub fn env( &self, key: &str ) -> Option<&String> {
        let key = key.to_string();
        match self {
            HostCli::Os(stub) => stub.env.env.get( &key )
        }
    }

    pub fn create<D>(&self) -> Result<D, HostErr>
    where
        D: TryFrom<CliOsExecutor, Error =HostErr>,
    {
        match self {
            HostCli::Os(stub) => Ok(D::try_from(CliOsExecutor::new(stub.clone()))?),
        }
    }


    pub fn with_env( &self, key: &str, value: &str) -> Self {
        match self {
            HostCli::Os(stub) => {
                let mut stub = stub.clone();
                stub.env.env.insert(key.to_string(), value.to_string());
                HostCli::Os(stub)
            }
        }
    }
}
