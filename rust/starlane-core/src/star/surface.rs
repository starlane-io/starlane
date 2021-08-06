use crate::cache::ProtoArtifactCachesFactory;
use crate::error::Error;
use crate::frame::{Reply, ReplyKind};
use crate::message::{Fail, ProtoStarMessage};
use crate::resource::ResourceRecord;
use crate::star::shell::message::MessagingCall;
use crate::star::{StarCommand, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use starlane_resources::ResourceIdentifier;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use crate::star::shell::locator::ResourceLocateCall;

#[derive(Clone)]
pub struct SurfaceApi {
    pub tx: mpsc::Sender<SurfaceCall>,
}

impl SurfaceApi {
    pub fn new(tx: mpsc::Sender<SurfaceCall>) -> Self {
        Self { tx }
    }

    pub fn init(&self)->Result<(),Error> {
        self.tx.try_send(SurfaceCall::Init)?;
        Ok(())
    }

    pub async fn locate(&self, identifier: ResourceIdentifier) -> Result<ResourceRecord, Fail> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Locate { identifier, tx })?;
        tokio::time::timeout(Duration::from_secs(15), rx).await??
    }

    pub async fn exchange(
        &self,
        proto: ProtoStarMessage,
        expect: ReplyKind,
        description: &str,
    ) -> Result<Reply, Fail> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Exchange {
            proto,
            expect,
            tx,
            description: description.to_string(),
        })?;
        tokio::time::timeout(Duration::from_secs(15), rx).await??
    }

    pub async fn get_caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Fail> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::GetCaches(tx))?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await??)
    }
}

pub enum SurfaceCall {
    Init,
    GetCaches(oneshot::Sender<Arc<ProtoArtifactCachesFactory>>),
    Locate {
        identifier: ResourceIdentifier,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    },
    Exchange {
        proto: ProtoStarMessage,
        expect: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Fail>>,
        description: String,
    },
}

impl Call for SurfaceCall {}

pub struct SurfaceComponent {
    skel: StarSkel,
}

impl SurfaceComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<SurfaceCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
            skel.surface_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<SurfaceCall> for SurfaceComponent {
    async fn process(&mut self, call: SurfaceCall) {
        match call {
            SurfaceCall::Init => {
                self.skel
                    .star_tx
                    .try_send(StarCommand::Init)
                    .unwrap_or_default();
            }
            SurfaceCall::GetCaches(tx) => {
                self.skel
                    .star_tx
                    .try_send(StarCommand::GetCaches(tx))
                    .unwrap_or_default();
            }
            SurfaceCall::Exchange {
                proto,
                expect,
                tx,
                description,
            } => {
                self.skel
                    .messaging_api
                    .tx
                    .try_send(MessagingCall::Exchange {
                        proto,
                        expect,
                        tx,
                        description,
                    })
                    .unwrap_or_default();
            }
            SurfaceCall::Locate { identifier, tx } => {
                self.skel
                    .resource_locator_api
                    .tx
                    .try_send(ResourceLocateCall::Locate { identifier, tx })
                    .unwrap_or_default();
            }
        }
    }
}

impl SurfaceComponent {}
