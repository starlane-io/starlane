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
use httparse::{Request, Header};
use std::sync::Arc;
use std::convert::TryInto;
use handlebars::Handlebars;
use serde_json::json;
use std::future::Future;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use anyhow::anyhow;
use mesh_portal_api::message::Message;
use mesh_portal_api_server::{MuxCall, PortalRequestHandler, Router};
use mesh_portal_serde::version::v0_0_1::config::{Assign, Config, ResourceConfigBody};
use mesh_portal_serde::version::v0_0_1::generic::entity::request::ReqEntity;
use mesh_portal_serde::version::v0_0_1::generic::portal::inlet::AssignRequest;
use mesh_portal_serde::version::v0_0_1::generic::resource::command::create::AddressSegmentTemplate;
use mesh_portal_serde::version::v0_0_1::id::RouteSegment;
use mesh_portal_serde::version::v0_0_1::resource::ResourceStub;
use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter};
use mesh_portal_tcp_server::PortalServer;
use nom::AsBytes;
use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::html::HTML;
use regex::Regex;
use crate::mesh::serde::entity::request::{Http, Rc};
use crate::mesh::serde::payload::{Payload, PayloadMap, Primitive, RcCommand};
use crate::resource::ArtifactKind;
use crate::resources::message::ProtoRequest;
use serde::{Serialize,Deserialize};
use crate::mesh::serde::entity::response::RespEntity;
use crate::mesh::serde::id::{Address, AddressSegment, KindParts, Meta};
use crate::mesh::serde::resource::{Status};
use crate::mesh::serde::resource::command::common::StateSrc;
use crate::mesh::serde::resource::command::create::{AddressTemplate, Create, KindTemplate, Strategy, Template};


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
            skel,
            request_handler: Arc::new(StarlanePortalRequestHandler::new() )
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
    async fn handle_assign_request(&self, request: AssignRequest) -> Result<Assign, Error> {
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
