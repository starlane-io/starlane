use futures::future::select_all;
use futures::FutureExt;
use tokio::sync::{broadcast, mpsc};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::sync::oneshot;
use tokio::time::Duration;

use crate::error::Error;
use crate::id::Id;
use crate::message::{Command, LaneGram, ProtoGram};
use crate::proto::{local_tunnels, ProtoStar, ProtoTunnel};
use crate::star::{Star, StarKey};
use crate::starlane::{ConnectCommand, StarlaneCommand};
use crate::starlane::StarlaneCommand::Connect;
use std::cmp::Ordering;

pub static STARLANE_PROTOCOL_VERSION: i32 = 1;
pub static LANE_QUEUE_SIZE: usize = 32;


pub struct LaneController
{
    pub tunnel_tx: Sender<Tunnel>
}

pub struct Lane
{
    pub remote_star: StarKey,
    pub tx: Sender<LaneGram>,
    //pub rx: Receiver<LaneGram>
}

pub struct LaneRunner
{
    pub closed: bool,
    pub tunnel: Option<Tunnel>,
    pub tunnel_rx: Receiver<Tunnel>,
    pub remote_rx: Receiver<LaneGram>,
}

impl LaneRunner
{
    pub fn new(remote_star: StarKey) -> (Self, LaneController, Lane)
    {
        let (tunnel_tx, tunnel_rx) = mpsc::channel(2);
//        let (local_tx,local_rx) = mpsc::channel(LANE_QUEUE_SIZE );
        let (remote_tx, remote_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        (LaneRunner {
            tunnel: None,
            closed: false,
            tunnel_rx: tunnel_rx,
            remote_rx: remote_rx,
        },
         LaneController {
             tunnel_tx: tunnel_tx
         },
         Lane {
             remote_star,
             tx: remote_tx,
         })
    }

    pub fn is_closed(&self)->bool
    {
        self.closed
    }

    fn has_working_tunnel(&self)->bool
    {
        self.tunnel.is_some() && !self.tunnel.as_ref().unwrap().is_closed()
    }

    pub async fn run(&mut self)
    {
        while !self.is_closed()
        {
            let mut tunnel_future = self.tunnel_rx.recv().boxed();
            let mut gram_future = self.remote_rx.recv().boxed();
            tokio::select!{
               tunnel = &mut tunnel_future => {
                 self.tunnel = tunnel;
               }

               Option::Some(gram) = &mut gram_future => {
                    if let Option::Some(tunnel) = &self.tunnel
                    {
                        tunnel.tx.send(gram).await;
                    }
               }
            }
        }
    }
}

pub struct Tunnel
{
    pub remote_star: StarKey,
    pub tx: Sender<LaneGram>,
    pub rx: Receiver<LaneGram>,
    pub signal_tx: broadcast::Sender<LaneSignal>,
}

impl Tunnel
{
    pub fn is_closed(&self)->bool
    {
        self.tx.is_closed()
    }
}

#[async_trait]
pub trait TunnelMaintainer
{
    async fn run(&mut self);
}

#[derive(Clone)]
pub enum LaneSignal
{
    Close
}

pub struct LocalTunnelMaintainer
{
    pub high_star: StarKey,
    pub low_star: StarKey,
    pub high_lane_ctrl: LaneController,
    pub low_lane_ctrl: LaneController,
    high_signal_rx: broadcast::Receiver<LaneSignal>,
    low_signal_rx: broadcast::Receiver<LaneSignal>,
}

impl LocalTunnelMaintainer
{
    pub fn new(high_star: StarKey, low_star: StarKey, high_lane_ctrl: LaneController, low_lane_ctrl: LaneController, high_signal_rx: broadcast::Receiver<LaneSignal>, low_signal_rx: broadcast::Receiver<LaneSignal>) -> Result<Self,Error>
    {
        if high_star.cmp(&low_star) != Ordering::Greater
        {
            Err("High star must have a greater StarKey (meaning higher constelation index array and star index value".into())
        }
        else {
            Ok(LocalTunnelMaintainer {
                high_star: high_star,
                low_star: low_star,
                high_lane_ctrl: high_lane_ctrl,
                low_lane_ctrl: low_lane_ctrl,
                high_signal_rx: high_signal_rx,
                low_signal_rx: low_signal_rx,
            })
        }
    }
}

#[async_trait]
impl TunnelMaintainer for LocalTunnelMaintainer
{
    async fn run(&mut self) {
        loop {
            let (mut high, mut low) = local_tunnels(self.high_star.clone(), self.low_star.clone());

            let (high, low) = tokio::join!(high.evolve(),low.evolve());

            if let (Ok(high), Ok(low)) = (high, low)
            {
                self.high_signal_rx = high.signal_tx.subscribe();
                self.low_signal_rx = low.signal_tx.subscribe();
                self.high_lane_ctrl.tunnel_tx.send(high);
                self.low_lane_ctrl.tunnel_tx.send(low);
            } else {
                eprintln!("connection failure... trying again in 10 seconds");
                tokio::time::sleep(Duration::from_secs(10)).await;
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
    use crate::message::ProtoGram;
    use crate::proto::local_tunnels;
    use crate::star::StarKey;

    #[test]
   pub fn test()
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
}


