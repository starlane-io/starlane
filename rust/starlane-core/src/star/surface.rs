use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use starlane_resources::message::{Fail, ResourcePortMessage, Message, MessageReply};
use starlane_resources::ResourceIdentifier;

use crate::cache::ProtoArtifactCachesFactory;
use crate::error::Error;
use crate::frame::{Reply, ReplyKind};
use crate::message::ProtoStarMessage;
use crate::resource::{ResourceRecord, ResourceKey};
use crate::star::{StarCommand, StarSkel};
use crate::star::shell::locator::ResourceLocateCall;
use crate::star::shell::message::MessagingCall;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::watch::{WatchSelector, Notification, Topic, Watch, WatchResourceSelector, WatchListener};

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

    pub async fn locate(&self, identifier: ResourceIdentifier) -> Result<ResourceRecord, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Locate { identifier, tx })?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    pub async fn exchange(
        &self,
        proto: ProtoStarMessage,
        expect: ReplyKind,
        description: &str,
    ) -> Result<Reply, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Exchange {
            proto,
            expect,
            tx,
            description: description.to_string(),
        })?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    pub async fn watch( &self, selector: WatchResourceSelector) -> Result<WatchListener, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Watch{selector, tx})?;
        tokio::time::timeout(Duration::from_secs(15), rx).await??
    }

    pub async fn get_caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Error> {
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
        tx: oneshot::Sender<Result<ResourceRecord, Error>>,
    },
    Exchange {
        proto: ProtoStarMessage,
        expect: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Error>>,
        description: String,
    },
    Watch{ selector: WatchResourceSelector, tx: oneshot::Sender<Result<WatchListener,Error>> }

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
            SurfaceCall::Watch { selector, tx } => {
                        let selector = match self.skel.resource_locator_api.fetch_resource_key(selector.resource.clone()).await
                        {
                            Ok(key) => {
                                WatchSelector{
                                    topic: Topic::Resource(key),
                                    property: selector.property
                                }
                            }
                            Err(err) => {
                                tx.send( Result::Err(err.into()) );
                                return;
                            }
                        };

                tx.send(self.skel.watch_api.listen( selector ).await);
            }
        }
    }
}

impl SurfaceComponent {}
