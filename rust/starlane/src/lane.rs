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
use crate::message::{Command, LaneGram, ProtoGram};
use crate::proto::{local_tunnels, ProtoStar, ProtoTunnel};
use crate::star::{Star, StarKey};
use crate::starlane::{ConnectCommand, StarlaneCommand};
use crate::starlane::StarlaneCommand::Connect;

pub static STARLANE_PROTOCOL_VERSION: i32 = 1;
pub static LANE_QUEUE_SIZE: usize = 32;

#[derive(Clone)]
pub struct LaneController
{
    pub command_tx: Sender<LaneCommand>
}

pub enum LaneCommand
{
    TunnelState(TunnelState)
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
    command_rx: Receiver<LaneCommand>,
    tunnel_state: TunnelState
}

impl Lane
{
    pub fn local_lanes(a: StarKey, b: StarKey ) ->(Lane, Lane,LaneController,LaneController)
    {
        let (a_tunnel_tx, a_tunnel_rx) = mpsc::channel(1);
        let (b_tunnel_tx, b_tunnel_rx) = mpsc::channel(1);

        let (a_tx, a_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        let (b_tx, b_rx) = mpsc::channel(LANE_QUEUE_SIZE);
        (
            Lane {
                remote_star: b,
                tx: a_tx,
                rx: a_rx,
                command_rx: a_tunnel_rx,
                tunnel_state: TunnelState::None
            },
            Lane {
                remote_star: a,
                tx: b_tx,
                rx: b_rx,
                command_rx: b_tunnel_rx,
                tunnel_state: TunnelState::None
            },
            LaneController{
                command_tx: a_tunnel_tx
            },
            LaneController{
                command_tx: b_tunnel_tx
            }
        )
    }

    pub async fn run(&mut self)
    {

        loop
        {
            if let TunnelState::Tunnel(tunnel_ctrl) = &self.tunnel_state
            {
                  let mut rx = self.rx.recv().boxed();
                  let mut command_rx = self.command_rx.recv().boxed();
                  tokio::select! {
                   command = command_rx => {
                          match command{
                            Option::Some(command)=>{
                                match command
                                {
                                    LaneCommand::TunnelState(tunnel_state) => {self.tunnel_state=tunnel_state;}
                                }
                            }
                            Option::None=>{
                                eprintln!("lane command stream ended.");
                            }
                        }
                    }
                  }
            }
            else
            {
                match self.command_rx.recv().await
                {
                    Option::Some(command)=>{
                        match command
                        {
                            LaneCommand::TunnelState(tunnel_state) => {self.tunnel_state=tunnel_state;}
                        }
                    }
                    Option::None=>{
                        eprintln!("lane command stream ended.");
                    }
                }
            }
        }
    }

    async fn process_command( &mut self, command: Option<LaneCommand> )
    {}
}

pub struct Tunnel
{
    pub remote_star: StarKey,
    pub tx: Sender<LaneGram>,
    pub rx: Receiver<LaneGram>,
    pub close_signal_rx: oneshot::Receiver<()>
}

impl Tunnel
{
    pub fn is_closed(&self)->bool
    {
        self.tx.is_closed()
    }

    pub async fn run(mut self)
    {
       self.close_signal_rx.await;
       self.rx.close();
       self.tx.send(LaneGram::Close).await;
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
    pub tx: Sender<LaneGram>
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
    pub high_lane_ctrl: LaneController,
    pub low_lane_ctrl: LaneController,
    pub command_rx: mpsc::Receiver<ConnectorCommand>
}

impl LocalTunnelConnector
{
    pub fn new(high_star: StarKey, low_star: StarKey, high_lane_ctrl: LaneController, low_lane_ctrl: LaneController ) -> Result<(Self, ConnectorController),Error>
    {
        if high_star.cmp(&low_star) != Ordering::Greater
        {
            Err("High star must have a greater StarKey (meaning higher constellation index array and star index value".into())
        }
        else {
            let (command_tx,command_rx) = mpsc::channel(1);

            Ok((LocalTunnelConnector {
                high_star: high_star,
                low_star: low_star,
                command_rx: command_rx,
                high_lane_ctrl: high_lane_ctrl,
                low_lane_ctrl: low_lane_ctrl,
            },ConnectorController{ command_tx: command_tx}))
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

            if let (Ok((high_tunnel, high_tunnel_ctrl)), Ok((low_tunnel, low_tunnel_ctrl))) = (high, low)
            {
                self.high_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::Tunnel(high_tunnel_ctrl)));
                self.low_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::Tunnel(low_tunnel_ctrl)));
            }
            else {
                eprintln!("connection failure... trying again in 10 seconds");
                tokio::time::sleep(Duration::from_secs(10)).await;
            }

                // then wait for next command
                match self.command_rx.recv().await
                {
                    None => {
                        self.high_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::None));
                        self.low_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::None));
                        return;
                    }
                    Some(Reset) => {
                        // first set olds to None
                        self.high_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::None));
                        self.low_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::None));
                        // allow loop to continue
                    }
                    Some(Close) => {
                        self.high_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::None));
                        self.low_lane_ctrl.command_tx.send(LaneCommand::TunnelState(TunnelState::None));
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


