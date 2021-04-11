use futures::future::select_all;
use futures::FutureExt;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
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


#[derive(Clone)]
pub struct LaneController
{
    pub tunnel_tx: Sender<Tunnel>
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
    pub tx: Sender<LaneGram>,
    rx: Receiver<LaneGram>,
    tunnel_rx: Receiver<Tunnel>,
    tunnel: Mutex<Chamber<Tunnel>>
}

impl Lane
{
    pub fn local_lanes(a: StarKey, b: StarKey ) ->((Lane, Lane),(Sender<Tunnel>,Sender<Tunnel>))
    {
        let (a_tunnel_tx, a_tunnel_rx) = mpsc::channel(2);
        let (b_tunnel_tx, b_tunnel_rx) = mpsc::channel(2);

        let (a_tx, a_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (b_tx, b_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        (
            (Lane {
                remote_star: b,
                tx: a_tx,
                rx: a_rx,
                tunnel_rx: a_tunnel_rx,
                tunnel: Mutex::new(Chamber::new() )
            },
            Lane {
                remote_star: a,
                tx: b_tx,
                rx: b_rx,
                tunnel_rx: b_tunnel_rx,
                tunnel: Mutex::new(Chamber::new() )
            }),
            (a_tunnel_tx,b_tunnel_tx)
        )
    }

    pub async fn update(&mut self)
    {
        let rx = self.rx.recv().fuse();
        let tunnel_rx= self.tunnel_rx.recv().fuse();
        pin_mut![rx,tunnel_rx];
        {
        let mut chamber = self.tunnel.lock().await;
          tokio::select! {
            Some(gram) = rx => { }
            Some(tunnel) = tunnel_rx => {
              chamber.holding = Option::Some(tunnel);
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

pub struct LocalTunnelConnector
{
    pub high_star: StarKey,
    pub low_star: StarKey,
    pub high_tunnel_tx: Sender<Tunnel>,
    pub low_tunnel_tx: Sender<Tunnel>
}

impl LocalTunnelConnector
{
    pub fn new(high_star: StarKey, low_star: StarKey, high_tunnel_tx: Sender<Tunnel>, low_tunnel_tx: Sender<Tunnel>) -> Result<Self,Error>
    {
        if high_star.cmp(&low_star) != Ordering::Greater
        {
            Err("High star must have a greater StarKey (meaning higher constelation index array and star index value".into())
        }
        else {
            Ok(LocalTunnelConnector {
                high_star: high_star,
                low_star: low_star,
                high_tunnel_tx: high_tunnel_tx,
                low_tunnel_tx: low_tunnel_tx,
            })
        }
    }
}

#[async_trait]
impl TunnelMaintainer for LocalTunnelConnector
{
    async fn run(&mut self) {
        loop {
            let (mut high, mut low) = local_tunnels(self.high_star.clone(), self.low_star.clone());

            let (high, low) = tokio::join!(high.evolve(),low.evolve());

            if let (Ok(high), Ok(low)) = (high, low)
            {
//                self.high_signal_rx = high.signal_tx.subscribe();
//                self.low_signal_rx = low.signal_tx.subscribe();
                self.high_tunnel_tx.send(high);
                self.low_tunnel_tx.send(low);
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


