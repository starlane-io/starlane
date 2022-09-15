use cosmic_hyperlane::{HyperConnectionDetails, HyperConnectionStatus, HyperGate, HyperGateSelector, HyperwayEndpoint, HyperwayEndpointFactory, VersionGate};
use openssl::error::ErrorStack;
use openssl::ssl::{Ssl, SslAcceptor, SslConnector, SslConnectorBuilder, SslFiletype, SslMethod};
use rcgen::{generate_simple_self_signed, Certificate, RcgenError};
use std::io::{Empty, Read};
use std::net::{SocketAddr, ToSocketAddrs};
use std::pin::Pin;
use std::str::FromStr;
use std::string::FromUtf8Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf, WriteHalf};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio::time::error::Elapsed;
use tokio_openssl::SslStream;
use cosmic_api::error::MsgErr;
use cosmic_api::log::PointLogger;
use cosmic_api::substance::substance::Substance;
use cosmic_api::sys::Knock;
use cosmic_api::VERSION;
use cosmic_api::wave::{Ping, UltraWave, Wave};


#[macro_use]
extern crate async_trait;

pub struct HyperlaneTcpClient {
    host: String,
    cert_dir: String,
    knock: Knock,
    logger: PointLogger,
    verify: bool
}

impl HyperlaneTcpClient {
    pub fn new<H,S>(host: H,
                     cert_dir: S,
                     knock: Knock,
                     verify: bool,
                     logger: PointLogger,
    ) -> Self where S:ToString, H: ToString{
        Self {
            host: host.to_string(),
            cert_dir: cert_dir.to_string(),
            knock,
            verify,
            logger
        }
    }
}

#[async_trait]
impl HyperwayEndpointFactory for HyperlaneTcpClient {
    async fn create(&self, status_tx: mpsc::Sender<HyperConnectionDetails>) -> Result<HyperwayEndpoint, MsgErr> {
        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Connecting,"init")).await.unwrap_or_default();
        let (kill_tx,kill_rx) = broadcast::channel(1);
        let mut connector :SslConnectorBuilder= SslConnector::builder(SslMethod::tls()).map_err(MsgErr::map)?;


        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Connecting,"loading cert")).await.unwrap_or_default();

        connector.set_ca_file(format!("{}/cert.pem", self.cert_dir)).map_err(MsgErr::map)?;

        let ssl =
         connector
            .build()
            .configure().map_err(MsgErr::map)?.verify_hostname(self.verify)
            .into_ssl(self.host.as_str()).map_err(MsgErr::map)?;

        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Connecting,"connecting tcp stream")).await.unwrap_or_default();
        let stream = TcpStream::connect(&self.host).await?;
        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Connecting,"creating ssl layer")).await.unwrap_or_default();
        let mut stream = SslStream::new(ssl, stream).map_err(MsgErr::map)?;
        Pin::new(&mut stream).connect().await.map_err(MsgErr::map)?;
        let mut stream = FrameStream::new( stream );
        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Connecting,"starting handshake")).await.unwrap_or_default();
        let endpoint = FrameMuxer::handshake(stream, kill_rx, status_tx.clone(), self.logger.clone()).await?;
        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Auth,"sending knock")).await.unwrap_or_default();
        let wave: Wave<Ping> = self.knock.clone().into();
        let wave = wave.to_ultra();
        endpoint.tx.send(wave).await.unwrap_or_default();

println!("Returning HyperwayEndpoint");
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
    pub data: Vec<u8>
}

impl Frame {
    pub fn from_string( string: String ) -> Frame {
        Frame{ data: string.as_bytes().to_vec() }
    }

    pub fn to_string(self) -> Result<String,MsgErr> {
        Ok(String::from_utf8(self.data)?)
    }


    pub fn from_version( version: &semver::Version ) -> Frame {
      Frame{ data: version.to_string().as_bytes().to_vec() }
  }

  pub fn to_version( self ) -> Result<semver::Version, MsgErr> {
      Ok(semver::Version::from_str(String::from_utf8(self.data )?.as_str())?)
  }



  pub async fn from_stream<'a>(read: &'a mut SslStream<TcpStream>) -> Result<Frame,MsgErr> {
      let size = read.read_u32().await?;
      let mut data = Vec::with_capacity(size as usize);
      read.read_buf(&mut data).await?;
      Ok(Self {
          data
      })
  }

  pub async fn to_stream<'a>(&self, write: &'a mut SslStream<TcpStream>) -> Result<(),MsgErr> {
      write.write_u32(self.data.len() as u32).await?;
      write.write_all(self.data.as_slice()).await?;
      Ok(())
  }

  pub fn to_wave(self) -> Result<UltraWave,MsgErr> {
      Ok(bincode::deserialize(self.data.as_slice())?)
  }

  pub fn from_wave(wave: UltraWave) -> Result<Self,MsgErr> {
      Ok(Self{data:bincode::serialize(&wave)?})
  }
}



pub struct FrameMuxer {
    stream: FrameStream,
    tx: mpsc::Sender<UltraWave>,
    rx: mpsc::Receiver<UltraWave>,
    terminate_rx: oneshot::Receiver<()>,
    kill_rx: broadcast::Receiver<()>,
    logger: PointLogger
}

impl FrameMuxer {

    pub async fn handshake(mut stream : FrameStream, kill_rx: broadcast::Receiver<()>, status_tx: mpsc::Sender<HyperConnectionDetails>, logger: PointLogger ) -> Result<HyperwayEndpoint,MsgErr> {
        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Handshake, "exchanging versions")).await.unwrap_or_default();
        stream.write_version(&VERSION.clone()).await?;
        let in_version = tokio::time::timeout(Duration::from_secs(30), stream.read_version()).await??;
        logger.info(format!("remote version: {}", in_version.to_string()));

        if in_version == *VERSION {
            logger.info("version match");
            status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Handshake, "version match")).await.unwrap_or_default();
            stream.write_string("Ok".to_string()).await?;
        } else {
            logger.warn("version mismatch");
            status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Handshake, "version mismatch")).await.unwrap_or_default();
            let msg = format!("Err(\"expected version {}. encountered version {}\")", VERSION.to_string(), in_version.to_string());
            stream.write_string(msg.clone()).await?;
            return Err(msg.into())
        }
        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Handshake, "waiting for Ok")).await.unwrap_or_default();
        let result = tokio::time::timeout(Duration::from_secs(30), stream.read_string()).await??;
        if "Ok".to_string() != result {
            return logger.result(Err(format!("remote did not indicate Ok. expected: 'Ok' encountered '{}'", result).into()));
        }

        status_tx.send(HyperConnectionDetails::new(HyperConnectionStatus::Handshake, "handshake complete")).await.unwrap_or_default();
        logger.info("remote signaled Ok");

        Ok(Self::new(stream, kill_rx, logger ))
    }

    pub fn new(stream : FrameStream, kill_rx: broadcast::Receiver<()>, logger: PointLogger ) -> HyperwayEndpoint
    {
        let (in_tx,in_rx) = mpsc::channel(1024);
        let (out_tx,out_rx) = mpsc::channel(1024);
        let (terminate_tx, terminate_rx) = oneshot::channel();
        let mut muxer = Self { stream, tx: in_tx, rx: out_rx, terminate_rx, kill_rx, logger };
        tokio::spawn( async move {
           muxer.mux().await.unwrap_or_default();
        });
        HyperwayEndpoint::new_with_drop(out_tx,in_rx,terminate_tx)
    }

    pub async fn mux(mut self) -> Result<(),MsgErr> {
        loop {
            tokio::select! {
                Some(wave) = self.rx.recv() => {
self.logger.info(format!("Writing wave: {}", wave.desc()));
                    self.stream.write_wave(wave).await?;
                }
                Ok(wave) = self.stream.read_wave() => {
self.logger.info(format!("Reading wave: {}", wave.desc()));
                    self.tx.send(wave).await?;
                }
                _ = self.kill_rx.recv() => {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}

pub struct FrameStream {
    stream: SslStream<TcpStream>
}

impl  FrameStream {
    pub fn new(stream: SslStream<TcpStream>) -> Self {
        Self {
           stream
        }
    }

    pub async fn frame(&mut self) -> Result<Frame,MsgErr> {
        Frame::from_stream(& mut self.stream).await
    }
    pub async fn read_version(&mut self) -> Result<semver::Version,MsgErr>{
        self.frame().await?.to_version()
    }

    pub async fn read_string(&mut self) -> Result<String,MsgErr>{
        self.frame().await?.to_string()
    }

    pub async fn read_wave(&mut self) -> Result<UltraWave,MsgErr>{
        self.frame().await?.to_wave()
    }

    pub async fn write_frame(&mut self, frame: Frame ) -> Result<(),MsgErr>{
        frame.to_stream(& mut self.stream).await
    }

    pub async fn write_string(&mut self, string: String ) -> Result<(),MsgErr> {
        self.write_frame(Frame::from_string(string)).await
    }

    pub async fn write_version(&mut self, version: &semver::Version) -> Result<(),MsgErr> {
        self.write_frame(Frame::from_version(version)).await
    }

    pub async fn write_wave(&mut self, wave: UltraWave) -> Result<(),MsgErr> {
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
    server_kill_rx: broadcast::Receiver<()>
}

impl HyperlaneTcpServer {
    pub async fn new(port: u16, cert_dir: String, gate: Arc<HyperGateSelector>, logger: PointLogger) -> Result<Self,Error> {
        let (server_kill_tx,server_kill_rx) = broadcast::channel(1);
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key_file(format!("{}/key.pem", cert_dir), SslFiletype::PEM)?;
        acceptor.set_certificate_chain_file(format!("{}/cert.pem", cert_dir))?;
        let acceptor = acceptor.build();
        let listener = TcpListener::bind(format!("127.0.0.1:{}",port)).await.unwrap();

        Ok(Self { acceptor, gate,  listener, logger, server_kill_tx, server_kill_rx })
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
                Err(_) => break
            };
            let mut server_kill_rx = self.server_kill_tx.subscribe();
            tokio::spawn(async move {
                async fn serve(stream: TcpStream, ssl: Ssl, gate: Arc<HyperGateSelector>, server_kill_rx: broadcast::Receiver<()>, logger: PointLogger) -> Result<(), Error> {
                    logger.info("accepted new client");
                    let mut stream = SslStream::new(ssl, stream)?;

                    Pin::new(&mut stream).accept().await?;
                    let mut stream = FrameStream::new(stream);

                    let (status_tx,mut status_rx):(mpsc::Sender<HyperConnectionDetails>,mpsc::Receiver<HyperConnectionDetails>) = mpsc::channel(1024);
                    {
                        let logger = logger.clone();
                        tokio::spawn(async move {
                            while let Some(details) = status_rx.recv().await {
                                logger.info( format!("{} | {}", details.status.to_string(), details.info))
                            }
                        });
                    }
                    let mut mux = FrameMuxer::handshake(stream, server_kill_rx,status_tx, logger.clone()).await?;

                    let knock= tokio::time::timeout(Duration::from_secs(30), mux.rx.recv()).await?.ok_or("expected wave")?;
                    let knock = knock.to_directed()?;
                    if let Substance::Knock(knock) = knock.body() {
                        let mut endpoint = gate.knock(knock.clone()).await?;
                        mux.connect(endpoint);
                     } else {
                        let msg = format!("expected client Substance::Knock(Knock) encountered '{}'",knock.body().kind().to_string());
                        return logger.result(Err(MsgErr::str(msg).into()));
                    }

                    Ok(())
                }
                serve(stream,ssl,gate,server_kill_rx, logger).await;
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

impl From<MsgErr> for Error {
    fn from(e: MsgErr) -> Self {
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
    use super::*;
    use std::time::Duration;
    use cosmic_api::id::id::{Point, ToPort};
    use cosmic_api::log::RootLogger;
    use cosmic_hyperlane::test_util::{FAE, LESS, SingleInterchangePlatform, WaveTest};

    #[tokio::test]
    async fn test() -> Result<(), Error> {
        let platform = SingleInterchangePlatform::new().await;

        CertGenerator::gen(vec!["localhost".to_string()])?
            .write_to_dir(".".to_string())
            .await?;
        let logger = RootLogger::default();
        let logger = logger.point(Point::from_str("tcp-server")?);
        let port = 4343u16;
        let server = HyperlaneTcpServer::new(port,".".to_string(),platform.gate.clone(), logger.clone()).await?;
        let api = server.start()?;

        let less_logger = logger.point(LESS.clone());
        let less_client = Box::new(HyperlaneTcpClient::new( format!("localhost:{}",port), ".", platform.knock(LESS.to_port()),false,less_logger  ));
        let fae_logger = logger.point(FAE.clone());
        let fae_client = Box::new(HyperlaneTcpClient::new( format!("localhost:{}",port), ".", platform.knock(FAE.to_port()),false,fae_logger ));

        let test = WaveTest::new(less_client,fae_client);

        test.go().await.unwrap();

        Ok(())
    }
}