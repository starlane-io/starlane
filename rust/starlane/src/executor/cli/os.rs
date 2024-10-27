use std::path::PathBuf;
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use std::process::Stdio;
use std::ops::{Deref, DerefMut};
use tokio::io::AsyncWriteExt;
use crate::executor::cli::{CliErr, CliIn, CliOut};
use crate::executor::{Executor, ExecutorConfig, ExecutorRunner};
use crate::host::{CommandHost, HostCli, ShellExecutor};

#[derive(Clone)]
pub struct CliOsExecutor
{
    pub stub: ExecutorConfig,
}

impl CliOsExecutor {
    pub fn new(stub: ExecutorConfig) -> Self
    {
        Self { stub }
    }
}


#[async_trait]
impl Executor for CliOsExecutor {

    type In = CliIn;
    type Out = CliOut;
    type Err = CliErr;

    async fn execute(&self, mut input: Self::In) -> Result<Self::Out, Self::Err> {
        let path: PathBuf = self.stub.identifier.clone().into();
        if !path.exists() {
            Result::Err(CliErr::FileNotFound(self.stub.identifier.clone()))?;
        }


        let mut command = Command::new(self.stub.identifier.clone());

        command.envs(self.stub.env.env.clone());
        command.args(&input.args);
        command.current_dir(self.stub.env.pwd.clone());
        command.env_clear();
        command.envs(&self.stub.env.env);
        //command.stdin(Stdio::piped()).output().await?;
        command.stdin(Stdio::piped()).output().await?;
        //command.stdout(Stdio::piped()).output().await?;
        command.stdout(Stdio::piped()).output().await?;
        //command.stderr(Stdio::piped()).output().await?;
        command.stderr(Stdio::piped()).output().await?;
        let child = command.spawn()?;
        //        Ok(OsProcess::new(child))
        let mut process = ShellExecutor::new(child);

        if let Option::Some(mut data) = input.stdin.take() {
            let mut stdin = process.stdin.take().ok_or(CliErr::TakeStdIn)?;
            stdin.write_all(&mut data).await?;
            stdin.flush().await?;
        }

        process.close_stdin()?;


        Ok(CliOut::Shell(process))
    }

    fn conf(&self) -> ExecutorRunner {
        ExecutorRunner::Shell(CommandHost::Cli(HostCli::Os(self.stub.clone())))
    }
}



pub struct OsProcess {
    child: Child,
}

impl ShellExecutor {
    pub fn close_stdin(&mut self) -> Result<(), CliErr> {
        if self.child.stdin.is_some() {
            drop(self.child.stdin.take().ok_or(CliErr::TakeStdIn)?);
        }
        Ok(())
    }
}

impl Deref for ShellExecutor {
    type Target = Child;

    fn deref(&self) -> &Self::Target {
        &self.child
    }
}

impl DerefMut for ShellExecutor {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.child
    }
}

impl ShellExecutor {
    pub fn new(child: Child) -> Self {
        Self { child }
    }
}

impl ShellExecutor for ShellExecutor {
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