use std::env;
use std::path::PathBuf;
use tokio_print::aprintln;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use std::process::Stdio;
use std::ops::{Deref, DerefMut};
use crate::err::ThisErr;
use crate::executor::cli::{CliIn, CliOut, HostEnv};
use crate::executor::Executor;
use crate::host::{ExeInfo, ExeStub, Proc, StrStub};

#[derive(Clone)]
pub struct CliOsExecutor {
    pub stub: OsExeStub,
}

impl CliOsExecutor {
    pub fn new<I>(info: I) -> Self
    where
        I: Into<OsExeStub>,
    {
        let info = info.into();
        Self { stub: info }
    }
}
#[async_trait]
impl Executor for CliOsExecutor {
    type In = CliIn;
    type Out = CliOut;

    async fn execute(&self, args: Self::In) -> Result<Self::Out, ThisErr> {
        if !self.stub.loc.exists() {
            Result::Err(ThisErr::String(format!(
                "file not found: {}",
                self.stub.loc.display()
            )))?;
        }

        aprintln!("pwd: {}", env::current_dir().unwrap().display());
        aprintln!("self.stub.loc.exists(): {}", self.stub.loc.exists());
        aprintln!("self.stub.loc: {}", self.stub.loc.display());
        let mut command = Command::new(self.stub.loc.clone());

        command.envs(self.stub.env.env.clone());
        command.args(&args.args);
        command.current_dir(self.stub.env.pwd.clone());
        command.env_clear();
        command.envs(&self.stub.env.env);
        aprintln!("GOT HERE...");
        //command.stdin(Stdio::piped()).output().await?;
        command.stdin(Stdio::piped()).output().await?;
        aprintln!("STDIN");
        //command.stdout(Stdio::piped()).output().await?;
        command.stdout(Stdio::piped()).output().await?;
        aprintln!("STDOUT");
        //command.stderr(Stdio::piped()).output().await?;
        command.stderr(Stdio::piped()).output().await?;
        aprintln!("STDERR");
        println!("{:?}", command);
        let child = command.spawn()?;
        aprintln!("child created...");
        //        Ok(OsProcess::new(child))
        let process = OsProcess::new(child);
        Ok(CliOut::Os(process))
    }
}

pub type OsExeInfo = ExeInfo<PathBuf, OsEnv, ()>;
pub type OsExeStub = ExeStub<PathBuf, OsEnv, ()>;

impl From<StrStub> for OsExeStub {
    fn from(stub : StrStub) -> Self {
        Self {
            args: (),
            env: stub.env,
            loc: PathBuf::from(stub.loc),
        }
    }
}
pub type OsExeStubArgs = ExeStub<PathBuf, HostEnv, Vec<String>>;
pub type OsStub = ExeStub<PathBuf, HostEnv, ()>;
pub type OsEnv = HostEnv;


pub struct OsProcess {
    child: Child,
}

impl OsProcess {
    pub fn close_stdin(&mut self) -> Result<(), ThisErr> {
        drop(self.child.stdin.take().unwrap());
        Ok(())
    }
}

impl Deref for OsProcess {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl DerefMut for OsProcess {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}

impl OsProcess {
    pub fn new(child: Child) -> Self {
        Self { child }
    }
}

impl Proc for OsProcess {
    type StdOut = ChildStdout;
    type StdIn = ChildStdin;
    type StdErr = ChildStderr;

    fn stderr(&self) -> Option<&Self::StdErr> {
        self.child.stderr.as_ref()
    }

    fn stdout(&self) -> Option<&Self::StdOut> {
        self.child.stdout.as_ref()
    }

    fn stdin(&mut self) -> Option<&Self::StdIn> {
        self.child.stdin.as_ref()
    }
}