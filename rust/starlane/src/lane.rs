use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;

use crate::error::Error;
use crate::id::Id;
use crate::message::{Command, ProtoGram, LaneGram};
use crate::proto::{ProtoTunnel, ProtoStar};
use crate::star::{Star, StarKey};
use crate::starlane::{StarlaneCommand, ConnectCommand};
use crate::starlane::StarlaneCommand::Connect;
use tokio::sync::mpsc::error::SendError;
use futures::FutureExt;
use futures::future::select_all;

pub static STARLANE_PROTOCOL_VERSION :i32 = 1;
pub static LANE_QUEUE_SIZE :usize = 32;

pub struct Lane
{
    pub closed: bool,
    pub remote_star: StarKey,
    pub tunnel: Option<Tunnel>,
    pub tunnel_tx: Sender<Tunnel>,
    pub tunnel_rx: Receiver<Tunnel>,
    pub local_tx: Sender<LaneGram>,
    pub local_rx: Receiver<LaneGram>,
    pub remote_tx: Sender<LaneGram>,
    pub remote_rx: Receiver<LaneGram>
}

impl Lane
{
    pub fn new( star: StarKey )->Self
    {
        let (tunnel_tx,tunnel_rx) = mpsc::channel(2 );
        let (local_tx,local_rx) = mpsc::channel(LANE_QUEUE_SIZE );
        let (remote_tx,remote_rx) = mpsc::channel(LANE_QUEUE_SIZE );
        Lane{
            remote_star: star,
            tunnel: None,
            closed: false,
            tunnel_tx: tunnel_tx,
            tunnel_rx: tunnel_rx,
            local_tx: local_tx,
            local_rx: local_rx,
            remote_tx: remote_tx,
            remote_rx: remote_rx,
        }
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
    pub rx: Receiver<LaneGram>
}

impl Tunnel
{
    pub fn is_closed(&self)->bool
    {
        self.tx.is_closed()
    }
}

#[async_trait]
pub trait LaneMaintainer
{
    async fn run( &mut self );
}

pub struct LocalLaneMaintainer
{
    pub key: StarKey,
    pub tx: Sender<StarlaneCommand>,
}

#[async_trait]
impl LaneMaintainer for LocalLaneMaintainer
{
    async fn run( &mut self ) {
        loop {
            let (tx,rx) = oneshot::channel();
            let mut lookup = ConnectCommand::new(self.key.clone(), tx );
            self.tx.send(StarlaneCommand::Connect(lookup)).await;
            rx.await;
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
    use crate::proto::local_lanes;
    use crate::star::StarKey;

    #[test]
   pub fn test()
   {

       let rt = Runtime::new().unwrap();
       rt.block_on(async {
           let star1id  =     StarKey::new(1);
           let star2id  =     StarKey::new(2);
           let (p1,p2) = local_lanes();
           let future1 = p1.evolve(Option::Some( star1id ));
           let future2 = p2.evolve(Option::Some( star2id ));
           let (result1,result2) = join!( future1, future2 );

           assert!(result1.is_ok());
           assert!(result2.is_ok());
       });



   }
}


