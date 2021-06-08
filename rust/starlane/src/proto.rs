use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU64, Ordering};
use std::task::Poll;

use futures::future::{err, join_all, ok, select_all};
use futures::FutureExt;
use futures::prelude::*;
use tokio::sync::{broadcast, mpsc, Mutex, oneshot};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::time::{Duration, Instant};

use crate::constellation::Constellation;
use crate::core::{CoreRunner, CoreRunnerCommand, StarCore, StarCoreCommand, StarCoreExtFactory, StarCoreExtKind, StarCoreFactory};
use crate::error::Error;
use crate::file::FileAccess;
use crate::frame::{Frame, ProtoFrame, SequenceMessage, StarMessage, StarMessagePayload, StarPattern, WindDown, WindHit, WindUp} ;
use crate::id::{Id, IdSeq};
use crate::lane::{ConnectorController, Lane, LaneCommand, LaneMeta, STARLANE_PROTOCOL_VERSION, TunnelConnector, TunnelReceiver, TunnelSender, TunnelSenderState};
use crate::logger::{Flag, Flags, Log, Logger, ProtoStarLog, ProtoStarLogPayload, StarFlag};
use crate::permissions::AuthTokenSource;
use crate::resource::HostedResourceStore;
use crate::star::{FrameHold, FrameTimeoutInner, Persistence, ResourceRegistryBacking, ResourceRegistryBackingSqLite, ShortestPathStarKey, Star, StarCommand, StarController, StarInfo, StarKernel, StarKey, StarKind, StarManagerFactory, StarSearchTransaction, StarSkel, StarVariantCommand, Transaction};
use crate::star::pledge::StarHandleBacking;
use crate::starlane::StarlaneCommand;
use crate::template::ConstellationTemplate;

pub static MAX_HOPS: i32 = 32;

pub struct ProtoStar
{
  star_key: Option<StarKey>,
  sequence: Arc<AtomicU64>,
  kind: StarKind,
  command_tx: mpsc::Sender<StarCommand>,
  command_rx: mpsc::Receiver<StarCommand>,
  lanes: HashMap<StarKey, LaneMeta>,
  connector_ctrls: Vec<ConnectorController>,
  star_manager_factory: Arc<dyn StarManagerFactory>,
  star_core_ext_factory: Arc<dyn StarCoreExtFactory>,
  core_runner: Arc<CoreRunner>,
  logger: Logger,
  frame_hold: FrameHold,
  flags: Flags,
  tracker: ProtoTracker
}

impl ProtoStar
{
    pub fn new(key: Option<StarKey>, kind: StarKind, star_manager_factory: Arc<dyn StarManagerFactory>, core_runner: Arc<CoreRunner>, star_core_ext_factory: Arc<dyn StarCoreExtFactory>, flags: Flags, logger: Logger ) ->(Self, StarController)
    {
        let (command_tx, command_rx) = mpsc::channel(32);
        (ProtoStar{
            star_key: key,
            sequence: Arc::new(AtomicU64::new(0)),
            kind,
            command_tx: command_tx.clone(),
            command_rx: command_rx,
            lanes: HashMap::new(),
            connector_ctrls: vec![],
            star_manager_factory: star_manager_factory,
            star_core_ext_factory: star_core_ext_factory,
            core_runner: core_runner,
            logger: logger,
            frame_hold: FrameHold::new(),
            tracker: ProtoTracker::new(),
            flags: flags
        }, StarController{
            star_tx: command_tx
        })
    }

    pub async fn evolve(mut self) -> Result<Star,Error>
    {
        if self.kind.is_central()
        {
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
                    StarCommand::ConstellationConstructionComplete => {
                        let info = StarInfo{
                            key: self.star_key.as_ref().unwrap().clone(),
                            kind: self.kind.clone()};
                        let manager_tx= self.star_manager_factory.create().await;


                        let (core_tx,core_rx) = mpsc::channel(16);

                        let resource_registry: Option<Arc<dyn ResourceRegistryBacking>>= if info.kind.is_resource_manager() {
                            Option::Some( Arc::new( ResourceRegistryBackingSqLite::new().await? ) )
                        } else {
                            Option::None
                        };

                        let star_handler: Option<StarHandleBacking>= if !info.kind.handles().is_empty() {
                            Option::Some(  StarHandleBacking::new().await )
                        } else {
                            Option::None
                        };

                        let skel = StarSkel {
                            info: info,
                            sequence: self.sequence.clone(),
                            star_tx: self.command_tx.clone(),
                            core_tx: core_tx.clone(),
                            variant_tx: manager_tx.clone(),
                            logger: self.logger.clone(),
                            flags: self.flags.clone(),
                            auth_token_source: AuthTokenSource {},
                            registry: resource_registry,
                            star_handler: star_handler,
                            persistence: Persistence::Memory,
                            file_access: FileAccess::new("data".to_string()).await?
                        };

                        let core_ext = self.star_core_ext_factory.create(&skel );
                        self.core_runner.send(CoreRunnerCommand::Core{
                            skel: skel.clone(),
                            ext: StarCoreExtKind::None,
                            rx: core_rx
                        } ).await;

                        // now send star data to manager and core... tricky!
                        manager_tx.send(StarVariantCommand::StarSkel(skel.clone()) ).await;

                        return Ok(Star::from_proto(skel.clone(),
                                                   self.command_rx,
                                                   core_tx,
                                                   self.lanes,
                                                   self.connector_ctrls,
                                                   self.frame_hold ).await );

                    }
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

                            Frame::Proto(ProtoFrame::GrantSubgraphExpansion(subgraph)) => {
                                let key = StarKey::new_with_subgraph(subgraph.clone(), 0);
                                self.star_key = Option::Some(key.clone());
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

    async fn resend(&mut self,  frame: Frame)
    {
        match frame
        {
            Frame::Proto(ProtoFrame::RequestSubgraphExpansion) => {
                self.broadcast_no_hold(frame, &Option::None ).await;
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
           eprintln!("could not find lane for {}", star.to_string());
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


