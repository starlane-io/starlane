use std::sync::Arc;
use std::sync::atomic::AtomicI32;

use futures::future::{err, join_all, ok, select_all};
use futures::FutureExt;
use futures::prelude::*;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex, broadcast, oneshot};

use crate::constellation::Constellation;
use crate::error::Error;
use crate::id::Id;
use crate::lane::{STARLANE_PROTOCOL_VERSION, Tunnel, Lane, TunnelConnector, TunnelController};
use crate::message::{ProtoGram, LaneGram};
use crate::star::{Star, StarKernel, StarKey, StarKind, StarCommand, StarController};
use std::cell::RefCell;
use std::collections::HashMap;
use std::task::Poll;

pub struct ProtoStar
{
  kind: StarKind,
  key: StarKey,
  command_rx: Receiver<StarCommand>,
  lanes: HashMap<StarKey,Lane>,
  connectors: Vec<Box<dyn TunnelConnector>>
}

impl ProtoStar
{
    pub fn new(key: StarKey, kind: StarKind) ->(Self, StarController)
    {
        let (command_tx, command_rx) = mpsc::channel(32);
        (ProtoStar{
            kind,
            key,
            command_rx: command_rx,
            lanes: HashMap::new(),
            connectors: vec![]
        },StarController{
            command_tx: command_tx
        })
    }

    pub async fn evolve(mut self)->Result<Star,Error>
    {
        loop {
            let mut futures = vec!();
//            futures.push(self.command().boxed());
            for (key,mut lane) in &mut self.lanes
            {
                futures.push(lane.run().boxed());
            }

            let result = select_all(futures).await;
        }

        Ok(Star::new( self.lanes, Box::new(PlaceholderKernel::new()) ))
    }

    async fn command(&mut self)
    {
        self.command_rx.recv().await;
    }


}

pub struct ProtoStarController
{
    command_tx: Sender<StarCommand>
}


#[derive(Clone)]
pub enum ProtoStarKernel
{
   Central,
   Mesh,
   Supervisor,
   Server,
   Gateway
}


impl ProtoStarKernel
{
    fn evolve(&self) -> Result<Box<dyn StarKernel>, Error>
    {
        Ok(Box::new(PlaceholderKernel::new()))
    }
}


pub struct PlaceholderKernel
{

}

impl PlaceholderKernel{
    pub fn new()->Self
    {
        PlaceholderKernel{}
    }
}

impl StarKernel for PlaceholderKernel
{

}


pub struct ProtoTunnel
{
    pub star: Option<StarKey>,
    pub tx: Sender<LaneGram>,
    pub rx: Receiver<LaneGram>,
}

impl ProtoTunnel
{

    pub async fn evolve(mut self) -> Result<(Tunnel, TunnelController),Error>
    {
        self.tx.send(LaneGram::Proto(ProtoGram::StarLaneProtocolVersion(STARLANE_PROTOCOL_VERSION))).await;

        if let Option::Some(star)=self.star
        {
            self.tx.send(LaneGram::Proto(ProtoGram::ReportStarKey(star))).await;
        }

        // first we confirm that the version is as expected
        if let Option::Some(LaneGram::Proto(recv)) = self.rx.recv().await
        {
            match recv
            {
                ProtoGram::StarLaneProtocolVersion(version) if version == STARLANE_PROTOCOL_VERSION => {
                    // do nothing... we move onto the next step
                },
                ProtoGram::StarLaneProtocolVersion(version) => {
                    return Err(format!("wrong version: {}", version).into());
                },
                gram => {
                    return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)", gram).into());
                }
            }
        }
        else {
            return Err("disconnected".into());
        }

        if let Option::Some(LaneGram::Proto(recv)) = self.rx.recv().await
        {
            match recv
            {
                ProtoGram::ReportStarKey(remote_star_key) => {
                    let (close_signal_tx,close_signal_rx) = oneshot::channel();

                    return Ok((Tunnel{
                        remote_star: remote_star_key,
                        rx: self.rx,
                        tx: self.tx.clone(),
                        close_signal_rx: close_signal_rx
                    }, TunnelController {
                        tx: self.tx,
                        close_signal_tx:close_signal_tx}));
                }
                gram => { return Err(format!("unexpected star gram: {} (expected to receive ReportStarKey next)", gram).into()); }
            };
        }
        else {
            return Err("disconnected!".into())
        }
    }
}

pub fn local_tunnels(high: StarKey, low:StarKey) ->(ProtoTunnel, ProtoTunnel)
{
    let (atx,arx) = mpsc::channel::<LaneGram>(32);
    let (btx,brx) = mpsc::channel::<LaneGram>(32);

    (ProtoTunnel {
        star: Option::Some(high),
        tx: atx,
        rx: brx
    },
     ProtoTunnel
    {
        star: Option::Some(low),
        tx: btx,
        rx: arx
    })
}
