use std::str::FromStr;

use std::thread;


use url::Url;

use crate::star::{PortalEvent, StarSkel};
use crate::starlane::api::{StarlaneApi, StarlaneApiRelay};
use tokio::sync::{oneshot, mpsc};
use crate::star::variant::{VariantCall, FrameVerdict};
use crate::util::{AsyncRunner, AsyncProcessor};
use tokio::net::{TcpListener, TcpStream};
use tokio::runtime::Runtime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use crate::error::Error;
use bytes::BytesMut;
use std::sync::Arc;
use std::convert::TryInto;
use handlebars::Handlebars;
use serde_json::json;
use std::future::Future;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use anyhow::anyhow;
use mesh_portal_api_server::{MuxCall, PortalAssignRequestHandler, Router};
use mesh_portal_serde::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal_serde::version::latest::config::{Assign, Config, ResourceConfigBody};
use mesh_portal_serde::version::latest::entity::request::create::{AddressSegmentTemplate, AddressTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal_serde::version::latest::entity::request::Rc;
use mesh_portal_serde::version::latest::id::{Address, AddressSegment, RouteSegment};
use mesh_portal_serde::version::latest::messaging::{Message, Request};
use mesh_portal_serde::version::latest::payload::{Payload, PayloadMap, Primitive};
use mesh_portal_serde::version::latest::portal::inlet::AssignRequest;
use mesh_portal_serde::version::latest::resource::ResourceStub;
use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter};
use mesh_portal_tcp_server::{Event, PortalServer, PortalTcpServer, TcpServerCall};
use nom::AsBytes;
use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::html::HTML;
use regex::Regex;
use crate::resource::ArtifactKind;
use serde::{Serialize,Deserialize};


lazy_static! {
    pub static ref DEFAULT_PORT: usize = std::env::var("STARLANE_PORTAL_PORT").unwrap_or("4344".to_string()).parse::<usize>().unwrap_or(4344);
}

pub struct PortalVariant {
    skel: StarSkel,
    server: Option<mpsc::Sender<TcpServerCall>>
}

impl PortalVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), server: Option::None }),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for PortalVariant {
    async fn process(&mut self, call: VariantCall) {
        match call {
            VariantCall::Init(tx) => {
                tx.send(self.init());
            }
            VariantCall::Frame { frame, session:_, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}

impl PortalVariant {
    fn init(&mut self) -> Result<(),Error> {
        let server = PortalTcpServer::new(DEFAULT_PORT.clone(), Box::new(StarlanePortalServer::new(self.skel.clone() )));
        let skel = self.skel.clone();
        tokio::spawn( async move {
            async fn process(skel: &StarSkel, server: mpsc::Sender<TcpServerCall>) -> Result<(),Error> {
                let (tx,rx) = oneshot::channel();
                server.send( TcpServerCall::ListenEvents(tx)).await;
                let mut server_broadcast_rx = rx.await?;
                while let Ok(event) = server_broadcast_rx.recv().await {
                    match event {
                        Event::Added(portal_info) => {
                            skel.portal_event_tx.send( PortalEvent::Added(portal_info));
                        }
                        Event::Removed(portal_info) => {
                            skel.portal_event_tx.send( PortalEvent::Removed(portal_info));
                        }
                        _ => {}
                    }
                }
                Ok(())
            }

        });
        self.server = Option::Some(server);
        Ok(())
    }
}
pub struct StarlanePortalServer {
    pub skel: StarSkel,
    pub request_assign_handler: Arc<dyn PortalAssignRequestHandler>
}

impl StarlanePortalServer {
    pub fn new(skel: StarSkel) -> Self {
        Self {
            skel: skel.clone(),
            request_assign_handler: Arc::new(StarlanePortalAssignRequestHandler::new(skel) )
        }
    }
}

#[async_trait]
impl PortalServer for StarlanePortalServer {
    fn flavor(&self) -> String {
        "starlane".to_string()
    }


    fn logger(&self) -> fn(&str) {
        test_logger
    }

    fn router_factory(&self, mux_tx: mpsc::Sender<MuxCall>) -> Box<dyn Router> {
        Box::new(StarlaneRouter{ skel: self.skel.clone() })
    }

    fn portal_request_handler(&self) -> Arc<dyn PortalAssignRequestHandler> {
        self.request_assign_handler.clone()
    }
}


fn test_logger(message: &str) {
    println!("{}", message);
}

pub struct StarlaneRouter {
    skel: StarSkel
}

impl Router for StarlaneRouter {
    fn route(&self, message: Message) {
        self.skel.messaging_api.message(message);
    }
}

#[derive(Debug)]
pub struct StarlanePortalAssignRequestHandler {
    skel: StarSkel
}

impl StarlanePortalAssignRequestHandler {
    pub fn new(skel: StarSkel)-> Self {
        StarlanePortalAssignRequestHandler {
            skel
        }
    }
}

#[async_trait]
impl PortalAssignRequestHandler for StarlanePortalAssignRequestHandler {
    async fn handle_assign_request(&self, request: AssignRequest, mux_tx: &mpsc::Sender<MuxCall>) -> Result<Assign, anyhow::Error> {
        match request {
            AssignRequest::Control => {
                let template = Template {
                    address: AddressTemplate { parent: Address { route: RouteSegment::Mesh(self.skel.info.address.to_string()), segments: vec![] }, child_segment_template: AddressSegmentTemplate::Pattern("control-%".to_string()) },
                    kind: KindTemplate {
                        resource_type: "Control".to_string(),
                        kind: None,
                        specific: None
                    }
                };

                let (messenger_tx,mut messenger_rx) = mpsc::channel(1024);

                match self.skel.sys_api.create(template,messenger_tx).await
                {
                    Ok(stub) => {
                        let config = Config {
                            address: stub.address.clone(),
                            body: ResourceConfigBody::Control
                        };
                        let assign = Assign {
                            config,
                            stub
                        };

                        // forward messages to portal muxer
                        {
                            let mux_tx = mux_tx.clone();
                            tokio::spawn(async move {
                                while let Option::Some(message) = messenger_rx.recv().await {
                                    mux_tx.send( MuxCall::MessageOut(message)).await;
                                }
                            });
                        }
                        Ok(assign)
                    }
                    Err(err) => {
                        Err(anyhow!("Error"))
                    }
                }
            }
        }
    }
}
