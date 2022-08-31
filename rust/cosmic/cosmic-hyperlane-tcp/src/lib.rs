use cosmic_hyperlane::{HyperGate, HyperGateSelector, HyperwayEndpoint, VersionGate};
use openssl::error::ErrorStack;
use openssl::ssl::{Ssl, SslAcceptor, SslFiletype, SslMethod};
use rcgen::{generate_simple_self_signed, Certificate, RcgenError};
use std::io::{Empty, Read};
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
use tokio::sync::mpsc;
use tokio_openssl::SslStream;
use cosmic_api::error::MsgErr;
use cosmic_api::log::PointLogger;
use cosmic_api::substance::substance::Substance;
use cosmic_api::VERSION;
use cosmic_api::wave::UltraWave;


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
        let mut certs = File::open(format!("{}/certs.pem", dir)).await?;
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
        let mut certs = File::create(format!("{}/certs.pem", dir)).await?;
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

pub struct FrameStream<'a> {
    stream: &'a mut SslStream<TcpStream>
}

impl <'a> FrameStream<'a> {
    pub fn new(stream: &'a mut SslStream<TcpStream>) -> Self {
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
    acceptor: SslAcceptor
}

impl HyperlaneTcpServer {
    pub async fn new(cert_dir: String, gate: Arc<HyperGateSelector>, logger: PointLogger) -> Result<Self,Error> {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls()).unwrap();
        acceptor.set_private_key_file(format!("{}/key.pem", cert_dir), SslFiletype::PEM)?;
        acceptor.set_certificate_chain_file(format!("{}/certs.pem", cert_dir))?;
        let acceptor = acceptor.build();
        let listener = TcpListener::bind("127.0.0.1:4343").await.unwrap();

        Ok(Self { acceptor, gate,  listener, logger})
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
            tokio::spawn(async move {
                async fn serve(stream: TcpStream, ssl: Ssl, gate: Arc<HyperGateSelector>, logger: PointLogger) -> Result<(), MsgErr> {
                    logger.info("accepted new client");
                    let mut stream = SslStream::new(ssl, stream).unwrap();

                    Pin::new(&mut stream).accept().await.unwrap();
                    let mut stream = FrameStream::new(&mut stream);
                    stream.write_version(&VERSION.clone()).await?;
                    let in_version = tokio::time::timeout(Duration::from_secs(30), stream.read_version()).await??;
                    logger.info(format!("client version: {}", in_version.to_string()));

                    if in_version == *VERSION {
                        logger.info("version match");
                        stream.write_string("Ok".to_string() ).await?;
                    } else {
                        logger.warn("version mismatch");
                        stream.write_string(format!("Err(\"expected version {}. encountered version {}\")",VERSION.to_string(),in_version.to_string()) ).await?;
                    }
                    let result = tokio::time::timeout(Duration::from_secs(30), stream.read_string()).await??;
                    if "Ok".to_string() != result {
                       return logger.result(Err(format!("client did not indicate Ok. expected: 'Ok' encountered '{}'",result).into()));
                    }

                    logger.info("client signaled Ok");

                    let knock= tokio::time::timeout(Duration::from_secs(30), stream.read_wave()).await??;
                    let knock = knock.to_directed()?;
                    if let Substance::Knock(knock) = knock.body() {
                        let mut endpoint = gate.knock(knock.clone()).await?;
                        let tx = endpoint.tx.clone();
                        loop {
                            tokio::select! {
                                Some(wave) = endpoint.rx.recv() => {
                                    stream.write_wave(wave).await?;
                                }
                                Ok(wave) = stream.read_wave() => {
                                    tx.send(wave).await?;
                                }
                            }
                        }
                   } else {
                        let msg = format!("expected client Substance::Knock(Knock) encountered '{}'",knock.body().kind().to_string());
                        return logger.result(Err(msg.into()));
                    }

                    Ok(())
                }
                serve(stream,ssl,gate,logger).await;
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

impl Error {
    pub fn new<S: ToString>(m: S) -> Self {
        Self {
            message: m.to_string(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use cosmic_api::id::id::Point;
    use cosmic_api::log::RootLogger;

    #[tokio::test]
    async fn test() -> Result<(), Error> {
        CertGenerator::gen(vec!["localhost".to_string()])?
            .write_to_dir(".".to_string())
            .await?;
        let logger = RootLogger::default();
        let logger = logger.point(Point::from_str("tcp-server")?);
        let gate = Arc::new(HyperGateSelector::default() );
        let server = HyperlaneTcpServer::new(".".to_string(),gate, logger).await?;
        let api = server.start()?;
        tokio::time::sleep(Duration::from_secs(2 * 60)).await;
        Ok(())
    }
}
