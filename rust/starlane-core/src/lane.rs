use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::pin::Pin;
use std::task::Poll;

use futures::{FutureExt, TryFutureExt};
use futures::future::select_all;
use futures::task;
use futures::task::Context;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
use tokio::time::Duration;
use url::Url;

use crate::error::Error;
use crate::frame::{Frame, ProtoFrame};
use crate::id::Id;
use crate::proto::{local_tunnels, ProtoStar, ProtoTunnel};
use crate::star::{Star, StarCommand, StarKey};
use crate::starlane::{StarlaneCommand, VersionFrame};
use crate::starlane::StarlaneCommand::Connect;
use std::cell::Cell;
use serde::de::DeserializeOwned;
use crate::template::{ConstellationSelector, StarInConstellationTemplateHandle, StarInConstellationTemplateSelector};
use std::convert::TryInto;

pub static STARLANE_PROTOCOL_VERSION: i32 = 1;
pub static LANE_QUEUE_SIZE: usize = 32;

#[derive(Clone)]
pub struct OutgoingSide {
    pub out_tx: Sender<LaneCommand>,
}

pub struct IncomingSide {
    rx: Receiver<Frame>,
    tunnel_receiver_rx: Receiver<TunnelInState>,
    tunnel: TunnelInState,
}

impl IncomingSide {
    pub async fn recv(&mut self) -> Option<StarCommand> {
        loop {
            match &mut self.tunnel {
                TunnelInState::None => match self.tunnel_receiver_rx.recv().await {
                    None => {
                        eprintln!("received None from tunnel");
                        return Option::None;
                    }
                    Some(tunnel) => {
                        self.tunnel = tunnel;
                    }
                },
                TunnelInState::In(tunnel) => {
                    match tunnel.rx.recv().await {
                        None => {
                            eprintln!("received None from tunnel.rx");
                            // let's hope the tunnel is reset soon
                            break None;
                        }
                        Some(frame) => {
                            return Option::Some(StarCommand::Frame(frame));
                        }
                    }
                }
            }
        }
    }
}

pub struct LaneMiddle {
    rx: Receiver<LaneCommand>,
    tx: Sender<Frame>,
    tunnel: TunnelOutState,
    queue: Vec<Frame>,
}

impl LaneMiddle {
    async fn die(&self, message: String) {
        eprintln!("{}", message.as_str());
    }

    pub async fn run(mut self) {
        while let Option::Some(command) = self.rx.recv().await {
            match command {
                LaneCommand::Tunnel(tunnel) => {
                    if let TunnelOutState::Out(tunnel) = &tunnel {
                        for frame in self.queue.drain(..) {
                            tunnel.tx.send(frame).await;
                        }
                    }
                    self.tunnel = tunnel;
                }
                LaneCommand::Frame(frame) => match &self.tunnel {
                    TunnelOutState::Out(tunnel) => {
                        tunnel.tx.send(frame).await;
                    }
                    TunnelOutState::None => {
                        self.queue.push(frame);
                    }
                },
            }
        }
        // need to signal to Connector that this lane is now DEAD
    }

    async fn process_command(&mut self, command: Option<LaneCommand>) {}
}

pub enum LaneCommand {
    Tunnel(TunnelOutState),
    Frame(Frame),
}

pub struct Chamber<T> {
    pub holding: Option<T>,
}

impl<T> Chamber<T> {
    pub fn new() -> Self {
        Chamber {
            holding: Option::None,
        }
    }
}

pub struct ProtoLaneEndpoint {
    pub remote_star: Option<StarKey>,
    pub incoming: IncomingSide,
    pub outgoing: OutgoingSide,
    tunnel_receiver_tx: Sender<TunnelInState>,
}

impl ProtoLaneEndpoint {
    pub fn new(star_key: Option<StarKey>) -> Self {
        let (mid_tx, mid_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (in_tx, in_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (tunnel_receiver_tx, tunnel_receiver_rx) = mpsc::channel(1);

        let midlane = LaneMiddle {
            rx: mid_rx,
            tx: in_tx,
            tunnel: TunnelOutState::None,
            queue: vec![],
        };

        tokio::spawn(async move {
            midlane.run().await;
        });

        ProtoLaneEndpoint {
            remote_star: star_key,
            tunnel_receiver_tx: tunnel_receiver_tx,
            incoming: IncomingSide {
                rx: in_rx,
                tunnel_receiver_rx: tunnel_receiver_rx,
                tunnel: TunnelInState::None,
            },
            outgoing: OutgoingSide { out_tx: mid_tx },
        }
    }

    pub fn get_tunnel_in_tx(&self) -> Sender<TunnelInState> {
        self.tunnel_receiver_tx.clone()
    }
}

impl TryInto<LaneEndpoint> for ProtoLaneEndpoint{
    type Error = Error;

    fn try_into(self) -> Result<LaneEndpoint, Self::Error> {
        if self.remote_star.is_some() {
            Ok(LaneEndpoint{
                remote_star: self.remote_star.unwrap(),
                incoming: self.incoming,
                outgoing: self.outgoing,
                tunnel_receiver_tx: self.tunnel_receiver_tx
            })
        } else {
            Err("star_key must be set before ProtoLaneEndpoint can evolve into a LaneEndpoint".into())
        }
    }
}


pub struct LaneEndpoint {
    pub remote_star: StarKey,
    pub incoming: IncomingSide,
    pub outgoing: OutgoingSide,
    tunnel_receiver_tx: Sender<TunnelInState>,
}


impl LaneEndpoint {

    pub fn get_tunnel_in_tx(&self) -> Sender<TunnelInState> {
        self.tunnel_receiver_tx.clone()
    }
}

pub enum TunnelOutState {
    Out(TunnelOut),
    None,
}

pub enum TunnelInState {
    In(TunnelIn),
    None,
}

impl fmt::Display for TunnelOutState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            TunnelOutState::Out(_) => "Sender".to_string(),
            TunnelOutState::None => "None".to_string(),
        };
        write!(f, "{}", r)
    }
}

impl fmt::Display for TunnelInState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            TunnelInState::In(_) => "Receiver".to_string(),
            TunnelInState::None => "None".to_string(),
        };
        write!(f, "{}", r)
    }
}

#[derive(Clone)]
pub struct TunnelOut {
    pub remote_star: StarKey,
    pub tx: Sender<Frame>,
}

pub struct TunnelIn {
    pub remote_star: StarKey,
    pub rx: Receiver<Frame>,
}

#[derive(Clone)]
pub struct ConnectorController {
    pub command_tx: mpsc::Sender<ConnectorCommand>,
}

#[async_trait]
pub trait TunnelConnector: Send {
}

#[derive(Clone)]
pub enum LaneSignal {
    Close,
}

pub enum ConnectorCommand {
    Reset,
    Close,
}
pub struct ClientSideTunnelConnector {
    pub in_tx: Sender<TunnelInState>,
    pub out: OutgoingSide,
    command_rx: Receiver<ConnectorCommand>,
    host_address: String,
    selector: StarInConstellationTemplateSelector
}

impl ClientSideTunnelConnector {
    pub async fn new(lane: &ProtoLaneEndpoint, host_address: String, selector: StarInConstellationTemplateSelector ) -> Result<ConnectorController, Error> {
        let (command_tx, command_rx) = mpsc::channel(1);
        let mut connector = Self {
            out: lane.outgoing.clone(),
            in_tx: lane.get_tunnel_in_tx(),
            command_rx,
            host_address,
            selector
        };

        tokio::spawn(async move { connector.run().await });

        Ok(ConnectorController {
            command_tx: command_tx,
        })
    }

    async fn run(mut self) {
        loop {
            if let Result::Ok(stream) = TcpStream::connect(self.host_address.clone()).await
            {
                let (tx, rx) = FrameCodex::new(stream);

                let proto_tunnel = ProtoTunnel {
                    star: Option::None,
                    tx: tx,
                    rx: rx
                };

                match proto_tunnel.evolve().await {
                    Ok((tunnel_out, tunnel_in)) => {
                        self.out.out_tx.send(LaneCommand::Tunnel(TunnelOutState::Out(tunnel_out))).await;
                        self.in_tx.send(TunnelInState::In(tunnel_in)).await;

                        if let Option::Some(command) = self.command_rx.recv().await
                        {
                            self.out.out_tx.send(LaneCommand::Tunnel(TunnelOutState::None)).await;
                            match command {
                                ConnectorCommand::Reset => {
                                    println!("reset connection");
                                }
                                ConnectorCommand::Close => {
                                    eprintln!("CLIENT CONNECTION CLOSING (0)");
                                    break;
                                }
                            }
                        } else {
                            eprintln!("CLIENT CONNECTION CLOSING (1)");
                            break;
                        }
                    }
                    Err(error) => {
                        eprintln!("CONNECTION ERROR: {}", error.error);
                    }
                }
            }
        }
    }
}

#[async_trait]
impl TunnelConnector for ClientSideTunnelConnector {


}




pub struct ServerSideTunnelConnector {
    pub low_in_tx: Sender<TunnelInState>,
    pub low: OutgoingSide,
    command_rx: Receiver<ConnectorCommand>,
    stream : Cell<Option<TcpStream>>
}

impl ServerSideTunnelConnector {
    pub async fn new(low_lane: &ProtoLaneEndpoint, stream: TcpStream) -> Result<ConnectorController, Error> {
        let (command_tx, command_rx) = mpsc::channel(1);
        let mut connector = Self {
            low: low_lane.outgoing.clone(),
            low_in_tx: low_lane.get_tunnel_in_tx(),
            command_rx,
            stream: Cell::new(Option::Some( stream ))
        };

        tokio::spawn(async move { connector.run().await });

        Ok(ConnectorController {
            command_tx: command_tx,
        })
    }

    async fn run(mut self) {

        let stream = match self.stream.replace(Option::None).ok_or("expected stream to be Some"){
            Err(err)=> {
                eprintln!("CONNECTION ERROR: {}", err);
                return;
            }
            Ok(stream) => stream
        };

        let (tx,rx) = FrameCodex::new(stream);

        let proto_tunnel = ProtoTunnel{
            star: Option::None,
            tx: tx,
            rx: rx
        };

        match proto_tunnel.evolve().await {
            Ok((low_out,low_in)) => {
                self.low.out_tx.send(LaneCommand::Tunnel(TunnelOutState::Out(low_out))) .await;
                self.low_in_tx.send(TunnelInState::In(low_in)).await;

                self.command_rx.recv().await;
                self.low.out_tx.send(LaneCommand::Tunnel(TunnelOutState::None)).await;
            }
            Err(error) => {
                eprintln!("CONNECTION ERROR: {}",error.error );
            }
        }
    }
}

#[async_trait]
impl TunnelConnector for ServerSideTunnelConnector {


}

pub struct LocalTunnelConnector {
    pub high_star: Option<StarKey>,
    pub low_star: Option<StarKey>,
    pub high: OutgoingSide,
    pub low: OutgoingSide,
    pub high_in_tx: Sender<TunnelInState>,
    pub low_in_tx: Sender<TunnelInState>,
    command_rx: Receiver<ConnectorCommand>,
}

impl LocalTunnelConnector {
    pub async fn new(high_lane: &ProtoLaneEndpoint, low_lane: &ProtoLaneEndpoint) -> Result<ConnectorController, Error> {
        let high_star = low_lane.remote_star.clone();
        let low_star = high_lane.remote_star.clone();

            let (command_tx, command_rx) = mpsc::channel(1);

            let mut connector = LocalTunnelConnector {
                high_star: high_star.clone(),
                low_star: low_star.clone(),
                high: high_lane.outgoing.clone(),
                low: low_lane.outgoing.clone(),
                high_in_tx: high_lane.get_tunnel_in_tx(),
                low_in_tx: low_lane.get_tunnel_in_tx(),
                command_rx: command_rx,
            };

            tokio::spawn(async move { connector.run().await });

            Ok(ConnectorController {
                command_tx: command_tx,
            })
    }

    async fn run(&mut self) {
        loop {
            let (mut high, mut low) = local_tunnels(self.high_star.clone(), self.low_star.clone());

            let (high, low) = tokio::join!(high.evolve(), low.evolve());

            if let (Ok((high_out, mut high_in)), Ok((low_out, low_in))) =
            (high, low)
            {
                self.high.out_tx.send(LaneCommand::Tunnel(TunnelOutState::Out(high_out))) .await;
                self.high_in_tx.send(TunnelInState::In(high_in)).await;
                self.low.out_tx.send(LaneCommand::Tunnel(TunnelOutState::Out(low_out))) .await;
                self.low_in_tx.send(TunnelInState::In(low_in)).await;
            } else {
                eprintln!("connection failure... trying again in 10 seconds");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }

            // then wait for next command
            match self.command_rx.recv().await {
                None => {
                    self.high
                        .out_tx
                        .send(LaneCommand::Tunnel(TunnelOutState::None))
                        .await;
                    self.low
                        .out_tx
                        .send(LaneCommand::Tunnel(TunnelOutState::None))
                        .await;
                    return;
                }
                Some(Reset) => {
                    // first set olds to None
                    self.high
                        .out_tx
                        .send(LaneCommand::Tunnel(TunnelOutState::None))
                        .await;
                    self.low
                        .out_tx
                        .send(LaneCommand::Tunnel(TunnelOutState::None))
                        .await;
                    // allow loop to continue
                }
                Some(Close) => {
                    self.high
                        .out_tx
                        .send(LaneCommand::Tunnel(TunnelOutState::None))
                        .await;
                    self.low
                        .out_tx
                        .send(LaneCommand::Tunnel(TunnelOutState::None))
                        .await;
                    return;
                }
            }
        }
    }
}

#[async_trait]
impl TunnelConnector for LocalTunnelConnector {

}

pub struct LaneMeta {
    pub star_paths: LruCache<StarKey, usize>,
    pub lane: LaneEndpoint,
}

impl LaneMeta {
    pub fn new(lane: LaneEndpoint) -> Self {
        LaneMeta {
            star_paths: LruCache::new(32 * 1024),
            lane: lane,
        }
    }

    pub fn get_hops_to_star(&mut self, star: &StarKey) -> Option<usize> {
        if *star == self.lane.remote_star {
            return Option::Some(1);
        }
        match self.star_paths.get(star) {
            None => Option::None,
            Some(hops) => Option::Some(hops.clone()),
        }
    }

    pub fn set_hops_to_star(&mut self, star: StarKey, hops: usize) {
        self.star_paths.put(star, hops);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub gateway: StarKey,
    pub kind: ConnectionKind,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionKind {
    Starlane,
    Url(String),
}

pub trait TunnelConnectorFactory: Send {
    fn connector(&self, data: &ConnectionInfo) -> Result<Box<dyn TunnelConnector>, Error>;
}

pub struct FrameCodex{
}

impl FrameCodex {

    pub fn new<F: Serialize+DeserializeOwned+Send+Sync+'static>(stream: TcpStream) -> (mpsc::Sender<F>, mpsc::Receiver<F>){
        let (mut read,mut write)= stream.into_split();
        let (in_tx,in_rx) = mpsc::channel(64);
        let (out_tx,mut out_rx) = mpsc::channel(64);

        tokio::spawn( async move {
            while let Option::Some(frame) = out_rx.recv().await {
                match FrameCodex::send(&mut write, frame).await
                {
                    Ok(_) => {}
                    Err(_) => {
                        break;
                    }
                }

            }
        });

        tokio::spawn( async move {
            while let Result::Ok(frame)= Self::receive(&mut read).await {
                in_tx.send(frame).await;
                // this HACK appears to be necessary in order for the receiver to
                // consistently receive values, but i do not know why
               tokio::time::sleep(Duration::from_secs(0)).await;
            }
        });

        (out_tx,in_rx)
    }

    async fn receive<F: Serialize+DeserializeOwned+Send+Sync+'static>( read: &mut OwnedReadHalf ) -> Result<F,Error> {
        let len = read.read_u32().await?;

        let mut buf = vec![0 as u8; len as usize];
        let mut buf_ref = buf.as_mut_slice();

        read.read_exact(buf_ref).await?;

        let frame: F = bincode::deserialize(buf_ref)?;

        Ok(frame)
    }

    async fn send<F: Serialize+DeserializeOwned+Send+Sync+'static>( write: &mut OwnedWriteHalf, frame: F) -> Result<(),Error> {
        let data = bincode::serialize(&frame)?;
        write.write_u32(data.len() as _ ).await?;
        write.write_all(data.as_slice()).await?;
        Ok(())
    }

}


pub enum LaneId {
    None,
    Lane(StarKey),
    ProtoLane(usize)
}

#[cfg(test)]
mod test {
    use futures::FutureExt;
    use tokio::runtime::Runtime;
    use tokio::time::Duration;

    use crate::error::Error;
    use crate::frame::{Diagnose, ProtoFrame};
    use crate::id::Id;
    use crate::lane::{LaneEndpoint, LaneCommand, FrameCodex, ProtoLaneEndpoint};
    use crate::lane::ConnectorCommand;
    use crate::lane::Frame;
    use crate::lane::LocalTunnelConnector;
    use crate::lane::TunnelConnector;
    use crate::proto::local_tunnels;
    use crate::star::{StarCommand, StarKey};
    use tokio::net::{TcpListener, TcpStream};
    use std::net::{ToSocketAddrs, SocketAddr};
    use std::str::FromStr;
    use tokio::sync::oneshot;

    #[test]
    fn frame_codex()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on( async {
            let (wait_tx,wait_rx) = oneshot::channel();
            tokio::spawn( async move {
                let std_listener = std::net::TcpListener::bind("127.0.0.1:7788").unwrap();
                let listener = TcpListener::from_std(std_listener).unwrap();
                println!("LISTENING!");
                if let Ok((mut stream,_)) = listener.accept().await {
                    println!("new client!");
                    let (mut tx,mut rx) = FrameCodex::new(stream);

                    tokio::spawn(async move {
                        println!("waiting for frame...");
                        while let frame = rx.recv().await.unwrap() {
                            println!("RECEIVED FRAME: {}", frame);
                            if let Frame::Close = frame {
                                break;
                            }
                        }
                        wait_tx.send(());
                    });
                }
            });

            tokio::time::sleep(Duration::from_secs(1)).await;

            let stream = TcpStream::connect(SocketAddr::from_str("127.0.0.1:7788").unwrap() ).await.unwrap();
            let (mut tx,mut rx) = FrameCodex::new(stream);

            tokio::spawn(async{
                println!("sending PING.");
            });

            tx.send( Frame::Ping ).await;
            tx.send( Frame::Close).await;

            tokio::spawn(async{
                println!("PING SENT.");
            });


            wait_rx.await;

            tokio::spawn(async{
                println!("all done.");
            })

        });
    }

    #[test]
    pub fn proto_tunnel() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let (mut p1, mut p2) =
                local_tunnels(Option::Some(StarKey::new(2)), Option::Some(StarKey::new(1)));

            let future1 = p1.evolve();
            let future2 = p2.evolve();
            let (result1, result2) = join!(future1, future2);

            assert!(result1.is_ok());
            assert!(result2.is_ok());
        });
    }

    #[test]
    pub fn lane() {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let high = StarKey::new(2);
            let low = StarKey::new(1);

            let mut high_lane = ProtoLaneEndpoint::new(Option::Some(low.clone()));
            let mut low_lane = ProtoLaneEndpoint::new(Option::Some(high.clone()));

            let connector_ctrl = LocalTunnelConnector::new(&high_lane, &low_lane)
                .await
                .unwrap();

            high_lane
                .outgoing
                .out_tx
                .send(LaneCommand::Frame(Frame::Diagnose(Diagnose::Ping)))
                .await;

            let result = low_lane.incoming.recv().await;
            if let Some(StarCommand::Frame(Frame::Diagnose(Diagnose::Ping))) = result {
                println!("RECEIVED PING!");
                assert!(true);
            } else if let Some(frame) = result {
                println!("RECEIVED {}", frame);
                assert!(false);
            } else {
                println!("RECEIVED NONE");
                assert!(false);
            }
            connector_ctrl
                .command_tx
                .send(ConnectorCommand::Reset)
                .await;
            high_lane
                .outgoing
                .out_tx
                .send(LaneCommand::Frame(Frame::Diagnose(Diagnose::Pong)))
                .await;
            let result = low_lane.incoming.recv().await;

            if let Some(StarCommand::Frame(Frame::Diagnose(Diagnose::Pong))) = result {
                println!("RECEIVED PoNG!");
                assert!(true);
            } else if let Some(frame) = result {
                println!("RECEIVED {}", frame);
                assert!(false);
            } else {
                println!("RECEIVED NONE");
                assert!(false);
            }
        });
    }
}




