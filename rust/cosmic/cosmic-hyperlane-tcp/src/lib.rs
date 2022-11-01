#![allow(warnings)]

#[macro_use]
extern crate async_trait;

use std::io::{Empty, Read};
use std::net::{SocketAddr, ToSocketAddrs};
use std::pin::Pin;
use std::str::FromStr;
use std::string::FromUtf8Error;
use std::sync::Arc;
use std::time::Duration;

use openssl::error::ErrorStack;
use openssl::ssl::{Ssl, SslAcceptor, SslConnector, SslConnectorBuilder, SslFiletype, SslMethod};
use rcgen::{generate_simple_self_signed, Certificate, RcgenError};
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::error::Elapsed;
use tokio_openssl::SslStream;

use cosmic_hyperlane::{
    HyperConnectionDetails, HyperConnectionStatus, HyperGate, HyperGateSelector, HyperwayEndpoint,
    HyperwayEndpointFactory, VersionGate,
};
use cosmic_space::err::SpaceErr;
use cosmic_space::hyper::Knock;
use cosmic_space::log::PointLogger;
use cosmic_space::substance::Substance;
use cosmic_space::wave::{Ping, UltraWave, Wave};
use cosmic_space::VERSION;
pub struct HyperlaneTcpClient {
    host: String,
    cert_dir: String,
    knock: Knock,
    logger: PointLogger,
    verify: bool,
}

impl HyperlaneTcpClient {
    pub fn new<H, S>(host: H, cert_dir: S, knock: Knock, verify: bool, logger: PointLogger) -> Self
    where
        S: ToString,
        H: ToString,
    {
        Self {
            host: host.to_string(),
            cert_dir: cert_dir.to_string(),
            knock,
            verify,
            logger,
        }
    }
}

#[async_trait]
impl HyperwayEndpointFactory for HyperlaneTcpClient {
    async fn create(
        &self,
        status_tx: mpsc::Sender<HyperConnectionDetails>,
    ) -> Result<HyperwayEndpoint, SpaceErr> {
        let mut connector: SslConnectorBuilder =
            SslConnector::builder(SslMethod::tls()).map_err(SpaceErr::map)?;

        connector
            .set_ca_file(format!("{}/cert.pem", self.cert_dir))
            .map_err(SpaceErr::map)?;

        let ssl = connector
            .build()
            .configure()
            .map_err(SpaceErr::map)?
            .verify_hostname(self.verify)
            .into_ssl(self.host.as_str())
            .map_err(SpaceErr::map)?;

        let stream = TcpStream::connect(&self.host).await?;

        let mut stream = SslStream::new(ssl, stream).map_err(SpaceErr::map)?;
        Pin::new(&mut stream)
            .connect()
            .await
            .map_err(SpaceErr::map)?;
        let mut stream = FrameStream::new(stream);

        let endpoint =
            FrameMuxer::handshake(stream, status_tx.clone(), self.logger.clone()).await?;

        let wave: Wave<Ping> = self.knock.clone().into();
        let wave = wave.to_ultra();
        endpoint.tx.send(wave).await?;

        Ok(endpoint)
    }
}

pub struct CertGenerator {
    certs: String,
    key: String,
}

impl CertGenerator {
    pub fn gen(subject_alt_names: Vec<String>) -> Result<Self, RcgenError> {
        let cert = generate_simple_self_signed(subject_alt_names)?;
        let certs = cert.serialize_pem()?;
        let key = cert.serialize_private_key_pem();
        Ok(Self { certs, key })
    }

    pub async fn read_from_dir(dir: String) -> Result<Self, Error> {
        let mut contents = vec![];
        let mut certs = File::open(format!("{}/cert.pem", dir)).await?;
        certs.read_to_end(&mut contents).await?;
        let certs = String::from_utf8(contents)?;

        let mut contents = vec![];
        let mut key = File::open(format!("{}/key.pem", dir)).await?;
        key.read_to_end(&mut contents).await?;
        let key = String::from_utf8(contents)?;

        Ok(Self { certs, key })
    }

    pub fn certs(&self) -> String {
        self.certs.clone()
    }

    pub fn private_key(&self) -> String {
        self.key.clone()
    }

    pub async fn write_to_dir(&self, dir: String) -> io::Result<()> {
        let mut certs = File::create(format!("{}/cert.pem", dir)).await?;
        certs.write_all(self.certs().as_bytes()).await?;
        let mut key = File::create(format!("{}/key.pem", dir)).await?;
        key.write_all(self.private_key().as_bytes()).await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Frame {
    pub data: Vec<u8>,
}

impl Frame {
    pub fn from_string(string: String) -> Frame {
        Frame {
            data: string.as_bytes().to_vec(),
        }
    }

    pub fn to_string(self) -> Result<String, SpaceErr> {
        Ok(String::from_utf8(self.data)?)
    }

    pub fn from_version(version: &semver::Version) -> Frame {
        Frame {
            data: version.to_string().as_bytes().to_vec(),
        }
    }

    pub fn to_version(self) -> Result<semver::Version, SpaceErr> {
        Ok(semver::Version::from_str(
            String::from_utf8(self.data)?.as_str(),
        )?)
    }

    pub async fn from_stream<'a>(read: &'a mut SslStream<TcpStream>) -> Result<Frame, SpaceErr> {
        let size = read.read_u32().await? as usize;
        let mut data = Vec::with_capacity(size as usize);

        while data.len() < size {
            read.read_buf(&mut data).await?;
        }
        Ok(Self { data })
    }

    pub async fn to_stream<'a>(&self, write: &'a mut SslStream<TcpStream>) -> Result<(), SpaceErr> {
        write.write_u32(self.data.len() as u32).await?;
        write.write_all(self.data.as_slice()).await?;
        Ok(())
    }

    pub fn to_wave(self) -> Result<UltraWave, SpaceErr> {
        Ok(bincode::deserialize(self.data.as_slice())?)
    }

    pub fn from_wave(wave: UltraWave) -> Result<Self, SpaceErr> {
        Ok(Self {
            data: bincode::serialize(&wave)?,
        })
    }
}

pub struct FrameMuxer {
    stream: FrameStream,
    tx: mpsc::Sender<UltraWave>,
    rx: mpsc::Receiver<UltraWave>,
    terminate_rx: mpsc::Receiver<()>,
    logger: PointLogger,
}
impl FrameMuxer {
    pub async fn handshake(
        mut stream: FrameStream,
        status_tx: mpsc::Sender<HyperConnectionDetails>,
        logger: PointLogger,
    ) -> Result<HyperwayEndpoint, SpaceErr> {
        stream.write_version(&VERSION.clone()).await?;
        let in_version =
            tokio::time::timeout(Duration::from_secs(30), stream.read_version()).await??;

        if in_version == *VERSION {
            //            logger.info("version match");

            stream.write_string("Ok".to_string()).await?;
        } else {
            logger.warn("version mismatch");
            status_tx
                .send(HyperConnectionDetails::new(
                    HyperConnectionStatus::Handshake,
                    "version mismatch",
                ))
                .await?;
            let msg = format!(
                "Err(\"expected version {}. encountered version {}\")",
                VERSION.to_string(),
                in_version.to_string()
            );
            stream.write_string(msg.clone()).await?;
            return Err(msg.into());
        }

        let result = tokio::time::timeout(Duration::from_secs(30), stream.read_string()).await??;
        if "Ok".to_string() != result {
            return logger.result(Err(format!(
                "remote did not indicate Ok. expected: 'Ok' encountered '{}'",
                result
            )
            .into()));
        }

        Ok(Self::new(stream, logger))
    }

    pub fn new(stream: FrameStream, logger: PointLogger) -> HyperwayEndpoint {
        let (in_tx, in_rx) = mpsc::channel(1024);
        let (out_tx, out_rx) = mpsc::channel(1024);
        let (terminate_tx, mut terminate_rx) = mpsc::channel(1);
        let mut muxer = Self {
            stream,
            tx: in_tx,
            rx: out_rx,
            terminate_rx,
            logger: logger.clone(),
        };
        {
            let logger = logger.clone();
            tokio::spawn(async move {
                logger.result(muxer.mux().await).unwrap();
            });
        }

        let (oneshot_terminate_tx, mut oneshot_terminate_rx) = oneshot::channel();
        tokio::spawn(async move {
            oneshot_terminate_rx.await.unwrap_or_default();
            terminate_tx.send(()).await.unwrap_or_default();
        });
        HyperwayEndpoint::new_with_drop(out_tx, in_rx, oneshot_terminate_tx, logger)
    }

    pub async fn mux(mut self) -> Result<(), SpaceErr> {
        loop {
            tokio::select! {
                            wave = self.rx.recv() => {
                                match wave {
                                    None => {
                                        self.logger.warn("rx discon");
                                        break
                                    },
                                    Some(wave) => {
                                       self.stream.write_wave(wave.clone()).await?;
                                    }
                                }
                            }
                            wave = self.stream.read_wave() => {
                                match wave {
                                   Ok(wave) => {
                                self.tx.send(wave).await?;
                                   },
                                   Err(err) => {
            //                          self.logger.error(format!("read stream err: {}",err.to_string()));
                                      break
                                   }
                                }
                            }
                            _ = self.terminate_rx.recv() => {
                                 self.logger.warn(format!("terminated"));
                                 return Ok(())
                                }
                        }
        }
        Ok(())
    }
}

pub struct FrameStream {
    stream: SslStream<TcpStream>,
}

impl FrameStream {
    pub fn new(stream: SslStream<TcpStream>) -> Self {
        Self { stream }
    }

    pub async fn frame(&mut self) -> Result<Frame, SpaceErr> {
        Frame::from_stream(&mut self.stream).await
    }

    pub async fn read_version(&mut self) -> Result<semver::Version, SpaceErr> {
        self.frame().await?.to_version()
    }

    pub async fn read_string(&mut self) -> Result<String, SpaceErr> {
        self.frame().await?.to_string()
    }

    pub async fn read_wave(&mut self) -> Result<UltraWave, SpaceErr> {
        self.frame().await?.to_wave()
    }

    pub async fn write_frame(&mut self, frame: Frame) -> Result<(), SpaceErr> {
        frame.to_stream(&mut self.stream).await
    }

    pub async fn write_string(&mut self, string: String) -> Result<(), SpaceErr> {
        self.write_frame(Frame::from_string(string)).await
    }

    pub async fn write_version(&mut self, version: &semver::Version) -> Result<(), SpaceErr> {
        self.write_frame(Frame::from_version(version)).await
    }

    pub async fn write_wave(&mut self, wave: UltraWave) -> Result<(), SpaceErr> {
        self.write_frame(Frame::from_wave(wave)?).await
    }
}

pub struct HyperlaneTcpServerApi {}

impl HyperlaneTcpServerApi {
    pub fn new() -> Self {
        Self {}
    }
}

pub struct HyperlaneTcpServer {
    gate: Arc<HyperGateSelector>,
    listener: TcpListener,
    logger: PointLogger,
    acceptor: SslAcceptor,
    server_kill_tx: broadcast::Sender<()>,
    server_kill_rx: broadcast::Receiver<()>,
}

impl HyperlaneTcpServer {
    pub async fn new(
        port: u16,
        cert_dir: String,
        gate: Arc<HyperGateSelector>,
        logger: PointLogger,
    ) -> Result<Self, Error> {
        let (server_kill_tx, server_kill_rx) = broadcast::channel(1);
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key_file(format!("{}/key.pem", cert_dir), SslFiletype::PEM)?;
        acceptor.set_certificate_chain_file(format!("{}/cert.pem", cert_dir))?;
        let acceptor = acceptor.build();
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
            .await
            .unwrap();

        Ok(Self {
            acceptor,
            gate,
            listener,
            logger,
            server_kill_tx,
            server_kill_rx,
        })
    }

    pub fn start(mut self) -> Result<HyperlaneTcpServerApi, Error> {
        tokio::spawn(async move {
            self.run().await;
        });
        Ok(HyperlaneTcpServerApi::new())
    }

    async fn run(mut self) {
        loop {
            let stream = self.listener.accept().await.unwrap().0;
            let gate = self.gate.clone();
            let logger = self.logger.clone();
            let ssl = match logger.result(Ssl::new(self.acceptor.context())) {
                Ok(ssl) => ssl,
                Err(_) => break,
            };
            let mut server_kill_rx = self.server_kill_tx.subscribe();
            tokio::spawn(async move {
                async fn serve(
                    stream: TcpStream,
                    ssl: Ssl,
                    gate: Arc<HyperGateSelector>,
                    server_kill_rx: broadcast::Receiver<()>,
                    logger: PointLogger,
                ) -> Result<(), Error> {
                    let mut stream = SslStream::new(ssl, stream)?;

                    Pin::new(&mut stream).accept().await?;
                    let mut stream = FrameStream::new(stream);

                    let (status_tx, mut status_rx): (
                        mpsc::Sender<HyperConnectionDetails>,
                        mpsc::Receiver<HyperConnectionDetails>,
                    ) = mpsc::channel(1024);
                    {
                        let logger = logger.clone();
                        tokio::spawn(async move {
                            while let Some(details) = status_rx.recv().await {
                                /*                                logger.info(format!(
                                    "{} | {}",
                                    details.status.to_string(),
                                    details.info
                                ))*/
                            }
                        });
                    }
                    let mut mux = FrameMuxer::handshake(stream, status_tx, logger.clone()).await?;

                    let knock = tokio::time::timeout(Duration::from_secs(30), mux.rx.recv())
                        .await?
                        .ok_or("expected wave")?;
                    let knock = knock.to_directed()?;
                    if let Substance::Knock(knock) = knock.body() {
                        let mut endpoint = gate.knock(knock.clone()).await?;
                        mux.connect(endpoint);
                    } else {
                        let msg = format!(
                            "expected client Substance::Knock(Knock) encountered '{}'",
                            knock.body().kind().to_string()
                        );
                        return logger.result(Err(SpaceErr::str(msg).into()));
                    }

                    Ok(())
                }
                serve(stream, ssl, gate, server_kill_rx, logger).await;
            });
        }
    }
}

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[derive(Debug, Clone)]
pub struct Error {
    pub message: String,
}

impl ToString for Error {
    fn to_string(&self) -> String {
        self.message.clone()
    }
}

impl Error {
    pub fn new<S: ToString>(m: S) -> Self {
        Self {
            message: m.to_string(),
        }
    }
}
impl From<Elapsed> for Error {
    fn from(e: Elapsed) -> Self {
        Self::new(e)
    }
}

impl From<openssl::ssl::Error> for Error {
    fn from(e: openssl::ssl::Error) -> Self {
        Self::new(e)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self {
        Self::new(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::new(e)
    }
}

impl From<SpaceErr> for Error {
    fn from(e: SpaceErr) -> Self {
        Error::new(e)
    }
}

impl From<RcgenError> for Error {
    fn from(e: RcgenError) -> Self {
        Error::new(e)
    }
}

impl From<ErrorStack> for Error {
    fn from(e: ErrorStack) -> Self {
        Error::new(e)
    }
}

impl From<String> for Error {
    fn from(e: String) -> Self {
        Error::new(e)
    }
}

impl From<&str> for Error {
    fn from(e: &str) -> Self {
        Error::new(e)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use cosmic_hyperlane::test_util::{SingleInterchangePlatform, WaveTest, FAE, LESS};
    use cosmic_space::loc::{Point, ToSurface};
    use cosmic_space::log::RootLogger;

    use chrono::DateTime;
    use chrono::Utc;
    use cosmic_hyperlane::HyperClient;
    use cosmic_space::settings::Timeouts;
    use cosmic_space::wave::exchange::asynch::Exchanger;
    use cosmic_space::wave::DirectedProto;

    use super::*;

    #[no_mangle]
    pub extern "C" fn cosmic_uuid() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    #[no_mangle]
    pub extern "C" fn cosmic_timestamp() -> DateTime<Utc> {
        Utc::now()
    }

    //#[tokio::test]
    async fn test_tcp() -> Result<(), Error> {
        let platform = SingleInterchangePlatform::new().await;

        CertGenerator::gen(vec!["localhost".to_string()])?
            .write_to_dir(".".to_string())
            .await?;
        let logger = RootLogger::default();
        let logger = logger.point(Point::from_str("tcp-server")?);
        let port = 4343u16;
        let server =
            HyperlaneTcpServer::new(port, ".".to_string(), platform.gate.clone(), logger.clone())
                .await?;
        let api = server.start()?;

        let less_logger = logger.point(LESS.clone());
        let less_client = Box::new(HyperlaneTcpClient::new(
            format!("localhost:{}", port),
            ".",
            platform.knock(LESS.to_surface()),
            false,
            less_logger,
        ));

        let fae_logger = logger.point(FAE.clone());
        let fae_client = Box::new(HyperlaneTcpClient::new(
            format!("localhost:{}", port),
            ".",
            platform.knock(FAE.to_surface()),
            false,
            fae_logger,
        ));

        let test = WaveTest::new(fae_client, less_client);

        test.go().await.unwrap();

        Ok(())
    }

   // #[tokio::test]
    async fn test_large_frame() -> Result<(), Error> {
        let platform = SingleInterchangePlatform::new().await;

        CertGenerator::gen(vec!["localhost".to_string()])?
            .write_to_dir(".".to_string())
            .await?;
        let logger = RootLogger::default();
        let logger = logger.point(Point::from_str("tcp-server")?);
        let port = 4343u16;
        let server =
            HyperlaneTcpServer::new(port, ".".to_string(), platform.gate.clone(), logger.clone())
                .await?;
        let api = server.start()?;

        let less_logger = logger.point(LESS.clone());
        let less_client = Box::new(HyperlaneTcpClient::new(
            format!("localhost:{}", port),
            ".",
            platform.knock(LESS.to_surface()),
            false,
            less_logger,
        ));

        let less_exchanger = Exchanger::new(
            LESS.push("exchanger").unwrap().to_surface(),
            Timeouts::default(),
            PointLogger::default(),
        );

        let root_logger = RootLogger::default();
        let logger = root_logger.point(Point::from_str("less-client").unwrap());
        let less_client =
            HyperClient::new_with_exchanger(less_client, Some(less_exchanger.clone()), logger)
                .unwrap();

        less_client
            .wait_for_ready(Duration::from_secs(5))
            .await
            .unwrap();
        let transmitter = less_client.transmitter_builder().await.unwrap().build();

        let size = 100_000;
        let mut bin = Vec::with_capacity(100_100);
        for _ in 0..size {
            bin.push(0u8);
        }
        let bin = Arc::new(bin);
        let mut wave = DirectedProto::signal();
        wave.body(Substance::Bin(bin));
        transmitter.signal(wave).await.unwrap();

        Ok(())
    }
}
