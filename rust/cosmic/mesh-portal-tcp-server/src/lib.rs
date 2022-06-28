#![allow(warnings)]

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate anyhow;

#[macro_use]
extern crate strum_macros;

use std::convert::{TryFrom, TryInto};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::error::SendTimeoutError;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};

use mesh_portal_api_server::{Portal, PortalEvent, PortalInfo};
use mesh_portal::version::latest::config::PortalConfig;
use mesh_portal::version::latest::frame::CloseReason;
use mesh_portal::version::latest::messaging::ReqShell;
use mesh_portal::version::latest::messaging::{Message, RespShell};
use mesh_portal_tcp_common::{
    FrameReader, FrameWriter, PrimitiveFrameReader, PrimitiveFrameWriter,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::broadcast::Receiver;
use tokio::sync::oneshot::error::RecvError;
use tokio::task::yield_now;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::log::{Log, LogSource, PointlessLog, RootLogBuilder, RootLogger, LogAppender};
use mesh_portal::version::latest::particle::Status;

#[derive(Clone, strum_macros::Display)]
pub enum PortalServerEvent {
    Status(Status),
    ClientConnected,
    FlavorNegotiation(EventResult<String>),
    Authorization(EventResult<String>),
    Shutdown,
}

#[derive(Clone)]
pub enum EventResult<E> {
    Ok(E),
    Err(String),
}

pub enum TcpServerCall {
    GetServerEvents(oneshot::Sender<broadcast::Receiver<PortalServerEvent>>),
    GetPortalEvents(oneshot::Sender<broadcast::Receiver<PortalEvent>>),
    Shutdown,
}

struct Alive {
    pub alive: bool,
}

impl Alive {
    pub fn new() -> Self {
        Self { alive: true }
    }
}

pub trait PointFactory: Send+Sync{
    fn point(&self) -> Point;
}

pub struct PortalTcpServer  {
    portal_config: PortalConfig,
    port: usize,
    server: Arc<dyn PortalServer>,
    server_event_broadcaster_tx: broadcast::Sender<PortalServerEvent>,
    portal_broadcast_tx: broadcast::Sender<PortalEvent>,
    call_tx: mpsc::Sender<TcpServerCall>,
    alive: Arc<Mutex<Alive>>,
    request_handler: Arc<dyn PortalRequestHandler>,
    logger: RootLogger,
    point_factory: Arc<dyn PointFactory>
}

impl PortalTcpServer {
    pub fn new(port: usize, server: Box<dyn PortalServer>, point_factory: Arc<dyn PointFactory> ) -> mpsc::Sender<TcpServerCall> {
        let (call_tx, mut call_rx) = mpsc::channel(1024);
        {
            let call_tx = call_tx.clone();
            let point_factory = point_factory.clone();
            tokio::task::spawn_blocking(move || {
                let server: Arc<dyn PortalServer> = server.into();
                let (server_event_broadcaster_tx, _) = broadcast::channel(32);
                let (portal_broadcast_tx, _) = broadcast::channel(1024);

                let server = Self {
                    request_handler: server.portal_request_handler(),
                    portal_config: Default::default(),
                    port,
                    server,
                    server_event_broadcaster_tx,
                    portal_broadcast_tx,
                    call_tx: call_tx.clone(),
                    alive: Arc::new(Mutex::new(Alive::new())),
                    logger: Default::default(),
                    point_factory
                };

                tokio::spawn(async move {
                    server
                        .server_event_broadcaster_tx
                        .send(PortalServerEvent::Status(Status::Unknown))
                        .unwrap_or_default();
                    {
                        let port = server.port.clone();
                        let server_event_broadcaster_tx = server.server_event_broadcaster_tx.clone();
                        let portal_broadcast_tx = server.portal_broadcast_tx.clone();
                        let alive = server.alive.clone();
                        tokio::spawn(async move {
                            yield_now().await;
                            while let Option::Some(call) = call_rx.recv().await {
                                match call {
                                    TcpServerCall::GetServerEvents(tx) => {
                                        tx.send(server_event_broadcaster_tx.subscribe());
                                    }
                                    TcpServerCall::GetPortalEvents(tx) => {
                                        tx.send(portal_broadcast_tx.subscribe());
                                    }
                                    TcpServerCall::Shutdown => {
                                        server_event_broadcaster_tx
                                            .send(PortalServerEvent::Shutdown)
                                            .unwrap_or_default();
                                        alive.lock().await.alive = false;
                                        match std::net::TcpStream::connect(format!(
                                            "localhost:{}",
                                            port
                                        )) {
                                            Ok(_) => {}
                                            Err(_) => {}
                                        }
                                        return;
                                    }
                                }
                            }
                        });
                    }

                    server.start().await;
                });
            });
        }
        call_tx
    }

    async fn start(self) {
        let addr = format!("localhost:{}", self.port);
        match std::net::TcpListener::bind(addr.clone()) {
            Ok(std_listener) => {
                tokio::time::sleep(Duration::from_secs(0)).await;
                let listener = TcpListener::from_std(std_listener).unwrap();
                self.server_event_broadcaster_tx
                    .send(PortalServerEvent::Status(Status::Ready))
                    .unwrap_or_default();
                tokio::time::sleep(Duration::from_secs(0)).await;
                while let Ok((stream, _)) = listener.accept().await {
                    {
                        if !self.alive.lock().await.alive.clone() {
                            (self.server.logger())("server reached final shutdown");
                            break;
                        }
                    }
                    self.server_event_broadcaster_tx
                        .send(PortalServerEvent::ClientConnected)
                        .unwrap_or_default();
                    (&self).handle(stream).await;
                }
                self.server_event_broadcaster_tx
                    .send(PortalServerEvent::Status(Status::Done))
                    .unwrap_or_default();
            }
            Err(error) => {
                let message = format!("FATAL: could not setup TcpListener {}", error);
                (self.server.logger())(message.as_str());
                self.server_event_broadcaster_tx
                    .send(PortalServerEvent::Status(Status::Panic))
                    .unwrap_or_default();
            }
        }
    }

    async fn handle(&self, stream: TcpStream) -> Result<(), Error> {
        let (reader, writer) = stream.into_split();
        let mut reader = PrimitiveFrameReader::new(reader);
        let mut writer = PrimitiveFrameWriter::new(writer);

        let mut reader: FrameReader<initin::Frame> = FrameReader::new(reader);
        let mut writer: FrameWriter<initout::Frame> = FrameWriter::new(writer);

        if let initin::Frame::Flavor(flavor) = reader.read().await? {
            // first verify flavor matches
            if flavor != self.server.flavor() {
                let message = format!(
                    "ERROR: flavor does not match.  expected '{}'",
                    self.server.flavor()
                );
                println!("{}", message);

                tokio::time::sleep(Duration::from_secs(0)).await;

                self.server_event_broadcaster_tx
                    .send(PortalServerEvent::FlavorNegotiation(EventResult::Err(
                        message.clone(),
                    )))
                    .unwrap_or_default();
                return Err(anyhow!(message));
            } else {
                self.server_event_broadcaster_tx
                    .send(PortalServerEvent::FlavorNegotiation(EventResult::Ok(
                        self.server.flavor(),
                    )))
                    .unwrap_or_default();
            }
        } else {
            let message = format!(
                "ERROR: unexpected frame.  expected flavor '{}'",
                self.server.flavor()
            );
            self.server_event_broadcaster_tx
                .send(PortalServerEvent::FlavorNegotiation(EventResult::Err(
                    message.clone(),
                )))
                .unwrap_or_default();
            return Err(anyhow!(message));
        }

        writer.write(initout::Frame::Ok).await?;
        yield_now().await;

        if let initin::Frame::Auth(portal_auth) = reader.read().await? {
            self.server_event_broadcaster_tx
                .send(PortalServerEvent::Authorization(EventResult::Ok(
                    portal_auth.user.clone(),
                )))
                .unwrap_or_default();
            tokio::time::sleep(Duration::from_secs(0)).await;
            writer.write(initout::Frame::Ok).await?;

            loop {
                match reader.read().await? {
                    initin::Frame::Artifact(request) => {
                        let response = self.server.portal_request_handler().handle_artifact_request(request.item.clone()).await?;
                        let response = request.with(response);
                        writer.write(initout::Frame::Artifact(response)).await?;
println!("portal server: wrote initout::Frame::Artifact");
                    }
                    initin::Frame::Ready => {
                        break;
                    }
                    _ => {
                        return Err(anyhow!("portal server: illegal initin::Frame encountered during client init process") )
                    }
                }
            }

            let mut reader: FrameReader<inlet::Frame> = FrameReader::new(reader.done());
            let mut writer: FrameWriter<outlet::Frame> = FrameWriter::new(writer.done());

            let (outlet_tx, mut outlet_rx) = mpsc::channel(1024);

            let portal_key = match portal_auth.portal_key {
                None => uuid::Uuid::new_v4().to_string(),
                Some(portal_key) => portal_key,
            };

            let info = PortalInfo { portal_key };

            let point_factory = self.point_factory.clone();

            let (portal, inlet_tx) = Portal::new(
                info,
                self.portal_config.clone(),
                outlet_tx,
                self.request_handler.clone(),
                self.portal_broadcast_tx.clone(),
                self.logger.clone(),
                point_factory.point(),
            );


            let portal_api = portal.api();
            self.server.add_portal(portal);
            self.portal_broadcast_tx.send( PortalEvent::PortalAdded(portal_api));

            {
                let logger = self.server.logger();
                tokio::spawn(async move {
                    loop {
                        match  reader.read().await {
                            Ok(frame) => {
                                println!("server TCP READ FRAME: {}", frame.to_string());
                                let result = inlet_tx.send(frame).await;
                                yield_now().await;
                                if result.is_err() {
                                    (logger)("FATAL: cannot send frame to portal inlet_tx");
                                    return;
                                }
                            }
                            Err(err) => {
                                eprintln!("portal server: TCP Reader end... {}",err.to_string());
                                break;
                            }
                        }
                    }
                });
            }

            {
                let logger = self.server.logger();
                let task = tokio::task::spawn_blocking(move || {
                    tokio::spawn(async move {
                        while let Option::Some(frame) = outlet_rx.recv().await {
                            println!(
                                "server... SENDING from outlet_rx frame :==:> {}",
                                frame.to_string()
                            );
                            writer.write(frame).await;
                        }
                        println!("server: outlet_rx complete.");
                    });
                });
                task.await?;
            }
        }
        Ok(())
    }
}

pub struct RouterProxy {
    pub server: Arc<dyn PortalServer>,
}

#[async_trait]
pub trait PortalServer: Sync + Send {
    fn flavor(&self) -> String;

    async fn auth(
        &self,
        reader: &mut PrimitiveFrameReader,
        writer: &mut PrimitiveFrameWriter,
    ) -> Result<PortalAuth, anyhow::Error> {
        let frame = reader.read().await?;
        let client_ident: PortalAuth = bincode::deserialize(frame.data.as_slice())?;
        tokio::time::sleep(Duration::from_secs(0)).await;
        Ok(client_ident)
    }

    fn logger(&self) -> fn(message: &str);
    fn portal_request_handler(&self) -> Arc<dyn PortalRequestHandler>;
    fn add_portal(&self, portal: Portal);
}



#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {

    }
}