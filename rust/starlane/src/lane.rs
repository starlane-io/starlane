use std::cmp::Ordering;
use std::pin::Pin;
use std::task::Poll;

use futures::{FutureExt, TryFutureExt};
use futures::future::select_all;
use futures::task;
use futures::task::Context;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
use tokio::time::Duration;
use serde::{Serialize,Deserialize};

use crate::error::Error;
use crate::id::Id;
use crate::frame::{Frame, ProtoFrame};
use crate::proto::{local_tunnels, ProtoStar, ProtoTunnel};
use crate::star::{Star, StarKey, StarCommand};
use crate::starlane::{ConnectCommand, StarlaneCommand};
use crate::starlane::StarlaneCommand::Connect;
use std::fmt;
use std::collections::{HashSet, HashMap};
use url::Url;
use lru::LruCache;

pub static STARLANE_PROTOCOL_VERSION: i32 = 1;
pub static LANE_QUEUE_SIZE: usize = 32;

#[derive(Clone)]
pub struct OutgoingLane
{
    pub tx: Sender<LaneCommand>,
}

pub struct IncomingLane
{
    rx: Receiver<Frame>,
    tunnel_receiver_rx: Receiver<TunnelReceiverState>,
    tunnel: TunnelReceiverState
}



impl IncomingLane
{
    pub async fn recv(&mut self) -> Option<StarCommand>
    {
        loop {
            match &mut self.tunnel
            {
                TunnelReceiverState::None => {
                    match self.tunnel_receiver_rx.recv().await
                    {
                        None => {
                            eprintln!("received None from tunnel");
                            return Option::None;
                        }
                        Some(tunnel) => {

                            self.tunnel = tunnel;
                        }
                    }
                }
                TunnelReceiverState::Receiver( tunnel) => {
                    match tunnel.rx.recv().await
                    {
                        None => {
                            eprintln!("received None from tunnel.rx")
                            // let's hope the tunnel is reset soon
                        }
                        Some(frame) => {return Option::Some(StarCommand::Frame(frame));}
                    }
                }

                }
            }
        }
}

pub struct MidLane
{
    rx: Receiver<LaneCommand>,
    tx: Sender<Frame>,
    tunnel: TunnelSenderState,
    queue: Vec<Frame>
}

impl MidLane
{
    async fn die(&self, message: String)
    {
        eprintln!("{}",message.as_str());
    }

    pub async fn run(mut self)
    {
        while let Option::Some(command) = self.rx.recv().await {
            match command
            {
                LaneCommand::Tunnel(tunnel) => {
                    if let TunnelSenderState::Sender(tunnel) = &tunnel
                    {
                        for frame in self.queue.drain(..)
                        {
                            tunnel.tx.send(frame).await;
                        }
                    }
                    self.tunnel = tunnel;
                }
                LaneCommand::Frame(frame) => {
                    match &self.tunnel {
                        TunnelSenderState::Sender(tunnel) => {
                            tunnel.tx.send(frame).await;
                        }
                        TunnelSenderState::None => {
                            self.queue.push(frame);
                        }
                    }
                }
            }
        }
        // need to signal to Connector that this lane is now DEAD
    }

    async fn process_command( &mut self, command: Option<LaneCommand> )
    {}
}


pub enum LaneCommand
{
    Tunnel(TunnelSenderState),
    Frame(Frame)
}

pub struct Chamber<T>
{
    pub holding: Option<T>
}

impl <T> Chamber<T>
{
    pub fn new()->Self
    {
      Chamber {
          holding: Option::None
      }
    }
}

pub struct Lane
{
    pub remote_star: Option<StarKey>,
    pub incoming: IncomingLane,
    pub outgoing: OutgoingLane,
    tunnel_receiver_tx: Sender<TunnelReceiverState>
}

impl Lane
{
    pub async fn new(remote_star: Option<StarKey>) -> Self
    {
        let (mid_tx, mid_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (in_tx, in_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (tunnel_receiver_tx, tunnel_receiver_rx) = mpsc::channel(1);

        let midlane = MidLane {
            rx: mid_rx,
            tx: in_tx,
            tunnel: TunnelSenderState::None,
            queue: vec![]
        };

        tokio::spawn( async move { midlane.run().await; } );

        Lane{
            remote_star: remote_star,
            tunnel_receiver_tx: tunnel_receiver_tx,
            incoming: IncomingLane{
                rx: in_rx,
                tunnel_receiver_rx: tunnel_receiver_rx,
                tunnel: TunnelReceiverState::None
            },
            outgoing: OutgoingLane {
                tx: mid_tx
            },
        }
    }

    pub fn get_tunnel_receiver_tx_channel(&self) -> Sender<TunnelReceiverState>
    {
        self.tunnel_receiver_tx.clone()
    }

}





pub enum TunnelSenderState
{
    Sender(TunnelSender),
    None
}

pub enum TunnelReceiverState
{
    Receiver(TunnelReceiver),
    None
}

impl fmt::Display for TunnelSenderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            TunnelSenderState::Sender(_) => "Sender".to_string(),
            TunnelSenderState::None => "None".to_string()
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for TunnelReceiverState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            TunnelReceiverState::Receiver(_) => "Receiver".to_string(),
            TunnelReceiverState::None => "None".to_string()
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone)]
pub struct TunnelSender
{
    pub remote_star: StarKey,
    pub tx: Sender<Frame>
}

pub struct TunnelReceiver
{
    pub remote_star: StarKey,
    pub rx: Receiver<Frame>
}

pub struct ConnectorController
{
    pub command_tx: mpsc::Sender<ConnectorCommand>,
}

#[async_trait]
pub trait TunnelConnector: Send
{
    async fn run(&mut self);
}

#[derive(Clone)]
pub enum LaneSignal
{
    Close
}

pub enum ConnectorCommand
{
    Reset,
    Close
}

pub struct LocalTunnelConnector
{
    pub high_star: Option<StarKey>,
    pub low_star: Option<StarKey>,
    pub high: OutgoingLane,
    pub low: OutgoingLane,
    pub high_receiver_tx: Sender<TunnelReceiverState>,
    pub low_receiver_tx: Sender<TunnelReceiverState>,
    command_rx: Receiver<ConnectorCommand>
}

impl LocalTunnelConnector
{
    pub async fn new(high_lane: &Lane, low_lane: &Lane ) -> Result<ConnectorController,Error>
    {
        let high_star = low_lane.remote_star.clone();
        let low_star = high_lane.remote_star.clone();
        if high_star.cmp(&low_star ) != Ordering::Greater
        {
            Err("High star must have a greater StarKey (meaning higher constellation index array and star index value".into())
        }
        else {
            let (command_tx,command_rx) = mpsc::channel(1);

            let mut connector = LocalTunnelConnector {
                high_star: high_star.clone(),
                low_star: low_star.clone(),
                high: high_lane.outgoing.clone(),
                low: low_lane.outgoing.clone(),
                high_receiver_tx: high_lane.get_tunnel_receiver_tx_channel(),
                low_receiver_tx: low_lane.get_tunnel_receiver_tx_channel(),
                command_rx: command_rx
            };

            tokio::spawn( async move { connector.run().await });

            Ok(ConnectorController{ command_tx: command_tx})
        }
    }
}

#[async_trait]
impl TunnelConnector for LocalTunnelConnector
{
    async fn run(&mut self) {
        loop {
            let (mut high, mut low) = local_tunnels(self.high_star.clone(), self.low_star.clone());

            let (high, low) = tokio::join!(high.evolve(),low.evolve());

            if let (Ok((high_sender, mut high_receiver)), Ok((low_sender, low_receiver))) = (high, low)
            {
                self.high.tx.send(LaneCommand::Tunnel(TunnelSenderState::Sender(high_sender))).await;
                self.high_receiver_tx.send( TunnelReceiverState::Receiver(high_receiver)).await;
                self.low.tx.send(LaneCommand::Tunnel(TunnelSenderState::Sender(low_sender))).await;
                self.low_receiver_tx.send( TunnelReceiverState::Receiver(low_receiver)).await;
            }
            else {
                eprintln!("connection failure... trying again in 10 seconds");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }

                // then wait for next command
                match self.command_rx.recv().await
                {
                    None => {
                        self.high.tx.send(LaneCommand::Tunnel(TunnelSenderState::None)).await;
                        self.low.tx.send(LaneCommand::Tunnel(TunnelSenderState::None)).await;
                        return;
                    }
                    Some(Reset) => {
                        // first set olds to None
                        self.high.tx.send(LaneCommand::Tunnel(TunnelSenderState::None)).await;
                        self.low.tx.send(LaneCommand::Tunnel(TunnelSenderState::None)).await;
                        // allow loop to continue
                    }
                    Some(Close) => {
                        self.high.tx.send(LaneCommand::Tunnel(TunnelSenderState::None)).await;
                        self.low.tx.send(LaneCommand::Tunnel(TunnelSenderState::None)).await;
                        return;
                    }
                }

        }
    }
}

pub struct LaneMeta
{
    pub star_paths: LruCache<StarKey,usize>,
    pub lane: Lane
}

impl LaneMeta
{
    pub fn new( lane: Lane ) -> Self
    {
        LaneMeta{
            star_paths: LruCache::new(32*1024 ),
            lane: lane
        }
    }

    pub fn get_hops_to_star(&mut self, star: &StarKey ) ->Option<usize>
    {
        if self.lane.remote_star.is_some() && star == self.lane.remote_star.as_ref().unwrap()
        {
            return Option::Some(1);
        }
        match self.star_paths.get(star)
        {
            None => Option::None,
            Some(hops) => Option::Some(hops.clone())
        }
    }

    pub fn set_hops_to_star(&mut self, star: StarKey, hops: usize )
    {
        self.star_paths.put(star, hops);
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo
{
    pub gateway: StarKey,
    pub kind: ConnectionKind,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionKind
{
    Starlane,
    Url(String)
}


pub trait TunnelConnectorFactory: Send
{
    fn connector(&self, data: &ConnectionInfo) -> Result<Box<dyn TunnelConnector>,Error>;
}

#[cfg(test)]
mod test
{
    use futures::FutureExt;
    use tokio::runtime::Runtime;

    use crate::error::Error;
    use crate::id::Id;
    use crate::frame::{ProtoFrame, FrameDiagnose};
    use crate::proto::local_tunnels;
    use crate::star::{StarKey, StarCommand};
    use crate::lane::{Lane, LaneCommand};
    use crate::lane::LocalTunnelConnector;
    use crate::lane::TunnelConnector;
    use crate::lane::ConnectorCommand;
    use crate::lane::Frame;
    use tokio::time::Duration;

    #[test]
   pub fn proto_tunnel()
   {
       let rt = Runtime::new().unwrap();
       rt.block_on(async {

           let (mut p1, mut p2) = local_tunnels(Option::Some(StarKey::new(2)), Option::Some(StarKey::new(1)));

           let future1 = p1.evolve();
           let future2 = p2.evolve();
           let (result1, result2) = join!( future1, future2 );

           assert!(result1.is_ok());
           assert!(result2.is_ok());
       });
   }

    #[test]
    pub fn lane()
    {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {

            let high = StarKey::new(2);
            let low = StarKey::new(1);

            let mut high_lane = Lane::new(Option::Some(low.clone())).await;
            let mut low_lane = Lane::new(Option::Some(high.clone())).await;

            let connector_ctrl = LocalTunnelConnector::new(&high_lane, &low_lane).await.unwrap();

                high_lane.outgoing.tx.send(LaneCommand::Frame(Frame::Diagnose(FrameDiagnose::Ping) ) ).await;

                let result = low_lane.incoming.recv().await;
                if let Some(StarCommand::Frame(Frame::Diagnose(FrameDiagnose::Ping))) = result
                {
println!("RECEIVED PING!");
                    assert!(true);
                } else if let Some(frame) = result{
println!("RECEIVED {}",frame);
                    assert!(false);
                }
                else
                {
println!("RECEIVED NONE");
                    assert!(false);
                }
            connector_ctrl.command_tx.send(ConnectorCommand::Reset ).await;
            high_lane.outgoing.tx.send(LaneCommand::Frame(Frame::Diagnose(FrameDiagnose::Pong)) ).await;
            let result = low_lane.incoming.recv().await;

            if let Some(StarCommand::Frame(Frame::Diagnose(FrameDiagnose::Pong))) = result
            {
                println!("RECEIVED PoNG!");
                assert!(true);
            } else if let Some(frame) = result{
                println!("RECEIVED {}",frame);
                assert!(false);
            }
            else
            {
                println!("RECEIVED NONE");
                assert!(false);
            }
        });
    }
}


