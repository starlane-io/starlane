use crate::err::ThisErr;
use crate::executor::cli::os::{CliOsExecutor, OsExeInfo};
use crate::executor::cli::HostEnv;
use crate::executor::dialect::HostDialect;
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
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use starlane::space::particle::{PointKind, Stub};

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
    pub dialect: HostDialect
}

impl ExeInfo
{
    pub fn new(dialect: HostDialect, stub: ExeStub) -> Self {
        Self {  dialect, stub }
    }


}

impl ExeInfo{
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

        ExeStub::new_with_args(path, env, ())
    }
}







