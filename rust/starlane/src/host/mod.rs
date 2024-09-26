use std::collections::HashSet;
use crate::err::ThisErr;
use crate::executor::cli::os::CliOsExecutor;
use crate::executor::cli::{CliIn, CliOut, HostEnv};
use crate::executor::Executor;
use crate::hyperspace::err::HyperErr;
use clap::CommandFactory;
use itertools::Itertools;
use nom::AsBytes;
use starlane::space::wave::exchange::asynch::DirectedHandler;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::io::Read;
//use virtual_fs::FileSystem;
use std::ops::{Deref, DerefMut};
use tokio::io::AsyncWriteExt;
pub mod err;
pub mod ext;





//pub mod wasm;

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
pub struct ExeStub
{
    pub loc: String,
    pub env: HostEnv,
    pub args: Vec<String>,
}

impl ExeStub
{
    pub fn new(loc: String, env: HostEnv ) -> Self {
        let args = vec![];
        Self::new_with_args(loc, env, args)
    }

    pub fn new_with_args(loc: String, env: HostEnv, args: Vec<String> ) -> Self {
        Self { loc, env, args }
    }
}

pub fn stringify_args(args: Vec<&str>) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}



#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeInfo
{
    pub stub: ExeStub,
    pub dialect: Host
}

impl ExeInfo
{
    pub fn new(dialect: Host, stub: ExeStub) -> Self {
        Self {  dialect, stub }
    }


}

impl ExeInfo{
    pub fn create<D>( &self ) -> Result<D,ThisErr> where D: TryFrom<CliOsExecutor,Error=ThisErr>{
        match &self.dialect {
            Host::Cli(cli) => {
                cli.create_cli( )
            }
            _=>  {
                Err("Host does not support CLI (Command Line Interface)".into())
            }
        }
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum Host {
    Cli(HostCli)
}

impl Host {
    pub fn create_cli<D>( &self) -> Result<D,ThisErr> where D: TryFrom<CliOsExecutor,Error=ThisErr>{
        match self {
            Host::Cli(host) => {
                host.create_cli()
            }
        }
    }

    pub fn sub( &mut self, key: ParamKey, param: ParamOp )-> Result<(),ThisErr> {

    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum HostCli {
   Os(ExeStub)
}

impl HostCli {
    pub fn create_cli<D>( &self) -> Result<D,ThisErr> where D: TryFrom<CliOsExecutor,Error=ThisErr>{
        match self {
            HostCli::Os(stub) => {
                D::try_from(CliOsExecutor::new(stub.clone()))
            }
        }
    }
}


#[derive(Clone, Eq, PartialEq)]
pub struct Params {
    params: HashSet<ParamKey>
}

impl Params {
     pub fn sub( &self, host: Host ) -> Host {

     }
}

#[derive(Clone, Hash,Eq, PartialEq)]
pub enum ParamKey {
    Env(String),
}

pub enum ParamOp {
    Replace(String),
    Append(String)
}