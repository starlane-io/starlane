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
use mesh_portal_api_server::{MuxCall, PortalAssignRequestHandler, Router};
use mesh_portal_serde::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal_serde::version::latest::config::{Assign, Config, ResourceConfigBody};
use mesh_portal_serde::version::latest::entity::request::create::{AddressSegmentTemplate, AddressTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal_serde::version::latest::entity::request::{Rc};
use mesh_portal_serde::version::latest::id::{Address, AddressSegment, RouteSegment};
use mesh_portal_serde::version::latest::messaging::{Message, Request};
use mesh_portal_serde::version::latest::payload::{Payload, PayloadMap, Primitive};
use mesh_portal_serde::version::latest::portal::inlet::AssignRequest;
use mesh_portal_serde::version::latest::resource::ResourceStub;
use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter};
use mesh_portal_tcp_server::{ClientIdent, PortalServer, PortalTcpServer, TcpServerCall};
use nom::AsBytes;
use crate::artifact::ArtifactRef;
use crate::cache::ArtifactItem;
use crate::html::HTML;
use regex::Regex;
use crate::resource::ArtifactKind;
use serde::{Serialize,Deserialize};


lazy_static! {
    pub static ref STARLANE_MECHTRON_PORT: usize = std::env::var("STARLANE_MECHTRON_PORT").unwrap_or("4345".to_string()).parse::<usize>().unwrap_or(4345);
}

pub struct MechtronStarVariant {
    skel: StarSkel,
    server: Option<mpsc::Sender<TcpServerCall>>
}

impl MechtronStarVariant {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<VariantCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), server: Option::None }),
            skel.variant_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<VariantCall> for MechtronStarVariant {
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

impl MechtronStarVariant {
    fn init(&mut self) -> Result<(),Error> {
        let server = PortalTcpServer::new(STARLANE_MECHTRON_PORT.clone(), Box::new(MechtronPortalServer::new(self.skel.clone() )));
        self.server = Option::Some(server);
        Ok(())
    }
}

pub struct MechtronPortalServer {
    pub skel: StarSkel,
    pub request_assign_handler: Arc<dyn PortalAssignRequestHandler>
}

impl MechtronPortalServer {
    pub fn new(skel: StarSkel) -> Self {
        Self {
            skel: skel.clone(),
            request_assign_handler: Arc::new(MechtronPortalAssignRequestHandler::new(skel) )
        }
    }
}

#[async_trait]
impl PortalServer for MechtronPortalServer {
    fn flavor(&self) -> String {
        "mechtron".to_string()
    }

    fn logger(&self) -> fn(&str) {
        test_logger
    }

    fn router_factory(&self, mux_tx: mpsc::Sender<MuxCall>) -> Box<dyn Router> {
        Box::new(MechtronStarRouter { skel: self.skel.clone() })
    }

    fn portal_request_handler(&self) -> Arc<dyn PortalAssignRequestHandler> {
        self.request_assign_handler.clone()
    }
}


fn test_logger(message: &str) {
    println!("{}", message);
}

pub struct MechtronStarRouter {
    skel: StarSkel
}

impl Router for MechtronStarRouter {
    fn route(&self, message: Message) {
        self.skel.messaging_api.message(message);
    }
}

#[derive(Debug)]
pub struct MechtronPortalAssignRequestHandler {
    skel: StarSkel
}

impl MechtronPortalAssignRequestHandler {
    pub fn new(skel: StarSkel)-> Self {
        MechtronPortalAssignRequestHandler {
            skel
        }
    }
}

#[async_trait]
impl PortalAssignRequestHandler for MechtronPortalAssignRequestHandler {

}
