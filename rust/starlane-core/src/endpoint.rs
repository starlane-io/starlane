use std::convert::TryFrom;
use std::str::FromStr;
use alcoholic_jwt::JWKS;
use mesh_portal::version::latest::frame::PrimitiveFrame;
use mesh_portal_tcp_common::{FrameReader, FrameWriter, PrimitiveFrameReader, PrimitiveFrameWriter};
use serde::{Deserialize, Serialize};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::{TcpListener, TcpStream};
use crate::command::cli::CliServer;
use crate::error::Error;
use crate::starlane::api::StarlaneApi;
use crate::starlane::StarlaneMachine;
use crate::util::JwksCache;

pub struct ServicesEndpoint {
  runner: ServicesEndpointRunner,
  port: usize
}

#[derive(Clone)]
struct ServicesEndpointRunner {
    api: StarlaneApi,
    machine: StarlaneMachine,
    jwksCache: JwksCache
}


impl ServicesEndpoint {

    pub async fn new( machine: StarlaneMachine, port: usize ) -> Result<(),Error> {
        let api = machine.get_starlane_api().await?;
        let runner = ServicesEndpointRunner {
                jwksCache: JwksCache::new(api.clone()),
                api,
                machine
            };

        let (result_tx,result_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            match std::net::TcpListener::bind(format!("0.0.0.0:{}", port)) {
                Ok(std_listener) => {
                    let listener = TcpListener::from_std(std_listener).unwrap();
                    result_tx.send(Ok(()));
                    while let Ok((stream, _)) = listener.accept().await {
                        let mut runner = runner.clone();

                        tokio::spawn( async move {
                            match runner.handle(stream).await {
                                Ok(_) => {}
                                Err(err) => {
                                    error!("{}", err.to_string());
                                }
                            };
                        });
                        // for some reason this is absoutly needed in a tokio spawn while loop
                        tokio::task::yield_now().await;
                    }
                }
                Err(error) => {
                    error!("FATAL: could not setup TcpListener {}", error);
                    result_tx.send(Err(error.into()));
                }
            }
        });

        result_rx.await?
    }
}


impl ServicesEndpointRunner {

    pub async fn handle( &mut self, stream: TcpStream ) -> Result<(),Error>{
        println!("adding Stream!");
        let (mut reader, mut writer) = stream.into_split();
        let mut reader = PrimitiveFrameReader::new(reader);
        let mut writer = PrimitiveFrameWriter::new(writer );

        // Authenticate
        let (reader,writer) = {
            let mut reader :FrameReader<AuthRequestFrame>= FrameReader::new(reader);
            let mut writer :FrameWriter<EndpointResponse>= FrameWriter::new(writer );
            info!("reading auth token...");
            async fn auth(end: &mut ServicesEndpointRunner, reader: & mut FrameReader<AuthRequestFrame>) -> Result<(),Error> {
                let request = reader.read().await?;
                let token = request.to_string();
                info!("TOKEN: {}",token);
                end.jwksCache.validate(token.as_str()).await?;
                Ok(())
            }

            match auth(self, & mut reader ).await {
                Ok(()) => writer.write(EndpointResponse::Ok).await?,
                Err(err) => {
                    writer.write( EndpointResponse::Err(format!("Authorization failed: {}",err.to_string()))).await;
                    return Err(err);
                }
            }

            (reader.done(),writer.done())
        };

        let (reader,writer, service) = {
            info!("service selection...");
            let mut reader: FrameReader<Service> = FrameReader::new(reader);
            let mut writer: FrameWriter<EndpointResponse>= FrameWriter::new(writer );
            match reader.read().await {
                Ok(service) => {
                    writer.write(EndpointResponse::Ok).await?;
                    (reader.done(),writer.done(), service)
                }
                Err(err) => {
                    writer.write(EndpointResponse::Err(format!("service selection failed: {}",err.to_string()))).await?;
                    return Err(err.into());
                }
            }
        };

        match service {
            Service::Cli => {
                info!("Cli Service selected");
                let mut writer = FrameWriter::new( writer);
                writer.write( EndpointResponse::Ok).await?;
                let mut writer = writer.done();
                info!("ServiceSelectionResponse::Cli sent...");

                let api = self.api.clone();
                tokio::spawn( async move {
                    CliServer::new(api, reader.done(), writer.done() ).await;
                });
            }
        }
        Ok(())
    }
}


#[derive(Clone,Serialize,Deserialize)]
pub enum AuthRequestFrame {
    Token(String)
}

impl TryFrom<PrimitiveFrame> for AuthRequestFrame {
    type Error = mesh_portal::error::MsgErr;

    fn try_from(value: PrimitiveFrame) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize(value.data.as_slice())?)
    }
}

impl ToString for AuthRequestFrame {
    fn to_string(&self) -> String {
        match self {
            AuthRequestFrame::Token(token) => token.clone()
        }
    }
}


#[derive(Clone,Serialize,Deserialize)]
pub enum EndpointResponse{
    Ok,
    Err(String)
}

impl  EndpointResponse {
    pub fn to_result(self) -> Result<(),String> {
        match self {
            EndpointResponse::Ok => Result::Ok(()),
            EndpointResponse::Err(err) => Result::Err(err),
        }
    }
}



#[derive(Clone,Serialize,Deserialize)]
pub enum Service {
    Cli,
}

impl TryFrom<PrimitiveFrame> for EndpointResponse{
    type Error = mesh_portal::error::MsgErr;

    fn try_from(value: PrimitiveFrame) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize(value.data.as_slice())?)
    }
}


impl TryFrom<PrimitiveFrame> for Service {
    type Error = mesh_portal::error::MsgErr;

    fn try_from(value: PrimitiveFrame) -> Result<Self, Self::Error> {
        Ok(bincode::deserialize(value.data.as_slice())?)
    }
}


impl FromStr for Service {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Cli" => Ok(Self::Cli),
            what => Err(format!("invalid service selection: {}",what).into())
        }
    }
}
