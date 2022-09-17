#![allow(warnings)]

use std::io;
use cosmic_universe::error::{UniErr, StatusErr};
use cosmic_universe::frame2::frame::PrimitiveFrame;
use cosmic_universe::id2::id::{};
use cosmic_universe::log::PointLogger;
use cosmic_universe::substance2::substance::Substance;
use cosmic_universe::hyper::{Knock, HyperSubstance};
use cosmic_universe::wave::{DirectedCore, DirectedProto, Pong, HypMethod, UltraWave, Wave};
use cosmic_universe::VERSION;
use cosmic_hyperlane::{HyperGate, HyperGateSelector, VersionGate};
use quinn::{
    ClientConfig, Connecting, Connection, Endpoint, NewConnection, RecvStream, ServerConfig, VarInt,
};
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use rustls::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use cosmic_universe::id::{Point, ToPort};

fn generate_self_signed_cert() -> Result<(rustls::Certificate, rustls::PrivateKey), UniErr> {
    let cert = rcgen::generate_simple_self_signed(vec!["cosmic-hyperlane".to_string()])?;
    let key = rustls::PrivateKey(cert.serialize_private_key_der());
    Ok((rustls::Certificate(cert.serialize_der()?), key))
}

fn configure_client() -> ClientConfig {
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_no_client_auth();

    ClientConfig::new(Arc::new(crypto))
}

// Implementation of `ServerCertVerifier` that verifies everything as trustworthy.
struct SkipServerVerification;

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::Certificate,
        _intermediates: &[rustls::Certificate],
        _server_name: &rustls::ServerName,
        _scts: &mut dyn Iterator<Item = &[u8]>,
        _ocsp_response: &[u8],
        _now: std::time::SystemTime,
    ) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

pub enum HyperServerCall {}

pub struct HyperServerQuic {
    pub endpoint: Endpoint,
}

impl HyperServerQuic {
    pub async fn new(addr: SocketAddr, gate: Arc<VersionGate>) -> Result<Self, QuicErr> {
        let (cert, key) = generate_self_signed_cert()?;
        let server_config = ServerConfig::with_single_cert(vec![cert], key)?;
        let (endpoint, mut incoming) = Endpoint::server(server_config, addr)?;

        tokio::spawn(async move {
            while let Poll::Ready(Some(conn)) = incoming.poll_next().await {
                let gate = gate.clone();
                tokio::spawn(async move {
                    async fn connect(
                        conn: Connecting,
                        gate: Arc<VersionGate>,
                    ) -> Result<
                        (
                            NewConnection,
                            mpsc::Sender<UltraWave>,
                            mpsc::Receiver<UltraWave>,
                        ),
                        ConErr,
                    > {
                        let mut connection: NewConnection = conn.await?;
                        let recv = tokio::time::timeout(
                            Duration::from_secs(30),
                            connection.uni_streams.next(),
                        )
                        .await?
                        .ok_or(UniErr::server_error())??;
                        let version = recv.read_to_end(2 * 1024).await?;
                        let version = PrimitiveFrame::from(version);
                        let version = version.try_into()?;
                        let entry_router = match gate.unlock(version).await {
                            Ok(entry_router) => {
                                let mut send = connection.connection.open_uni().await?;
                                let ok = PrimitiveFrame::from("Ok".to_string());
                                send.write_all(ok.data.as_bytes()).await?;
                                send.finish().await?;
                                entry_router
                            }
                            Err(err) => {
                                let mut send = connection.connection.open_uni().await?;
                                let frame = PrimitiveFrame::from(err);
                                send.write_all(frame.data.as_bytes()).await?;
                                send.finish().await?;
                                /// send an error and disconnect
                                return Err(ConErr::new());
                            }
                        };

                        let recv = tokio::time::timeout(
                            Duration::from_secs(30),
                            connection.uni_streams.next(),
                        )
                        .await?
                        .ok_or(UniErr::server_error())??;
                        let req = recv.read_to_end(32 * 1024).await?;
                        let req = PrimitiveFrame::from(req);
                        let req = req.try_into()?;
                        let stub = req.as_stub();
                        match entry_router.knock(req).await {
                            Ok((tx, rx)) => {
                                let mut send = connection.connection.open_uni().await?;
                                let resp = stub.ok();
                                let frame = PrimitiveFrame::from(resp);
                                send.write_all(frame.data.as_bytes()).await?;
                                send.finish().await?;
                                Ok((connection, tx, rx))
                            }
                            Err(err) => {
                                let mut send = connection.connection.open_uni().await?;
                                let frame = PrimitiveFrame::from(err);
                                send.write_all(frame.data.as_bytes()).await?;
                                send.finish().await?;
                                /// send a response and disconnect
                                Err(ConErr::new())
                            }
                        }
                    }

                    match connect(conn, gate).await {
                        Ok((connection, tx, mut rx)) => {
                            let uni_streams = connection.uni_streams;
                            let connection = connection.connection;
                            tokio::spawn(async move {
                                while let Poll::Ready(Some(recv)) = uni_streams.next().await {
                                    let wave = recv.read_to_end(32 * 1024).await?;
                                    let wave = PrimitiveFrame::from(wave);
                                    let wave = wave.try_into()?;
                                    tx.send(wave).await;
                                }
                            });

                            tokio::spawn(async move {
                                while let Some(wave) = rx.recv().await {
                                    let mut send = connection.open_uni().await?;
                                    let wave = PrimitiveFrame::from(wave);
                                    send.write_all(wave.data.as_bytes()).await?;
                                    send.finish().await;
                                }
                            });
                        }
                        Err(_) => {
                            // nothing to do here.
                        }
                    }
                });
                // Save connection somewhere, start transferring, receiving data, see DataTransfer tutorial.
            }
        });

        Ok(Self { endpoint })
    }

    pub fn close(self) {
        self.endpoint.close(
            VarInt::from_u64(0u64).unwrap(),
            "no reason given".as_bytes(),
        )
    }
}

pub struct QuicErr {
    pub message: String
}

impl QuicErr {
    pub fn new<S:ToString>(m:S) -> Self {
        Self {
            message: m.to_string()
        }
    }
}

impl ToString for QuicErr {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl From<rustls::Error> for QuicErr {
    fn from(err: Error) -> Self {
        QuicErr::new(err.to_string())
    }
}


impl From<io::Error> for QuicErr {
    fn from(err: io::Error) -> Self {
        QuicErr::new(err.to_string())
    }
}

impl From<UniErr> for QuicErr {
    fn from(err: UniErr) -> Self {
        QuicErr::new(err.to_string())
    }
}

pub struct HyperClientQuic {
    endpoint: Endpoint,
    connection: Connection,
}

impl HyperClientQuic {
    pub async fn new(
        endpoint: Endpoint,
        server_addr: SocketAddr,
        knock: Knock,
        deliver_tx: mpsc::Sender<UltraWave>,
        logger: PointLogger,
    ) -> Result<Self, UniErr> {
        // Connect to the server passing in the server name which is supposed to be in the server certificate.
        let new_connection = endpoint.connect(server_addr, "cosmic-hyperlane")?.await?;

        let mut send = new_connection.connection.open_uni().await?;
        let version = PrimitiveFrame::from(VERSION.clone());
        send.write_all(version.data.as_bytes()).await?;
        send.finish().await?;

        let recv = tokio::time::timeout(Duration::from_secs(30), new_connection.uni_streams.next())
            .await?
            .ok_or(UniErr::server_error())??;
        recv.read_to_end(1024).await?;
        // let's hope it said 'Ok' ...

        let req = knock.into();
        let req = req.try_into()?;

        let mut send = new_connection.connection.open_uni().await?;
        send.write_all(req.data.as_bytes()).await?;
        send.finish().await?;

        let recv = tokio::time::timeout(Duration::from_secs(30), new_connection.uni_streams.next())
            .await?
            .ok_or(UniErr::server_error())??;
        let resp = recv.read_to_end(1024).await?;
        let resp: Pong = resp.try_into()?;

        if !resp.core.is_ok() {
            Err(UniErr::from_status(resp.core.status.as_u16()))
        } else {
            let connection = new_connection.connection;
            let uni_streams = new_connection.uni_streams;

            tokio::spawn(async move {
                while let Some(Ok(recv)) = uni_streams.next().await {
                    async fn process(
                        recv: RecvStream,
                        delivery_tx: mpsc::Sender<UltraWave>,
                    ) -> Result<(), UniErr> {
                        let wave = recv.read_to_end(32 * 1024).await?;
                        let wave = PrimitiveFrame::from(wave);
                        let wave = wave.try_into()?;
                        delivery_tx.send(wave).await;
                    }
                    if let Err(err) = process(recv, deliver_tx.clone()).await {
                        logger.error(err);
                    }
                }
            });

            Ok(Self {
                endpoint,
                connection,
            })
        }
    }

    pub async fn send(&self, wave: UltraWave) -> Result<(), UniErr> {
        let wave: PrimitiveFrame = wave.try_into()?;
        let mut send = self.connection.open_uni().await?;
        send.write_all(wave.data.as_bytes()).await?;
        send.finish().await?;
        Ok(())
    }
}

pub struct ConErr {}

impl ConErr {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::sync::Arc;
    use dashmap::DashMap;
    use cosmic_universe::error::UniErr;
    use cosmic_hyperlane::{HyperGateSelector, VersionGate};
    use crate::HyperServerQuic;

    #[tokio::test]
    pub async fn test() -> Result<(), UniErr> {
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 4343);
        let gate = Arc::new(VersionGate::new(HyperGateSelector::new(Arc::new(DashMap::new()))));
        let server = HyperServerQuic::new( addr, gate).await?;
    }
}
