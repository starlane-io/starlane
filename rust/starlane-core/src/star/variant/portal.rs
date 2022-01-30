use std::str::FromStr;

use std::thread;


use url::Url;

use crate::star::{StarSkel};
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
use mesh_portal_api_server::{MuxCall, PortalRequestHandler, Router};
use mesh_portal_serde::version::latest::command::common::StateSrc;
use mesh_portal_serde::version::latest::config::{Assign, Config, ResourceConfigBody};
use mesh_portal_serde::version::latest::entity::request::create::{AddressSegmentTemplate, AddressTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal_serde::version::latest::entity::request::{Rc, RcCommand, ReqEntity};
use mesh_portal_serde::version::latest::entity::response::RespEntity;
use mesh_portal_serde::version::latest::id::{Address, AddressSegment, RouteSegment};
use mesh_portal_serde::version::latest::messaging::{Message, Request};
use mesh_portal_serde::version::latest::payload::{Payload, PayloadMap, Primitive};
use mesh_portal_serde::version::latest::portal::inlet::AssignRequest;
use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter};
use mesh_portal_tcp_server::PortalServer;
use nom::AsBytes;
use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::html::HTML;
use regex::Regex;
use crate::resource::ArtifactKind;
use crate::resources::message::ProtoRequest;
use serde::{Serialize,Deserialize};


pub struct PortalVariant {
    skel: StarSkel,
}

impl PortalVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone() }),
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
//                self.init_web(tx);
            }
            VariantCall::Frame { frame, session:_, tx } => {
                tx.send(FrameVerdict::Handle(frame));
            }
        }
    }
}

impl PortalVariant {
    fn init(&self) {

    }
}
pub struct StarlanePortalServer {
    pub skel: StarSkel,
    pub request_handler: Arc<dyn PortalRequestHandler>
}

impl StarlanePortalServer {
    pub fn new(skel: StarSkel) -> Self {
        Self {
            skel: skel.clone(),
            request_handler: Arc::new(StarlanePortalRequestHandler::new(skel) )
        }
    }
}

#[async_trait]
impl PortalServer for StarlanePortalServer {
    fn flavor(&self) -> String {
        "starlane".to_string()
    }

    async fn auth(
        &self,
        reader: &mut PrimitiveFrameReader,
        writer: &mut PrimitiveFrameWriter,
    ) -> Result<String, anyhow::Error> {
        let username = reader.read_string().await?;
        Ok(username)
    }

    fn logger(&self) -> fn(&str) {
        test_logger
    }

    fn router_factory(&self, mux_tx: mpsc::Sender<MuxCall>) -> Box<dyn Router> {
        Box::new(StarlaneRouter{ skel: self.skel.clone() })
    }

    fn portal_request_handler(&self) -> Arc<dyn PortalRequestHandler> {
        self.request_handler.clone()
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
pub struct StarlanePortalRequestHandler {
    skel: StarSkel
}

impl StarlanePortalRequestHandler {
    pub fn new(skel: StarSkel)-> Self {
        StarlanePortalRequestHandler {
            skel
        }
    }
}

#[async_trait]
impl PortalRequestHandler for StarlanePortalRequestHandler {
    async fn handle_assign_request(&self, request: AssignRequest) -> Result<Assign, anyhow::Error> {
        match request {
            AssignRequest::Control => {
                let create = Create {
                    template: Template {
                        address: AddressTemplate { parent: Address { route: RouteSegment::Resource, segments: vec![AddressSegment::Space("space".to_string())] }, child_segment_template: AddressSegmentTemplate::Pattern("control-%".to_string()) },
                        kind: KindTemplate {
                            resource_type: "Control".to_string(),
                            kind: None,
                            specific: None
                        }
                    },
                    state: StateSrc::Stateless,
                    properties: PayloadMap { map: Default::default() },
                    strategy: Strategy::HostedBy(self.skel.info.key.to_string()),
                    registry: Default::default()
                };

                let entity = ReqEntity::Rc( Rc {
                    command: RcCommand::Create(create),
                    payload: Payload::Empty
                });

                let request = Request::new( entity, _, Address::from_str("space")?);

                let mut proto = ProtoRequest::new();
                proto.entity(entity);
                proto.to(Address::from_str("space")?);
                let response = self.skel.messaging_api.exchange(proto).await?;

                match response.entity {
                    RespEntity::Ok(Payload::Primitive(Primitive::Stub(stub))) => {
                        let config = Config {
                            address: Address::from_str("space:artifacts:1.0.0:/control.config")?,
                            body: ResourceConfigBody::Control
                        };
                        let assign = Assign{
                            stub: stub.try_into()?,
                            config
                        };
                        Ok(assign)
                    }
                    RespEntity::Ok(_) => {
                        Err("Unexpected response:  Expected Resource Stub".into())
                    }
                    RespEntity::Fail(fail) => {
                        Err(fail.to_string().into())
                    }
                }
            }
        }
    }
}
