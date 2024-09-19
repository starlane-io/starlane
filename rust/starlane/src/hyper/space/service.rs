use std::collections::HashMap;
use std::path::PathBuf;
use starlane_space::parse::Env;
use starlane_space::point::Point;
use starlane_space::wave::{Bounce, DirectedWave, ReflectedWave, UltraWave};
use starlane_space::wave::core::Method;
use crate::err::StarlaneErr;
use crate::host::{Executor, ExtExecutor};
use crate::hyper::space::err::HyperErr;
use crate::store::FileStore;

pub enum ServiceKey {
   RepoStore,
   FileStore
}

pub enum ComStyle {
   Cli,
   Connection(ConnectionKind)
}

pub enum Spawn {
    None,
    Exe(ExtExecutor)
}



pub enum ConnectionKind {
    Receiver,
    Connector{
        host: String,
        port: u16
    }
}


///
///  Invocation -- the service command is invoked for every message (slowest and most secure)
///  Particle  --  A service must be created for each  Particle which can handle multiple requests
///  Portal --
///
pub enum Isolation {
    Invocation,
    Particle,
    Portal
}

pub struct ServiceTemplate{
    pub invocation_kind: ComStyle,
    pub ReentrantIsolation: Isolation,
    pub key: ServiceKey,
    pub env: HashMap<String, String>,
    pub pwd: PathBuf,
}


pub trait Service: Sync + Send where Self::Err : HyperErr{
   type Err;

}