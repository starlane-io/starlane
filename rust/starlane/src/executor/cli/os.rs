use std::env;
use std::path::PathBuf;
use tokio_print::aprintln;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use std::process::Stdio;
use std::ops::{Deref, DerefMut};
use tokio::io::AsyncWriteExt;
use crate::executor::cli::{CliErr, CliIn, CliOut, HostEnv};
use crate::executor::{ExeConf, Executor};
use crate::host::{ ExeStub, Host, HostCli, Proc};

#[derive(Clone)]
pub struct CliOsExecutor
{
    pub stub: ExeStub,
}

impl CliOsExecutor {
    pub fn new(stub: ExeStub) -> Self
    {
        Self { stub }
    }
}


#[async_trait]
impl Executor for CliOsExecutor {
    type In = CliIn;
    type Out = CliOut;

    async fn execute(&self, mut input: Self::In) -> Result<Self::Out, CliErr> {
        let path: PathBuf = self.stub.loc.clone().into();
        if !path.exists() {
            Result::Err(CliErr::FileNotFound(self.stub.loc.clone()))?;
        }

        aprintln!("pwd: {}", env::current_dir().unwrap().display());
        aprintln!("self.stub.loc.exists(): {}", path.exists());
        aprintln!("self.stub.loc: {}", path.display());
        let mut command = Command::new(self.stub.loc.clone());

        command.envs(self.stub.env.env.clone());
        command.args(&input.args);
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
        let mut process = OsProcess::new(child);

        if let Option::Some(mut data) = input.stdin.take() {
            let mut stdin = process.stdin.take().ok_or(CliErr::TakeStdIn)?;
            stdin.write_all(&mut data).await?;
            stdin.flush().await?;
        }

        process.close_stdin()?;


        Ok(CliOut::Os(process))
    }

    fn conf(&self) -> ExeConf {
        ExeConf::Host(Host::Cli(HostCli::Os(self.stub.clone())))
    }
}



pub struct OsProcess {
    child: Child,
}

impl OsProcess {
    pub fn close_stdin(&mut self) -> Result<(), CliErr> {
        if self.child.stdin.is_some() {
            drop(self.child.stdin.take().ok_or(CliErr::TakeStdIn)?);
        }
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