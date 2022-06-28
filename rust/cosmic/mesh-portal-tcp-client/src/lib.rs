#![allow(warnings)]

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate anyhow;

use mesh_portal_tcp_common::{PrimitiveFrameReader, PrimitiveFrameWriter, FrameWriter, FrameReader };
use anyhow::Error;
use mesh_portal_api_client::{Portal, ParticleCtrl, PortalSkel, InletApi, Inlet, ParticleCtrlFactory, Exchanges, PrePortalSkel};
use std::sync::Arc;
use dashmap::DashMap;
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc};
use mesh_portal::version::latest::portal;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::yield_now;
use mesh_portal::version;
use tokio::time::Duration;
use mesh_portal::version::latest::log::{SpanLogger, PointLogger};
use mesh_portal::version::latest::portal::{outlet, inlet, Exchanger, initin, initout};
use mesh_portal::version::latest::portal::initin::PortalAuth;
use mesh_portal::version::latest::portal::inlet::AssignRequest;

pub struct PortalTcpClient {
    pub host: String,
    pub portal: Arc<Portal>,
    pub close_tx: broadcast::Sender<i32>
}

impl PortalTcpClient {

    pub async fn new( host: String, mut client: Box<dyn PortalClient> ) -> Result<Self,Error> {
        let span_logger = client.logger().span();
        let stream = TcpStream::connect(host.clone()).await?;

        let (reader,writer) = stream.into_split();
        let mut reader = PrimitiveFrameReader::new(reader);
        let mut writer = PrimitiveFrameWriter::new(writer);

        let mut reader : FrameReader<initout::Frame> = FrameReader::new(reader );
        let mut writer : FrameWriter<initin::Frame>  = FrameWriter::new(writer );

        writer.write(initin::Frame::Flavor(client.flavor())).await?;

        if let initout::Frame::Ok = reader.read().await? {
println!("client: Flavor negotiaion Ok");
        } else {
            let message = "FLAVOR NEGOTIATION FAILED".to_string();
            span_logger.error(message.as_str());
            return Err(anyhow!(message));
        }

        let auth = client.auth();
        writer.write( initin::Frame::Auth(auth)).await?;

        if let initout::Frame::Ok = reader.read().await? {
println!("client: auth Ok.");
        } else {
            let message = "AUTH FAILED".to_string();
            span_logger.error(message.as_str());
            return Err(anyhow!(message));
        }

        let (inlet_tx, mut inlet_rx) = mpsc::channel(1024 );
        let (outlet_tx, mut outlet_rx) = mpsc::channel(1024 );

        let inlet = Arc::new(TcpInlet{
            sender: inlet_tx,
            logger: client.logger()
        });

        let skel = PrePortalSkel {
            config: Default::default(),
            inlet,
            logger: client.logger(),
            exchanges: Arc::new(DashMap::new() ),
            assign_exchange: Arc::new(DashMap::new() ),
        };

println!("client: init client pre");
        let factory = client.init( &mut reader, &mut writer, skel.clone() ).await?;

println!("client: init client post");
        writer.write( initin::Frame::Ready ).await?;
println!("client: signaled ready");

        let mut reader : FrameReader<outlet::Frame> = FrameReader::new(reader.done() );
        let mut writer : FrameWriter<inlet::Frame>  = FrameWriter::new(writer.done() );

println!("client: transitioned to portal frames.");

        let (close_tx,_) = broadcast::channel(128 );

        {
            let logger = client.logger();
            let close_tx = close_tx.clone();
            tokio::spawn(async move {
                while let Option::Some(frame) = inlet_rx.recv().await {
                    let logger = logger.span();
                    match writer.write(frame).await {
                        Ok(_) => {}
                        Err(err) => {
                            logger.error("FATAL: writer disconnected");
                            eprintln!("client: FATAL! writer disconnected.");
                            break;
                        }
                    }
                    yield_now().await;
                }
println!("client: inlet_rx complete.");
                close_tx.send(0);
            });
        }


        let portal = Portal::new(skel, outlet_tx.clone(), outlet_rx, factory, client.logger()).await?;
        {
            let logger = client.logger();
            let close_tx = close_tx.clone();
            tokio::spawn(async move {
                while let Result::Ok(frame) = reader.read().await {
println!("client reading frame: {}",frame.to_string());
                    match outlet_tx.send( frame ).await {
                        Result::Ok(_) => {

                        }
                        Result::Err(err) => {
                            span_logger.error("FATAL: reader disconnected");
                            eprintln!("client: FATAL! reader disconnected.");
                            break;
                        }
                    }
                    yield_now().await;
                }
println!("client reader.read() complete.");
                close_tx.send(0);
            });
        }

        return Ok(Self {
            host,
            portal,
            close_tx
        });

    }

    pub async fn request_assign( &self, request: AssignRequest ) -> Result<Arc<dyn ParticleCtrl>,Error> {
        self.portal.request_assign(request).await
    }
}

#[async_trait]
pub trait PortalClient: Send+Sync {
    fn flavor(&self) -> String;
    fn auth( &self ) -> PortalAuth;
    fn logger(&self) -> PointLogger;
    async fn init( &self, reader: & mut FrameReader<initout::Frame>, writer: & mut FrameWriter<initin::Frame>, skel: PrePortalSkel ) -> Result<Arc< dyn ParticleCtrlFactory >,Error>;

}

struct TcpInlet {
    pub sender: mpsc::Sender<inlet::Frame>,
    pub logger: PointLogger
}

impl Inlet for TcpInlet {
    fn inlet_frame(&self, frame: inlet::Frame) {
        let sender = self.sender.clone();
        let logger = self.logger.span();
        tokio::spawn(async move {
println!("Sending FRAME via inlet api...{}", frame.to_string());
            match sender.send(frame).await
            {
                Ok(_) => {
                    println!("SENT FRAME via inlet!");
                }
                Err(err) => {
                    logger.error(format!("ERROR: frame failed to send to client inlet").as_str())
                }
            }
        });
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
