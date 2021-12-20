use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::cache::ProtoArtifactCachesFactory;
use crate::error::Error;
use crate::frame::{StarPattern};
use crate::message::{ProtoStarMessage, ReplyKind, Reply};
use crate::resource::{ResourceRecord };
use crate::star::{StarCommand, StarSkel, StarInfo};
use crate::star::shell::locator::ResourceLocateCall;
use crate::star::shell::message::MessagingCall;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::watch::{WatchSelector, Notification, Topic, Watch, WatchResourceSelector, Watcher};
use crate::star::shell::search::SearchHits;
use crate::mesh::serde::resource::command::create::AddressTemplate;
use crate::mesh::serde::id::Address;
use crate::resources::message::ProtoRequest;
use crate::mesh::Response;
use crate::mesh::serde::messaging::ExchangeType;

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

    pub async fn locate(&self, address: Address ) -> Result<ResourceRecord, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Locate { address: address, tx })?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    pub fn notify( &self, mut request: ProtoRequest ) {
        request.exchange = ExchangeType::Notification;
        self.tx.try_send(SurfaceCall::Notify(request));
    }

    pub async fn exchange( &self, mut request: ProtoRequest ) -> Result<Response,Error> {
        request.exchange = ExchangeType::RequestResponse;
        let (tx,rx) = oneshot::channel();
        self.tx.send(SurfaceCall::Exchange{request,tx}).await;

        Ok(rx.await??)
    }


    pub async fn exchange_proto_star_message(
        &self,
        proto: ProtoStarMessage,
        expect: ReplyKind,
        description: &str,
    ) -> Result<Reply, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::ExchangeStarMessage {
            proto,
            expect,
            tx,
            description: description.to_string(),
        })?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    pub async fn watch( &self, selector: WatchResourceSelector) -> Result<Watcher, Error> {
println!("SurfaceApi::watch()");
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Watch{selector, tx})?;
        tokio::time::timeout(Duration::from_secs(15), rx).await??
    }

    pub async fn get_caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::GetCaches(tx))?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await??)
    }

    pub async fn star_search(&self, star_pattern: StarPattern) -> Result<SearchHits,Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::StarSearch{star_pattern,tx})?;
        tokio::time::timeout(Duration::from_secs(15), rx).await??
    }


}

pub enum SurfaceCall {
    Init,
    GetCaches(oneshot::Sender<Arc<ProtoArtifactCachesFactory>>),
    Locate {
        address: Address,
        tx: oneshot::Sender<Result<ResourceRecord, Error>>,
    },
    ExchangeStarMessage {
        proto: ProtoStarMessage,
        expect: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Error>>,
        description: String,
    },
    Notify(ProtoRequest),
    Exchange{request: ProtoRequest, tx: oneshot::Sender<Result<Response,Error>>},
    Watch{ selector: WatchResourceSelector, tx: oneshot::Sender<Result<Watcher,Error>> },
    StarSearch{ star_pattern: StarPattern, tx: oneshot::Sender<Result<SearchHits,Error>>},
    RequestStarAddress { address_template: AddressTemplate, tx: oneshot::Sender<Result<Address,Error>> },

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
            SurfaceCall::ExchangeStarMessage {
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
            SurfaceCall::Locate { address: identifier, tx } => {
                self.skel
                    .resource_locator_api
                    .tx
                    .try_send(ResourceLocateCall::Locate { address: identifier, tx })
                    .unwrap_or_default();
            }
            SurfaceCall::Watch { selector, tx } => {
                let selector = WatchSelector{
                    topic: Topic::Resource(selector.resource),
                    property: selector.property
                };

                let listener = self.skel.watch_api.listen( selector ).await;
println!("SurfaceApi: go watch listener {}",listener.is_ok());
                tx.send(listener);
            }
            SurfaceCall::StarSearch { star_pattern, tx } => {
                tx.send(self.skel.star_search_api.search( star_pattern ).await);
            }
            SurfaceCall::RequestStarAddress { .. } => {}
            SurfaceCall::Notify(request) => {
                self.skel.messaging_api.notify(request).await;
            }
            SurfaceCall::Exchange { request, tx } => {
                tx.send(self.skel.messaging_api.exchange(request).await.into());
            }
        }
    }
}

impl SurfaceComponent {}
