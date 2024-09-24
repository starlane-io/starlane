use crate::hyperspace::err::HyperErr;
use itertools::Itertools;
use starlane::space::wave::exchange::asynch::DirectedHandler;
use std::fmt::Write;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::PathBuf;
use virtual_fs::FileSystem;
use std::ops::{Deref, DerefMut};
use clap::CommandFactory;
use nom::AsBytes;
use tokio::io::AsyncWriteExt;
use crate::err::{StarErr, ThisErr};
use crate::executor::cli::{CliExecutor, CliIn, CliOut, HostEnv};
use crate::executor::cli::os::{CliOsExecutor, OsExeInfo, OsStub};
use crate::executor::dialect::filestore::FileStore;
use crate::executor::dialect::HostDialect;
use crate::executor::Executor;

pub mod err;
pub mod ext;



pub enum Host {
    Cli(CliHost)
}

pub enum CliHost {
    Os
}


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


pub type StrStub = ExeStub<String,HostEnv,Vec<String>>;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeStub<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub loc: L,
    pub env: E,
    pub args: A,
}

impl<L, E, A> ExeStub<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub fn new(loc: L, env: E, args: A) -> Self {
        Self { loc, env, args }
    }
}

pub fn stringify_args(args: Vec<&str>) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}



#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExeInfo<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub stub: ExeStub<L, E, A>,
    pub dialect: HostDialect
}

impl<L, E, A> ExeInfo<L, E, A>
where
    E: Clone + Hash + Eq + PartialEq,
    L: Clone + Hash + Eq + PartialEq,
    A: Clone + Hash + Eq + PartialEq,
{
    pub fn new(dialect: HostDialect, stub: ExeStub<L, E, A>) -> Self {
        Self {  dialect, stub }
    }


}

impl OsExeInfo{
    pub fn create<D>( &self ) -> Result<Box<D>,ThisErr> where D: From<Box<CliOsExecutor>> {
        self.dialect.create_cli( &self.stub )
    }
}

impl StrStub {

}





impl<L, E, A> ExeInfo<L, E, A>
where
    L: Clone + Hash + Eq + PartialEq + Into<PathBuf>,
    E: Clone + Hash + Eq + PartialEq + Into<HostEnv>,
    A: Clone + Hash + Eq + PartialEq,
{



}

impl<L, E, A> From<&ExeStub<L, E, A>> for ExeStub<PathBuf, HostEnv, ()>
where
    L: Clone + Hash + Eq + PartialEq + Into<PathBuf>,
    E: Clone + Hash + Eq + PartialEq + Into<HostEnv>,
    A: Clone + Hash + Eq + PartialEq,
{
    fn from(stub: &ExeStub<L, E, A>) -> Self {
        let path = stub.loc.clone().into();
        let env = stub.env.clone().into();

        ExeStub::new(path, env, ())
    }
}





