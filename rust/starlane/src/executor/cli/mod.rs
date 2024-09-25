pub mod os;

use crate::err::ThisErr;
use os::OsProcess;
use crate::executor::Executor;
use itertools::Itertools;
use std::collections::HashMap;
use std::env;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use tokio::io::AsyncWriteExt;
use crate::host::Proc;

pub type CliIn = CliInDef<Option<Vec<u8>>>;
#[derive(Clone, Eq, PartialEq)]
pub struct HostEnv {
    pub pwd: String,
    pub env: HashMap<String, String>,
}

impl HostEnv {
    pub fn builder() -> HostEnvBuilder {
        HostEnvBuilder::default()
    }
}

impl Hash for HostEnv {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_str(self.pwd.as_str());
        for key in self.env.keys().sorted() {
            state.write_str(key.as_str());
            state.write_str(self.env.get(key).unwrap());
        }
    }
}

impl Default for HostEnv {
    fn default() -> Self {
        Self {
            pwd: env::current_dir().unwrap().to_str().unwrap().to_string(),
            env: HashMap::default(),
        }
    }
}

#[derive(Clone)]
pub struct HostEnvBuilder {
    pwd: String,
    env: HashMap<String, String>,
}

impl Default for HostEnvBuilder {
    fn default() -> Self {
        Self {
            pwd: ".".to_string(),
            env: Default::default(),
        }
    }
}

impl HostEnvBuilder {
    pub fn build(self) -> HostEnv {
        HostEnv {
            pwd: self.pwd,
            env: self.env,
        }
    }
    pub fn pwd<S>(&mut self, pwd: S) -> &mut Self
    where
        S: ToString,
    {
        self.pwd = pwd.to_string();
        self
    }

    pub fn env<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: ToString,
        V: ToString,
    {
        self.env.insert(key.to_string(), value.to_string());
        self
    }
}

pub struct CliInDef<S> {
    pub args: Vec<String>,
    pub stdin: S,
}

impl<S> CliInDef<S> {
    pub fn new(args: Vec<String>, stdin: S) -> Self {
        Self { args, stdin }
    }
}

impl CliIn {
    pub fn args(args: Vec<&str>) -> Self {
        let args = args.iter().map(|s| s.to_string()).collect();
        Self::str_args(args)
    }

    pub fn str_args(args: Vec<String>) -> Self {
        Self { args, stdin: None }
    }

    pub fn stdin(args: Vec<&str>, stdin: Vec<u8>) -> Self {
        let args = args.iter().map(|s| s.to_string()).collect();
        Self::str_stdin(args, stdin)
    }

    pub fn str_stdin(args: Vec<String>, stdin: Vec<u8>) -> Self {
        Self {
            args,
            stdin: Some(stdin),
        }
    }
}

pub enum CliOut {
    Os(OsProcess),
}

impl CliOut {

    pub async fn copy_stdin(&mut self, input: & mut Vec<u8>) -> Result<(),ThisErr>{
        match self {
            CliOut::Os(proc) => {
                let mut stdin = proc.stdin.take().unwrap();
                stdin.write_all( &input[..] ).await?;
                stdin.flush().await?;
            }
        }
        Ok(())
    }
    pub fn close_stdin(&mut self)  -> Result<(),ThisErr>{
        match self {
            CliOut::Os(proc) => proc.close_stdin()?
        }
        Ok(())
    }

    pub async fn copy_stout(&mut self, out: & mut Vec<u8>)  -> Result<(),ThisErr>{
        match self {
            CliOut::Os(proc) => {
                let mut stdout = proc.stdout.take().ok_or("could not get stdout")?;
                tokio::io::copy(&mut stdout, out).await?;
            }
        }
        Ok(())
    }
}

impl CliOut {
    pub async fn stdout(&mut self) -> Result<Vec<u8>, ThisErr> {
        match self {
            CliOut::Os(proc) => {
                let mut out = vec![];
                let mut stdout = proc
                    .stdout
                    .take()
                    .ok_or(ThisErr::String("could not unwarp stdout".to_string()))?;
                tokio::io::copy(&mut stdout, &mut out).await?;
                Ok(out)
            }
        }
    }
}

pub type CliExecutor = Box<dyn Executor<In=CliIn,Out=CliOut>>;