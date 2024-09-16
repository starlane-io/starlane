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

#[async_trait]
impl HostService<ExtBin,Child> for ExtHostService {
    async fn provision(&mut self, bin: ExtBin, env: Env) -> Result<Arc<dyn Host<Child>>, err::Err> {
        let key = HostKey::new(bin.clone(),env.clone());
        if !self.hosts.contains_key(&key) {
            tokio::fs::try_exists(&bin.file).await?;
            let host = Arc::new(ExtHost::new( bin.clone(), env ));
            self.hosts.insert(key.clone(),host);
        }

        Result::Ok(
            self.hosts
                .get(&key)
                .ok_or(err::Err::new(format!("could not find bin {} in hosts", &bin.file)))?
                .clone(),
        )
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
    #[tokio::test]
    pub async fn test() {

    }

}