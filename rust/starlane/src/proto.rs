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
use crate::star::{Star, StarKernel, StarKey, StarKind, StarCommand, StarController, Transaction, StarSearchTransaction, StarCore, StarLogger, StarCoreProvider, FrameTimeoutInner, FrameHold};
use std::cell::RefCell;
use std::collections::HashMap;
use std::task::Poll;
use crate::frame::Frame::{StarMessage, StarSearch};
use crate::template::ConstellationTemplate;
use crate::starlane::StarlaneCommand;
use tokio::time::{Duration, Instant};
use std::ops::Deref;

pub static MAX_HOPS: i32 = 32;

pub struct ProtoStar
{
  star_key: Option<StarKey>,
  sequence: Option<IdSeq>,
  kind: StarKind,
  command_tx: Sender<StarCommand>,
  command_rx: Receiver<StarCommand>,
  evolution_tx: oneshot::Sender<ProtoStarEvolution>,
  lanes: HashMap<StarKey, LaneMeta>,
  connector_ctrls: Vec<ConnectorController>,
  star_core_provider: Arc<dyn StarCoreProvider>,
  logger: StarLogger,
  frame_hold: FrameHold,
  tracker: ProtoTracker
}

impl ProtoStar
{
    pub fn new(key: Option<StarKey>, kind: StarKind, evolution_tx: oneshot::Sender<ProtoStarEvolution>, star_core_provider: Arc<dyn StarCoreProvider>) ->(Self, StarController)
    {
        let (command_tx, command_rx) = mpsc::channel(32);
        (ProtoStar{
            star_key: key,
            sequence: Option::None,
            kind,
            evolution_tx,
            command_tx: command_tx.clone(),
            command_rx: command_rx,
            lanes: HashMap::new(),
            connector_ctrls: vec![],
            star_core_provider: star_core_provider,
            logger: StarLogger::new(),
            frame_hold: FrameHold::new(),
            tracker: ProtoTracker::new(),
        }, StarController{
            command_tx: command_tx
        })
    }

    pub async fn evolve(mut self) -> Result<Star,Error>
    {
        if self.star_key.is_none()
        {
            self.send_expansion_request().await;
        }
        else {
            self.send_sequence_request().await;
        }

        loop {

            // request a sequence from central
            let mut futures = vec!();

            let mut lanes = vec!();
            for (key, mut lane) in &mut self.lanes
            {
                futures.push(lane.lane.incoming.recv().boxed());
                lanes.push( key.clone() )
            }

            futures.push(self.command_rx.recv().boxed());

            if self.tracker.has_expectation()
            {
                futures.push(self.tracker.check().boxed())
            }


            let (command, future_index, _) = select_all(futures).await;

            if let Some(command) = command
            {
                match command {
                    StarCommand::AddLane(lane) => {
println!("Adding Lane!");
                        if let Some(remote_star) = &lane.remote_star
                        {
                            let remote_star = remote_star.clone();
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));

                            if let Option::Some(frames) = self.frame_hold.release(&remote_star)
                            {
                                for frame in frames
                                {
                                    self.send( &remote_star, frame );
                                }
                            }

                            if self.kind.is_central()
                            {
println!("Sending CentralFound!");
                                self.send( &remote_star, Frame::Proto(ProtoFrame::CentralFound));
                            }

                        } else {
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
                        self.tracker.process(&frame);
                        match frame {
                            Frame::Proto(ProtoFrame::CentralFound) => {
println!("Received CentralFound!");
                                let central = StarKey::central();
                                {
                                    let lane_key = lanes.get(future_index).unwrap();
                                    let lane = self.lanes.get_mut(&lane_key).unwrap();
                                    lane.star_paths.insert(central.clone());
                                }
                                if let Option::Some(frames) = self.frame_hold.release(&central )
                                {
                                    for frame in frames
                                    {
                                        self.send( &central, frame );
                                    }
                                }
                            },
                            Frame::Proto(ProtoFrame::GrantSubgraphExpansion(subgraph)) => {
                                let key = StarKey::new_with_subgraph(subgraph.to_owned(), 0);
                                self.star_key = Option::Some(key.clone());

                                self.send_sequence_request().await;
                            },
                            Frame::StarMessage(message) => {
                                if let StarMessagePayload::AssignSequence(sequence) = message.payload
                                {
                                    self.sequence = Option::Some(IdSeq::new(sequence));

                                    self.evolution_tx.send(ProtoStarEvolution {
                                        star: self.star_key.as_ref().unwrap().clone(),
                                        controller: StarController {
                                            command_tx: self.command_tx.clone()
                                        }
                                    });

                                    return Ok(Star::from_proto(self.star_key.as_ref().unwrap().clone(),
                                                                            self.star_core_provider.provide(&self.kind, self.star_key.as_ref().unwrap().clone()),
                                                                            self.command_rx,
                                                                            self.lanes,
                                                                            self.connector_ctrls,
                                                                            self.logger,
                                                                            self.sequence.unwrap(),
                                                                            self.frame_hold)
                                    );
                                }
                            }
                            _ => {
                                println!("frame unsupported by ProtoStar: {}", frame);
                            }
                        }
                    }

                    StarCommand::FrameTimeout(timeout) => {
                        eprintln!("frame timeout: {}.  resending.", timeout.frame);
                        self.broadcast(timeout.frame).await;
                    }
                    _ => {
                        eprintln!("not implemented");
                    }
                }
            } else {
                //            return Err("command_rx has been disconnected".into());
            }
        }

    }

    async fn send_expansion_request( &mut self )
    {
        let frame = Frame::Proto(ProtoFrame::RequestSubgraphExpansion);
        self.tracker.track( frame.clone(),  | frame |{
            if let Frame::Proto( ProtoFrame::GrantSubgraphExpansion(_))  = frame
            {
                return true;
            }
            else
            {
                return false;
            }
        } );

        self.broadcast(frame).await;
    }

    async fn send_sequence_request( &mut self )
    {
        let frame = Frame::StarMessage( StarMessageInner{
            from: self.star_key.as_ref().unwrap().clone(),
            to: StarKey::central(),
            transaction: None,
            payload: StarMessagePayload::RequestSequence
        } );

        self.tracker.track( frame.clone(),  | frame |{
            if let Frame::StarMessage( inner )  = frame
            {
                if let StarMessagePayload::AssignSequence(_) = inner.payload
                {
                    return true;
                }
                else
                {
                    return false;
                }
            }
            else
            {
                return false;
            }
        } );


        println!("sending sequence request.");
        self.send( &StarKey::central(), frame.clone() ).await;
    }


    async fn broadcast(&mut self,  frame: Frame )
    {
        let mut stars = vec!();
        for star in self.lanes.keys()
        {
            stars.push(star.clone());
        }
        for star in stars
        {
            self.send(&star, frame.clone());
        }
    }


    async fn send(&mut self, star: &StarKey, frame: Frame )
    {
        for (remote_star,lane) in &self.lanes
        {
            if lane.has_path_to_star(star)
            {
                lane.lane.outgoing.tx.send( LaneCommand::Frame(frame) ).await;
                return;
            }
        }
        self.frame_hold.add( star, frame );
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

struct ProtoTrackerCase
{
    frame: Frame,
    instant: Instant,
    expect: fn(&Frame)->bool,
    retries: usize
}

impl ProtoTrackerCase
{
    pub fn reset(& mut self )
    {
        self.instant = Instant::now();
    }
}

struct ProtoTracker
{
    case: Option<ProtoTrackerCase>
}

impl ProtoTracker
{
    pub fn new()->Self
    {
        ProtoTracker {
            case: Option::None
        }
    }

    pub fn track( &mut self, frame: Frame, expect: fn(&Frame)->bool)
    {
        self.case = Option::Some(ProtoTrackerCase {
            frame: frame,
            instant: Instant::now(),
            expect: expect,
            retries: 0
        });
    }

    pub fn process( &mut self, frame: &Frame )
    {
        if let Option::Some(case) = &self.case
        {
            if (case.expect)(frame)
            {
                self.case = Option::None;

            }
        }
    }

    pub fn has_expectation(&self)->bool
    {
        return self.case.is_some();
    }

    pub async fn check( &mut self ) -> Option<StarCommand>
    {
        if let Option::Some( case) = &mut self.case
        {
            let now = Instant::now();
            let seconds = 5 - (now.duration_since(case.instant).as_secs() as i64);
            if seconds > 0
            {
                let duration = Duration::from_secs(seconds as u64 );
                tokio::time::sleep(duration).await;
            }

            case.retries = case.retries + 1;

            case.reset();

            Option::Some(StarCommand::FrameTimeout(FrameTimeoutInner { frame: case.frame.clone(), retries: case.retries }))
        }
        else {
            Option::None
        }
    }
}
