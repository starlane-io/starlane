use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};
use std::task::Poll;

use futures::future::{err, join_all, ok, select_all};
use futures::FutureExt;
use futures::prelude::*;
use tokio::sync::{broadcast, mpsc, Mutex, oneshot};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{Duration, Instant};

use crate::constellation::Constellation;
use crate::core::StarCoreFactory;
use crate::error::Error;
use crate::frame::{Frame, ProtoFrame, WindHit, StarMessage, StarMessagePayload, WindUp, StarPattern, WindDown, SequenceMessage, ProtoEvolution, ProtoSequence};
use crate::id::{Id, IdSeq};
use crate::lane::{ConnectorController, Lane, LaneCommand, LaneMeta, STARLANE_PROTOCOL_VERSION, TunnelConnector, TunnelReceiver, TunnelSender, TunnelSenderState};
use crate::star::{FrameHold, FrameTimeoutInner, ShortestPathStarKey, Star, StarCommand, StarController, StarInfo, StarKernel, StarKey, StarKind, StarManagerFactory, StarSearchTransaction, Transaction};
use crate::starlane::StarlaneCommand;
use crate::template::ConstellationTemplate;
use crate::logger::Logger;
use crate::frame::ProtoFrame::Evolution;

pub static MAX_HOPS: i32 = 32;

pub struct ProtoStar
{
  star_key: Option<StarKey>,
  sequence: Option<Arc<IdSeq>>,
  kind: StarKind,
  command_tx: mpsc::Sender<StarCommand>,
  command_rx: mpsc::Receiver<StarCommand>,
  evolution_tx: oneshot::Sender<ProtoStarEvolution>,
  lanes: HashMap<StarKey, LaneMeta>,
  connector_ctrls: Vec<ConnectorController>,
  star_manager_factory: Arc<dyn StarManagerFactory>,
  star_core_factory: Arc<dyn StarCoreFactory>,
  logger: Logger,
  frame_hold: FrameHold,
  tracker: ProtoTracker
}

impl ProtoStar
{
    pub fn new(key: Option<StarKey>, kind: StarKind, evolution_tx: oneshot::Sender<ProtoStarEvolution>, star_manager_factory: Arc<dyn StarManagerFactory>, star_core_factory: Arc<dyn StarCoreFactory>) ->(Self, StarController)
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
            star_manager_factory: star_manager_factory,
            star_core_factory: star_core_factory,
            logger: Logger::new(),
            frame_hold: FrameHold::new(),
            tracker: ProtoTracker::new(),
        }, StarController{
            command_tx: command_tx
        })
    }

    pub async fn evolve(mut self) -> Result<Star,Error>
    {
        if self.kind.is_central()
        {
            let sequence = Arc::new(IdSeq::new(0));
            self.star_key = Option::Some(StarKey::central());
            self.sequence = Option::Some(sequence.clone());
            let info = StarInfo{
                star_key: self.star_key.as_ref().unwrap().clone(),
                kind: self.kind.clone(),
                sequence: self.sequence.as_ref().unwrap().clone(),
                command_tx: self.command_tx.clone()
            };

            let manager_tx= self.star_manager_factory.create(info.clone() ).await;
            let core_tx = self.star_core_factory.create(&info.kind,manager_tx.clone());


            return Ok(Star::from_proto(info.clone(),
                                       self.command_rx,
                                       manager_tx,
                                       core_tx,
                                       self.lanes,
                                       self.connector_ctrls,
                                       self.logger,
                                       self.frame_hold ));
        }
        else {
            self.send_central_search().await;
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
                        if let Some(remote_star) = &lane.remote_star
                        {
                            let remote_star = remote_star.clone();
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));

                            if let Option::Some(frames) = self.frame_hold.release(&remote_star)
                            {
                                for frame in frames
                                {
                                    self.send_frame(&remote_star, frame ).await;
                                }
                            }

                            self.broadcast( Frame::Proto(ProtoFrame::CentralSearch), &Option::None ).await;
                            self.broadcast( Frame::Proto(ProtoFrame::Evolution(ProtoEvolution::Request)), &Option::None ).await;

                        } else {
                            eprintln!("cannot add a lane to a star that doesn't have a remote_star");
                        }
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::AddLogger(logger) => {
//                        self.logger =
                    }
                    StarCommand::Frame(frame) => {
                        self.tracker.process(&frame);
                        let lane_key = lanes.get(future_index).unwrap();
                        let lane = self.lanes.get_mut(&lane_key).unwrap();
                        match frame {
                            Frame::Proto(ProtoFrame::CentralSearch) => {
                                if let Option::Some(hops) = self.get_hops_to_star(&StarKey::central())
                                {
                                    self.broadcast( Frame::Proto(ProtoFrame::CentralFound(hops + 1)), &Option::None ).await;
                                }
                           }
                            Frame::Proto(ProtoFrame::CentralFound(hops)) => {

                                if Option::None == lane.star_paths.get(&StarKey::central())
                                {
                                    lane.star_paths.put(StarKey::central(), hops);
                                    //now tell all the other lanes that CENTRAL is this way...
                                    {
                                        let mut exclude = HashSet::new();
                                        exclude.insert(lane_key.clone());
                                        let exclude = Option::Some(exclude);
                                        self.broadcast(Frame::Proto(ProtoFrame::CentralFound(hops + 1)), &exclude).await;
                                    }
                                    self.send_sequence_request().await;
                                }
                            },
                            Frame::Proto(ProtoFrame::GrantSubgraphExpansion(subgraph)) => {
                                let key = StarKey::new_with_subgraph(subgraph.to_owned(), 0);
                                self.star_key = Option::Some(key.clone());

                                self.send_central_search().await;
                            },
                            Frame::Proto(ProtoFrame::Evolution(ProtoEvolution::Request)) =>
                            {
                                // ignore
                            },
                            Frame::Proto(ProtoFrame::Evolution(ProtoEvolution::Report)) =>
                            {
                                // a nearby evolution triggers a send sequence request
                                self.send_sequence_request().await;
                            },
                            Frame::Proto(ProtoFrame::Sequence(ProtoSequence::Reply(sequence))) =>
                            {
                                self.sequence = Option::Some(Arc::new(IdSeq::new(sequence)));
                                let info = StarInfo{
                                    star_key: self.star_key.as_ref().unwrap().clone(),
                                    kind: self.kind.clone(),
                                    sequence: self.sequence.as_ref().unwrap().clone(),
                                    command_tx: self.command_tx.clone()
                                };

                                let manager_tx= self.star_manager_factory.create(info.clone() ).await;
                                let core_tx = self.star_core_factory.create(&info.kind,manager_tx.clone());


println!("{} .... EVOLVED .... ", self.kind);
                                return Ok(Star::from_proto(info.clone(),
                                                           self.command_rx,
                                                           manager_tx,
                                                           core_tx,
                                                           self.lanes,
                                                           self.connector_ctrls,
                                                           self.logger,
                                                           self.frame_hold ));

                            },


                            _ => {
                                println!("{} frame unsupported by ProtoStar: {}", self.kind, frame);
                            }
                        }
                    }

                    StarCommand::FrameTimeout(timeout) => {
                        eprintln!("frame timeout: {}.  resending {} retry.", timeout.frame, timeout.retries);
                        self.resend(timeout.frame).await;
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

    async fn send_central_search(&mut self )
    {
        let frame = Frame::Proto(ProtoFrame::CentralSearch);
        self.tracker.track( frame.clone(),  | frame |{
            if let Frame::Proto( ProtoFrame::CentralFound(_))  = frame
            {
                return true;
            }
            else
            {
                return false;
            }
        } );

        self.broadcast(frame, &Option::None ).await;
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

        self.broadcast(frame, &Option::None ).await;
    }

    async fn send_sequence_request( &mut self )
    {
        self.send_frame(&StarKey::central(), Frame::Proto(ProtoFrame::Sequence(ProtoSequence::Request)) ).await;
    }

    async fn resend(&mut self,  frame: Frame)
    {
        match frame
        {
            Frame::Proto(ProtoFrame::RequestSubgraphExpansion) => {
                self.broadcast_no_hold(frame, &Option::None ).await;
            }
            Frame::Proto(ProtoFrame::CentralSearch) => {
                self.send_frame_no_hold(&StarKey::central(), frame ).await;
            }
            Frame::StarMessage(message) => {
                self.send_no_hold(message).await;
            }
            _ => {
                eprintln!("no rule to resend frame of type: {}", frame);
            }
        }
    }

    async fn broadcast(&mut self,  frame: Frame, exclude: &Option<HashSet<StarKey>> )
    {
        let mut stars = vec!();
        for star in self.lanes.keys()
        {
            if exclude.is_none() || !exclude.as_ref().unwrap().contains(star)
            {
                stars.push(star.clone());
            }
        }
        for star in stars
        {
            self.send_frame(&star, frame.clone()).await;
        }
    }

    async fn broadcast_no_hold(&mut self,  frame: Frame, exclude: &Option<HashSet<StarKey>> )
    {
        let mut stars = vec!();
        for star in self.lanes.keys()
        {
            if exclude.is_none() || !exclude.as_ref().unwrap().contains(star)
            {
                stars.push(star.clone());
            }
        }
        for star in stars
        {
            self.send_frame_no_hold(&star, frame.clone()).await;
        }
    }



    async fn send_no_hold(&mut self, message: StarMessage)
    {
        self.send_frame_no_hold(&message.to.clone(), Frame::StarMessage(message) ).await;
    }

    async fn send_frame_no_hold(&mut self, star: &StarKey, frame: Frame )
    {
        let lane = self.lane_with_shortest_path_to_star(star);
        if let Option::Some(lane) = lane
        {
            lane.lane.outgoing.tx.send(LaneCommand::Frame(frame)).await;
        }
        else {
           eprintln!("could not find lane for {}", star);
        }
    }


    async fn send(&mut self, message: StarMessage)
    {
        self.send_frame(&message.to.clone(), Frame::StarMessage(message) ).await;
    }

    async fn send_frame(&mut self, star: &StarKey, frame: Frame )
    {
        let lane = self.lane_with_shortest_path_to_star(star);
        if let Option::Some(lane) = lane
        {

            lane.lane.outgoing.tx.send(LaneCommand::Frame(frame)).await;
        }
        else {
            self.frame_hold.add(star, frame);
        }
    }

    fn lane_with_shortest_path_to_star( &mut self, star: &StarKey ) -> Option<&mut LaneMeta>
    {
        let mut min_hops= usize::MAX;
        let mut rtn = Option::None;

        for (_,lane) in &mut self.lanes
        {
            if let Option::Some(hops) = lane.get_hops_to_star(star)
            {
                if hops < min_hops
                {
                    rtn = Option::Some(lane);
                }
            }
        }

        rtn
    }
    fn shortest_path_star_key(&mut self, to: &StarKey ) -> Option<ShortestPathStarKey>
    {
        let mut rtn = Option::None;

        for (_,lane) in &mut self.lanes
        {
            if let Option::Some(hops) = lane.get_hops_to_star(to)
            {
                if lane.lane.remote_star.is_some()
                {
                    if let Option::None = rtn
                    {
                        rtn = Option::Some(ShortestPathStarKey {
                            to: to.clone(),
                            next_lane: lane.lane.remote_star.as_ref().unwrap().clone(),
                            hops
                        });
                    }
                    else if let Option::Some(min) = &rtn
                    {
                        if hops < min.hops
                        {
                            rtn = Option::Some(ShortestPathStarKey {
                                to: to.clone(),
                                next_lane: lane.lane.remote_star.as_ref().unwrap().clone(),
                                hops
                            });
                        }
                    }
                }
            }
        }

        rtn
    }

    fn get_hops_to_star( &mut self, star: &StarKey ) -> Option<usize>
    {
        let mut rtn= Option::None;

        for (_,lane) in &mut self.lanes
        {
            if let Option::Some(hops) = lane.get_hops_to_star(star)
            {
                if rtn.is_none()
                {
                    rtn = Option::Some(hops);
                }
                else if let Option::Some(min_hops) = rtn
                {
                    if hops < min_hops
                    {
                        rtn = Option::Some(hops);
                    }
                }
            }
        }

        rtn
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

pub enum LaneToCentralState
{
    Found(LaneToCentral),
    None
}

pub struct LaneToCentral
{
    remote_star: StarKey,
    hops: usize
}


