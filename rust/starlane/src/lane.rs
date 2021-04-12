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

use crate::error::Error;
use crate::id::Id;
use crate::frame::{Command, LaneFrame, ProtoFrame};
use crate::proto::{local_tunnels, ProtoStar, ProtoTunnel};
use crate::star::{Star, StarKey};
use crate::starlane::{ConnectCommand, StarlaneCommand};
use crate::starlane::StarlaneCommand::Connect;
use std::fmt;

pub static STARLANE_PROTOCOL_VERSION: i32 = 1;
pub static LANE_QUEUE_SIZE: usize = 32;

#[derive(Clone)]
pub struct OutgoingLane
{
    pub tx: Sender<LaneCommand>
}

pub struct IncomingLane
{
    rx: Receiver<LaneFrame>
}

pub struct IncomingLaneRunner
{
    rx: Receiver<LaneFrame>
}

impl IncomingLaneRunner
{
   pub async fn run(mut self)
   {

   }
}

pub struct OutgoingLaneRunner
{
    rx: Receiver<LaneCommand>,
    tx: Sender<LaneFrame>,
    tunnel: TunnelSenderState,
    queue: Vec<LaneFrame>
}

impl OutgoingLaneRunner
{
    async fn die(&self, message: String)
    {
        eprintln!("{}",message.as_str());
    }

    pub async fn run(mut self)
    {
println!("running Mid.run()");
        while let Option::Some(command) = self.rx.recv().await {
            match command
            {
                LaneCommand::Tunnel(tunnel) => {
println!("new Tunnel: {}",tunnel);
                    if let TunnelSenderState::Tunnel(tunnel) = &tunnel
                    {
                        for frame in self.queue.drain(..)
                        {
println!("flushing frame {}",frame);
                            tunnel.tx.send(frame).await;
                        }
                    }
                    self.tunnel = tunnel;
                }
                LaneCommand::LaneFrame(frame) => {
                    match &self.tunnel {
                        TunnelSenderState::Tunnel(tunnel) => {
println!("relaying frame to tunnel");
                            tunnel.tx.send(frame).await;
                        }
                        TunnelSenderState::None => {
println!("adding frame to queue since Tunnel is not ready...");
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
    LaneFrame(LaneFrame)
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
    pub remote_star: StarKey,
    pub incoming: IncomingLane,
    pub outgoing: OutgoingLane,
}

impl Lane
{
    pub async fn new(remote_star: StarKey) -> Self
    {
        let (mid_tx, mid_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (in_tx, in_rx) = mpsc::channel(LANE_QUEUE_SIZE);

        let midlane = OutgoingLaneRunner {
            rx: mid_rx,
            tx: in_tx,
            tunnel: TunnelSenderState::None,
            queue: vec![]
        };

        tokio::spawn( async move { midlane.run().await; } );
            Lane{
                remote_star: remote_star,
                incoming: IncomingLane{
                    rx: in_rx
                },
                outgoing: OutgoingLane {
                    tx: mid_tx
                },
            }
    }

}





pub enum TunnelSenderState
{
    Sender(TunnelSender),
    None
}

pub enum TunnelReceiverState
{
    Tunnel(TunnelReceiver),
    None
}

impl fmt::Display for TunnelSenderState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            TunnelSenderState::Tunnel(_) => "Tunnel".to_string(),
            TunnelSenderState::None => "None".to_string()
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone)]
pub struct TunnelSender
{
    pub tx: Sender<LaneFrame>
}

pub struct TunnelReceiver
{
    pub rx: Receiver<LaneFrame>
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
    pub high_star: StarKey,
    pub low_star: StarKey,
    pub high: OutgoingLane,
    pub low: OutgoingLane,
    command_rx: Receiver<ConnectorCommand>
}

impl LocalTunnelConnector
{
    pub fn new(high_lane: &Lane, low_lane: &Lane ) -> Result<(Self, ConnectorController),Error>
    {
        let high_star = low_lane.remote_star.clone();
        let low_star = high_lane.remote_star.clone();
println!("High {:?} Low {:?}",high_star.clone(),low_star.clone());
        if high_star.cmp(&low_star ) != Ordering::Greater
        {
            Err("High star must have a greater StarKey (meaning higher constellation index array and star index value".into())
        }
        else {
            let (command_tx,command_rx) = mpsc::channel(1);

            Ok((LocalTunnelConnector {
                high_star: high_star.clone(),
                low_star: low_star.clone(),
                high: high_lane.outgoing.clone(),
                low: low_lane.outgoing.clone(),
                command_rx: command_rx
            },ConnectorController{ command_tx: command_tx}))
        }
    }
}

#[async_trait]
impl TunnelConnector for LocalTunnelConnector
{
    async fn run(&mut self) {
println!("Entering LocalTunnelConnector.run()");
        loop {
            let (mut high, mut low) = local_tunnels(self.high_star.clone(), self.low_star.clone());

            let (high, low) = tokio::join!(high.evolve(),low.evolve());

            if let (Ok((high_tunnel, high_tunnel_ctrl)), Ok((low_tunnel, low_tunnel_ctrl))) = (high, low)
            {

println!("Sending high tunnel");
                self.high.tx.send(LaneCommand::Tunnel(TunnelSenderState::Tunnel(high_tunnel))).await;
println!("Sending low tunnel");
                self.low.tx.send(LaneCommand::Tunnel(TunnelSenderState::Tunnel(low_tunnel))).await;
println!("all tunnels sent");
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
                        return; }
                }

        }
    }
}

#[cfg(test)]
mod test
{
    use futures::FutureExt;
    use tokio::runtime::Runtime;

    use crate::error::Error;
    use crate::id::Id;
    use crate::frame::ProtoFrame;
    use crate::proto::local_tunnels;
    use crate::star::StarKey;
    use crate::lane::{OutgoingLaneRunner, Lane, LaneCommand};
    use crate::lane::LocalTunnelConnector;
    use crate::lane::TunnelConnector;
    use crate::lane::ConnectorCommand;
    use crate::lane::LaneFrame;
    use tokio::time::Duration;

    #[test]
   pub fn proto_tunnel()
   {
       let rt = Runtime::new().unwrap();
       rt.block_on(async {
           let (mut p1, mut p2) = local_tunnels(StarKey::new(2), StarKey::new(1));

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

            let mut high_lane = Lane::new(low.clone()).await;
            let mut low_lane = Lane::new(high.clone()).await;


            println!("pre...");
            let (mut connector, connector_ctrl) = LocalTunnelConnector::new(&high_lane, &low_lane).unwrap();
            tokio::spawn( async move { connector.run().await } );

            println!("sending PING");
            high_lane.outgoing.tx.send(LaneCommand::LaneFrame(LaneFrame::Ping) ).await;

            /*
            match low_lane.incoming.rx.recv().await
            {
                None => {assert!(false);}
                Some(frame) => {
                    if let LaneFrame::Ping = frame
                    {
println!("received ping.");
                        assert!(true);
                    }
                    else {
                        assert!(false);
                    }
                }
            }

             */

            tokio::time::sleep(Duration::from_secs(10)).await;
        });
    }
}


