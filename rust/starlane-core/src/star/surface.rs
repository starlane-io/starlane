use std::sync::Arc;
use mesh_portal::version::latest::entity::request::create::{PointTemplate, Template};
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Message, Request, Response};
use mesh_portal::version::latest::particle::Stub;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;

use crate::cache::ProtoArtifactCachesFactory;
use crate::error::Error;
use crate::frame::{StarMessagePayload, StarPattern};
use crate::message::{ProtoStarMessage, ReplyKind, Reply, ProtoStarMessageTo};
use crate::particle::{ParticleRecord};
use crate::star::{StarCommand, StarSkel, StarInfo};
use crate::star::shell::locator::ResourceLocateCall;
use crate::star::shell::message::MessagingCall;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::watch::{WatchSelector, Notification, Topic, Watch, WatchResourceSelector, Watcher};
use crate::star::shell::search::SearchHits;

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

    pub async fn create_sys_resource( &self, template: Template, messenger_tx: mpsc::Sender<Message> ) -> Result<Stub,Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(SurfaceCall::CreateSysResource {template, messenger_tx, tx }).await;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    pub async fn locate(&self, address: Point) -> Result<ParticleRecord, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(SurfaceCall::Locate { address: address, tx })?;
        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    pub fn notify( &self, mut request: Request ) {
        self.tx.try_send(SurfaceCall::Notify(request));
    }

    pub async fn exchange( &self, request: Request ) -> Response {
        let (tx,rx) = oneshot::channel();
        self.tx.send( SurfaceCall::Request {request:request.clone(), tx }).await;
        match rx.await {
            Ok(response) => response,
            Err(err) => {
                request.fail(err.to_string().as_str() )
            }
        }
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
        address: Point,
        tx: oneshot::Sender<Result<ParticleRecord, Error>>,
    },
    ExchangeStarMessage {
        proto: ProtoStarMessage,
        expect: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Error>>,
        description: String,
    },
    Notify(Request),
    Watch{ selector: WatchResourceSelector, tx: oneshot::Sender<Result<Watcher,Error>> },
    StarSearch{ star_pattern: StarPattern, tx: oneshot::Sender<Result<SearchHits,Error>>},
    RequestStarAddress { address_template: PointTemplate, tx: oneshot::Sender<Result<Point,Error>> },
    CreateSysResource{template:Template, messenger_tx: mpsc::Sender<Message>, tx:oneshot::Sender<Result<Stub,Error>>},
    Request{ request: Request, tx:oneshot::Sender<Response>}
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
            SurfaceCall::CreateSysResource{template,messenger_tx, tx} => {
                tx.send(self.skel.sys_api.create(template, messenger_tx).await);
            }
            SurfaceCall::Request { request, tx  } => {
                let skel = self.skel.clone();
                tokio::spawn ( async move {
                     tx.send(skel.messaging_api.request(request).await);
                });
            }
        }
    }
}

impl SurfaceComponent {}
