use crate::starlane_hyperspace::executor::cli::os::CliOsExecutor;
use crate::starlane_hyperspace::executor::cli::{CliIn, CliOut, HostEnv};
use crate::starlane_hyperspace::executor::{ExeConf, Executor};
use clap::CommandFactory;
use itertools::Itertools;
use nom::AsBytes;
use starlane::space::wave::exchange::asynch::DirectedHandler;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::ops::{Deref, DerefMut};
use tokio::io::AsyncWriteExt;
use crate::starlane_hyperspace::host::err::HostErr;
use crate::starlane_hyperspace::service::ServiceErr;

pub mod err;

#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ExtKey<B>
where
    B: Hash + Eq + PartialEq,
{
    bin: B,
    env: HostEnv,
}

impl<B> ExtKey<B>
where
    B: Clone + Hash + Eq + PartialEq,
{
    pub fn new(bin: B, env: HostEnv) -> Self {
        Self { bin, env }
    }
}

pub trait Proc {
    type StdOut;
    type StdErr;
    type StdIn;
    fn stderr(&self) -> Option<&Self::StdErr>;
    fn stdout(&self) -> Option<&Self::StdOut>;

    fn stdin(&mut self) -> Option<&Self::StdIn>;
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeStub {
    pub loc: String,
    pub env: HostEnv,
    pub args: Vec<String>,
}


impl ExeStub {
    pub fn new(loc: String, env: HostEnv) -> Self {
        let args = vec![];
        Self::new_with_args(loc, env, args)
    }

    pub fn new_with_args(loc: String, env: HostEnv, args: Vec<String>) -> Self {
        Self { loc, env, args }
    }
}

pub fn stringify_args(args: Vec<&str>) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}




#[derive(Clone, Hash, Eq, PartialEq)]
pub enum Host {
    Cli(HostCli),
}

impl Host {

    pub fn env( &self, key: &str ) -> Option<&String> {
        match self {
            Host::Cli(cli) => cli.env(key)
        }
    }
    pub fn create<D>(&self) -> Result<D, HostErr>
    where
        D: TryFrom<CliOsExecutor, Error =HostErr>,
    {
        match self {
            Host::Cli(host) => host.create(),
        }
    }

    pub fn with_env( &self, key: &str, value: &str) -> Self {
        match self {
            Host::Cli(cli) => Host::Cli(cli.with_env(key,value))
        }
    }

}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostCli {
    Os(ExeStub),
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
