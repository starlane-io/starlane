use crate::core::InertHost;
use crate::data::{BinSrc, DataSet};
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, ResourceAssign, ResourceKey, Resource, ResourceType};
use tokio::sync::{mpsc, oneshot};
use crate::star::StarSkel;
use crate::util::{AsyncRunner, AsyncProcessor, Call};
use crate::error::Error;
use crate::star::core::resource::host::space::SpaceHost;

pub mod artifact;
pub mod default;
pub mod file_store;
pub mod kube;
mod space;

pub enum HostCall{
    Assign{   assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>, tx: oneshot::Sender<Result<(),Fail>> },
    Get{ key: ResourceKey, tx: oneshot::Sender<Result<Option<DataSet<BinSrc>>,Fail>> },
    Has{ key: ResourceKey, tx: oneshot::Sender<bool> }
}

impl Call for HostCall{}

pub struct HostComponent{
    skel: StarSkel
}

impl HostComponent{
    pub fn new(skel: StarSkel) -> mpsc::Sender<HostCall> {
        let (tx,rx) = mpsc::channel(1024);
        AsyncRunner::new(Box::new(Self{
            skel
        }), tx.clone(), rx );
        tx
    }
}

#[async_trait]
impl AsyncProcessor<HostCall> for HostComponent{
    async fn process(&mut self, call: HostCall) {
        match call {
            HostCall::Get { key, tx } => {
                let host = self.host( key.resource_type() ).await;
                tx.send(host.get(key).await);
            }
            HostCall::Assign { assign, tx } => {
                let host = self.host( assign.stub.key.resource_type() ).await;
                tx.send(host.assign(assign).await);
            }
            HostCall::Has { key, tx } => {
                let host = self.host( key.resource_type() ).await;
                tx.send(host.has(key).await);
            }
        }
    }
}

impl HostComponent {
    async fn host( &self, rt: ResourceType ) -> Box<dyn Host> {
        match rt {
            ResourceType::Space => {
                Box::new(SpaceHost::new(self.skel.clone()).await )
            }
            _ => Box::new(InertHost::new() )
        }
    }
}



#[async_trait]
impl Host for InertHost {
    async fn assign(
        &self,
        _assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<(), Fail> {
        Err(Fail::Error(
            "This is an InertHost which cannot actually host anything".into(),
        ))
    }

    async fn has(&self, key: ResourceKey) -> bool {
        todo!()
    }

    async fn get(&self, identifier: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Fail> {
        Err(Fail::Error(
            "This is an InertHost which cannot actually host anything".into(),
        ))
    }

    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Fail> {
        Err(Fail::Error(
            "This is an InertHost which cannot actually host anything".into(),
        ))
    }
}

#[async_trait]
pub trait Host: Send + Sync {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<(), Fail>;
    async fn has(&self, key: ResourceKey) -> bool;
    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Fail>;
    async fn delete(&self, key: ResourceKey) -> Result<(), Fail>;
    fn shutdown(&self) {}
}
