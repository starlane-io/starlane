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
use crate::message::{Command, LaneFrame, ProtoFrame};
use crate::proto::{local_tunnels, ProtoStar, ProtoTunnel};
use crate::star::{Star, StarKey};
use crate::starlane::{ConnectCommand, StarlaneCommand};
use crate::starlane::StarlaneCommand::Connect;

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

pub struct MidLane
{
    rx: Receiver<LaneCommand>,
    tx: Sender<LaneFrame>,
    tunnel_state: TunnelState
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
                    self.tunnel_state = tunnel;
                }
                LaneCommand::LaneFrame(frame) => {
                    match &self.tunnel_state{
                        TunnelState::Tunnel(tunnel) => {
                            tunnel.tx.send(frame).await;
                        }
                        TunnelState::None => {
                            eprintln!("BAD! tunnel is not set, so FRAME was DROPPED!");
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
    Tunnel(TunnelState),
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
    mid: Option<MidLane>
}

impl Lane
{
    pub fn new(remote_star: StarKey) -> Self
    {
        let (a_to_mid_tx, a_to_mid_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (a_to_in_tx, a_to_in_rx) = mpsc::channel(LANE_QUEUE_SIZE);

            Lane{
                remote_star: remote_star,
                incoming: IncomingLane{
                    rx: a_to_in_rx
                },
                outgoing: OutgoingLane {
                    tx: a_to_mid_tx
                },
                mid: Option::Some(MidLane {
                    rx: a_to_mid_rx,
                    tx: a_to_in_tx,
                    tunnel_state: TunnelState::None
                })
            }
    }

    pub async fn start(mut self) ->Self
    {
        if let Some(mut mid)=self.mid
        {
            self.mid = Option::None;
            tokio::spawn(async move { mid.run(); } );
        }
        self
    }


}





pub struct Tunnel
{
    pub remote_star: StarKey,
    pub tx: Sender<LaneFrame>,
    pub rx: Receiver<LaneFrame>,
}

impl Tunnel
{
    pub fn is_closed(&self)->bool
    {
        self.tx.is_closed()
    }
}

#[derive(Clone)]
pub enum TunnelState
{
    Tunnel(TunnelController),
    None
}

#[derive(Clone)]
pub struct TunnelController
{
    pub tx: Sender<LaneFrame>
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
                self.high.tx.send(LaneCommand::Tunnel(TunnelState::Tunnel(high_tunnel_ctrl))).await;
println!("Sending low tunnel");
                self.low.tx.send(LaneCommand::Tunnel(TunnelState::Tunnel(low_tunnel_ctrl))).await;
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
                        self.high.tx.send(LaneCommand::Tunnel(TunnelState::None)).await;
                        self.low.tx.send(LaneCommand::Tunnel(TunnelState::None)).await;
                        return;
                    }
                    Some(Reset) => {
                        // first set olds to None
                        self.high.tx.send(LaneCommand::Tunnel(TunnelState::None)).await;
                        self.low.tx.send(LaneCommand::Tunnel(TunnelState::None)).await;
                        // allow loop to continue
                    }
                    Some(Close) => {
                        self.high.tx.send(LaneCommand::Tunnel(TunnelState::None)).await;
                        self.low.tx.send(LaneCommand::Tunnel(TunnelState::None)).await;
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
    use crate::message::ProtoFrame;
    use crate::proto::local_tunnels;
    use crate::star::StarKey;
    use crate::lane::{MidLane, Lane, LaneCommand};
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

            let high_lane = Lane::new(StarKey::new(2));
            let low_lane = Lane::new(StarKey::new(1));

            let (mut connector ,connector_ctrl) = LocalTunnelConnector::new(&high_lane, &low_lane ).unwrap();
            tokio::spawn( async move { connector.run().await } );

            high_lane.outgoing.tx.send(LaneCommand::LaneFrame(LaneFrame::Ping) ).await;

            tokio::time::sleep(Duration::from_secs(10)).await;
        });
    }
}


