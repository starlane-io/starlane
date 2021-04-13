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
use crate::lane::{STARLANE_PROTOCOL_VERSION, TunnelSenderState, Lane, TunnelConnector, TunnelSender, LaneCommand, TunnelReceiver, ConnectorController};
use crate::frame::{ProtoFrame, Frame};
use crate::star::{Star, StarKernel, StarKey, StarKind, StarCommand, StarController};
use std::cell::RefCell;
use std::collections::HashMap;
use std::task::Poll;

pub struct ProtoStar
{
  kind: StarKind,
  key: StarKey,
  command_rx: Receiver<StarCommand>,
  lanes: HashMap<StarKey, Lane>,
  connector_ctrls: Vec<ConnectorController>
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
            connector_ctrls: vec![]
        },StarController{
            command_tx: command_tx
        })
    }

    pub async fn evolve(mut self)->Result<Star,Error>
    {
        loop {
            let mut futures = vec!();
            futures.push(self.command_rx.recv().boxed() );

            for (key,mut lane) in &mut self.lanes
            {
               futures.push( lane.incoming.recv().boxed() )
            }

            let (command,_,_) = select_all(futures).await;

            if let Some(command) = command
            {
                match command{
                    StarCommand::AddLane(lane) => {
                        self.lanes.insert(lane.remote_star.clone(), lane);
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::Frame(frame) => {
                        println!("received frame: {}", frame);
                    }
                }
            }
            else
            {
                return Err("command_rx has been disconnected".into());
            }

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
    pub tx: Sender<Frame>,
    pub rx: Receiver<Frame>
}

impl ProtoTunnel
{

    pub async fn evolve(mut self) -> Result<(TunnelSender, TunnelReceiver),Error>
    {
        self.tx.send(Frame::Proto(ProtoFrame::StarLaneProtocolVersion(STARLANE_PROTOCOL_VERSION))).await;

        if let Option::Some(star)=self.star
        {
            self.tx.send(Frame::Proto(ProtoFrame::ReportStarKey(star))).await;
        }

        // first we confirm that the version is as expected
        if let Option::Some(Frame::Proto(recv)) = self.rx.recv().await
        {
            match recv
            {
                ProtoFrame::StarLaneProtocolVersion(version) if version == STARLANE_PROTOCOL_VERSION => {
                    // do nothing... we move onto the next step
                },
                ProtoFrame::StarLaneProtocolVersion(version) => {
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

        if let Option::Some(Frame::Proto(recv)) = self.rx.recv().await
        {

            match recv
            {
                ProtoFrame::ReportStarKey(remote_star_key) => {
                    return Ok((TunnelSender{
                        remote_star: remote_star_key.clone(),
                        tx: self.tx,
                    }, TunnelReceiver{
                        remote_star: remote_star_key.clone(),
                        rx: self.rx,
                        }));
                }
                frame => { return Err(format!("unexpected star gram: {} (expected to receive ReportStarKey next)", frame).into()); }
            };
        }
        else {
            return Err("disconnected!".into())
        }
    }


}

pub fn local_tunnels(high: StarKey, low:StarKey) ->(ProtoTunnel, ProtoTunnel)
{
    let (atx,arx) = mpsc::channel::<Frame>(32);
    let (btx,brx) = mpsc::channel::<Frame>(32);

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
