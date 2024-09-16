use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::{Child, Command};
use crate::{err, Env, Host, HostKey, HostService, Process};

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct ExtBin {
    file: String,
}

impl ToString for ExtBin {
    fn to_string(&self) -> String {
        self.file.clone()
    }
}

impl ExtBin {
    pub fn new(file: String) -> Self {
        Self { file }
    }
}

pub struct ExtHostService {
    hosts: HashMap<HostKey<ExtBin>, Arc<ExtHost>>,
}

impl ExtHostService {
    pub fn new() -> Self {
        Self {
            hosts: Default::default(),
        }
    }
}

#[async_trait]
impl HostService<ExtBin,Child> for ExtHostService {
    async fn provision(&mut self, bin: ExtBin, env: Env) -> Result<Box<dyn Host<Child>>, err::Err> {
        let key = HostKey::new(bin.clone(),env.clone());
        return Ok(Box::new(ExtHost::new( bin.clone(), env )));

    }
}

pub struct ExtHost {
    env: Env,
    bin: ExtBin,
}

impl ExtHost {
    fn new( bin: ExtBin, env: Env) -> Self {
        Self {
            env,
            bin,
        }
    }
}

#[async_trait]
impl Host<Child> for ExtHost {
    async fn execute(&mut self, args: Vec<String>) -> Result<Child, err::Err> {

        let mut command = Command::new( self.bin.file.clone() );
        command.envs( self.env.env.clone() );
        command.args( args.clone() );
        command.current_dir(self.env.pwd.clone());
        command.env_clear();
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        Ok(command.spawn()?)

    }

}


#[cfg(test)]
pub mod test {
    use std::env::current_dir;
    use crate::{EnvBuilder, HostService};
    use crate::ext::{ExtBin, ExtHostService};

    #[tokio::test]
    pub async fn test() -> Result<(),crate::err::Err> {
        let mut service =  ExtHostService::new();
        let mut builder = EnvBuilder::default();
        builder.pwd(format!("{}/bins",current_dir().unwrap().to_str().unwrap()));
        let bin = ExtBin::new( "./filestore".to_string() );
        let mut host = service.provision(bin,builder.build()).await.unwrap();

        let child = host.execute(vec!["list".to_string()]).await?;

        let output = child.wait_with_output().await?;

        let out = String::from_utf8(output.stdout).unwrap();
        println!("{}", out);
        Ok(())
    }

}