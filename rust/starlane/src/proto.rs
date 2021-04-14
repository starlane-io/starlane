use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};

use futures::future::{err, join_all, ok, select_all};
use futures::FutureExt;
use futures::prelude::*;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{mpsc, Mutex, broadcast, oneshot};

use crate::constellation::Constellation;
use crate::error::Error;
use crate::id::{Id, IdSeq};
use crate::lane::{STARLANE_PROTOCOL_VERSION, TunnelSenderState, Lane, TunnelConnector, TunnelSender, LaneCommand, TunnelReceiver, ConnectorController, LaneMeta};
use crate::frame::{ProtoFrame, Frame, StarMessageInner, StarMessagePayload, StarSearchInner, StarSearchPattern, StarSearchResultInner, StarSearchHit};
use crate::star::{Star, StarKernel, StarKey, StarKind, StarCommand, StarController, Transaction, StarSearchTransaction, StarData, StarLogger};
use std::cell::RefCell;
use std::collections::HashMap;
use std::task::Poll;
use crate::frame::Frame::{StarMessage, StarSearch};
use crate::template::ConstellationTemplate;
use crate::starlane::StarlaneCommand;

pub static MAX_HOPS: i32 = 32;

pub struct ProtoStar
{
  kind: StarKind,
  command_tx: Sender<StarCommand>,
  command_rx: Receiver<StarCommand>,
  evolution_tx: oneshot::Sender<ProtoStarEvolution>,
  lanes: HashMap<StarKey, LaneMeta>,
  connector_ctrls: Vec<ConnectorController>,
  logger: StarLogger
}

impl ProtoStar
{
    pub fn new(kind: StarKind, evolution_tx: oneshot::Sender<ProtoStarEvolution>) ->(Self, StarController)
    {
        let (command_tx, command_rx) = mpsc::channel(32);
        (ProtoStar{
            kind,
            evolution_tx,
            command_tx: command_tx.clone(),
            command_rx: command_rx,
            lanes: HashMap::new(),
            connector_ctrls: vec![],
            logger: StarLogger::new()
        }, StarController{
            command_tx: command_tx
        })
    }

    pub async fn evolve(mut self) -> Result<Star,Error>
    {
        // request a sequence from central
        loop {
            let mut futures = vec!();
            futures.push(self.command_rx.recv().boxed() );

            for (key,mut lane) in &mut self.lanes
            {
               futures.push( lane.lane.incoming.recv().boxed() )
            }

            let (command,_,_) = select_all(futures).await;

            if let Some(command) = command
            {
                match command{
                    StarCommand::AddLane(lane) => {
                        if let Some(remote_star) = &lane.remote_star
                        {
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));
                        }
                        else {
                            eprintln!("cannot add a lane to a star that doesn't have a remote_star");
                        }
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::AddLogger(logger) => {
                       self.logger.tx.push(logger);
                    }
                    StarCommand::Frame(frame) => {
                        match frame {
                            Frame::GrantSubgraphExpansion(subgraph) => {
                                let key = StarKey::new_with_subgraph(subgraph,0);
                                self.evolution_tx.send( ProtoStarEvolution{ star: key.clone(), controller: StarController {
                                    command_tx: self.command_tx.clone()
                                } });

                                return Ok(Star::from_proto( key.clone(),
                                                         self.kind.clone(),
                                                              self.command_rx,
                                                              self.lanes,
                                                              self.connector_ctrls,
                                                              self.logger
                                                              ));
                            },
                            _ => {
                                println!("frame unsupported by ProtoStar: {}",frame );
                            }
                        }
                    }
                    _ => {
                        eprintln!("not implemented");
                    }

                }
            }
            else
            {
    //            return Err("command_rx has been disconnected".into());
            }
        }
    }


    async fn send(&mut self, star: &StarKey, frame: Frame )
    {
        for (remote_star,lane) in &self.lanes
        {
            if lane.has_path_to_star(star)
            {
                lane.lane.outgoing.tx.send( LaneCommand::LaneFrame(frame) ).await;
                return;
            }
        }
        eprintln!("could not find star for frame: {}", frame );
    }


    async fn process_frame( &mut self, frame: Frame, lane: &mut LaneMeta )
    {
        match frame
        {
            _ => {
                eprintln!("star does not handle frame: {}", frame)
            }
        }
    }

}

pub struct ProtoStarEvolution
{
    pub star: StarKey,
    pub controller: StarController
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

pub fn local_tunnels(high: Option<StarKey>, low:Option<StarKey>) ->(ProtoTunnel, ProtoTunnel)
{
    let (atx,arx) = mpsc::channel::<Frame>(32);
    let (btx,brx) = mpsc::channel::<Frame>(32);

    (ProtoTunnel {
        star: high,
        tx: atx,
        rx: brx
    },
     ProtoTunnel
    {
        star: low,
        tx: btx,
        rx: arx
    })
}
