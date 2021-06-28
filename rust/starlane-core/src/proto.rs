use std::cell::{RefCell, Cell};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU64, Ordering};
use std::task::Poll;

use futures::future::{err, join_all, ok, select_all};
use futures::FutureExt;
use futures::prelude::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpSocket, TcpStream};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, mpsc, Mutex, oneshot};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::time::{Duration, Instant};

use crate::cache::ProtoArtifactCachesFactory;
use crate::constellation::Constellation;
use crate::core::{CoreRunner, CoreRunnerCommand};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::{
    Frame, ProtoFrame, SequenceMessage, StarMessage, StarMessagePayload, StarPattern, WindDown,
    WindHit, WindUp,
};
use crate::id::{Id, IdSeq};
use crate::lane::{ConnectorController, LaneEndpoint, LaneCommand, LaneMeta, STARLANE_PROTOCOL_VERSION, TunnelConnector, TunnelIn, TunnelOut, TunnelOutState, ProtoLaneEndpoint, LaneId};
use crate::logger::{Flag, Flags, Log, Logger, ProtoStarLog, ProtoStarLogPayload, StarFlag};
use crate::permissions::AuthTokenSource;
use crate::resource::HostedResourceStore;
use crate::star::{FrameHold, FrameTimeoutInner, Persistence, ResourceRegistryBacking, ResourceRegistryBackingSqLite, ShortestPathStarKey, Star, StarCommand, StarController, StarInfo, StarKernel, StarKey, StarKind, StarSearchTransaction, StarSkel, Transaction, ConstellationBroadcast};
use crate::star::pledge::StarHandleBacking;
use crate::star::variant::{StarVariantCommand, StarVariantFactory};
use crate::starlane::StarlaneCommand;
use crate::template::ConstellationTemplate;
use std::convert::TryInto;

pub static MAX_HOPS: i32 = 32;

pub struct ProtoStar {
    star_key: Option<StarKey>,
    sequence: Arc<AtomicU64>,
    kind: StarKind,
    star_tx: mpsc::Sender<StarCommand>,
    star_rx: mpsc::Receiver<StarCommand>,
    lanes: HashMap<StarKey, LaneMeta>,
    proto_lanes: Vec<ProtoLaneEndpoint>,
    connector_ctrls: Vec<ConnectorController>,
    star_manager_factory: Arc<dyn StarVariantFactory>,
    //  star_core_ext_factory: Arc<dyn StarCoreExtFactory>,
    core_runner: Arc<CoreRunner>,
    logger: Logger,
    frame_hold: FrameHold,
    caches: Arc<ProtoArtifactCachesFactory>,
    data_access: FileAccess,
    proto_constellation_broadcast: Cell<Option<broadcast::Receiver<ConstellationBroadcast>>>,
    flags: Flags,
    tracker: ProtoTracker,
}

impl ProtoStar {
    pub fn new(
        key: Option<StarKey>,
        kind: StarKind,
        star_tx: Sender<StarCommand>,
        star_rx: Receiver<StarCommand>,
        caches: Arc<ProtoArtifactCachesFactory>,
        data_access: FileAccess,
        star_manager_factory: Arc<dyn StarVariantFactory>,
        core_runner: Arc<CoreRunner>,
        proto_constellation_broadcast: broadcast::Receiver<ConstellationBroadcast>,
        flags: Flags,
        logger: Logger,
    ) -> (Self, StarController) {
        //        let (star_tx, star_rx) = mpsc::channel(32);
        (
            ProtoStar {
                star_key: key,
                sequence: Arc::new(AtomicU64::new(0)),
                kind,
                star_tx: star_tx.clone(),
                star_rx,
                lanes: HashMap::new(),
                proto_lanes: vec![],
                connector_ctrls: vec![],
                star_manager_factory: star_manager_factory,
                core_runner: core_runner,
                logger: logger,
                frame_hold: FrameHold::new(),
                caches: caches,
                data_access: data_access,
                proto_constellation_broadcast: Cell::new(Option::Some(proto_constellation_broadcast)),
                tracker: ProtoTracker::new(),
                flags: flags,

            },
            StarController { star_tx },
        )
    }

    pub async fn evolve(mut self) -> Result<Star, Error> {

        let mut proto_constellation_broadcast = self.proto_constellation_broadcast.replace(Option::None).ok_or("expected proto_constellation_broadcast to be Option::Some()")?;

        let star_tx = self.star_tx.clone();
        tokio::spawn( async move {
           while let Result::Ok(broadcast) =  proto_constellation_broadcast.recv().await {
               star_tx.send( StarCommand::ConstellationBroadcast(broadcast)).await;
           }
        });

        loop {
            // request a sequence from central
            let mut futures = vec![];

            let mut lanes = vec![];
            for (key, mut lane) in &mut self.lanes {
                futures.push(lane.lane.incoming.recv().boxed());
                lanes.push(key.clone())
            }
            let mut proto_lane_index = vec![];
//println!("adding proto lane to futures....{}", self.proto_lanes.len() );
            for (index,lane) in &mut self.proto_lanes.iter_mut().enumerate() {
                futures.push(lane.incoming.recv().boxed());
                proto_lane_index.push(index);
            }

            futures.push(self.star_rx.recv().boxed());

            if self.tracker.has_expectation() {
                futures.push(self.tracker.check().boxed())
            }

            let (command, future_index, _) = select_all(futures).await;

            let lane = if future_index < lanes.len() {
                LaneId::Lane(lanes.get(future_index).unwrap().clone())
            } else if future_index < lanes.len()+ proto_lane_index.len() {
                LaneId::ProtoLane(future_index-lanes.len())
            } else {
                LaneId::None
            };

            if let Some(command) = command {
                match command {
                    StarCommand::GetStarKey(tx) => {
                        tx.send( self.star_key.clone() );
                    }
                    StarCommand::ConstellationBroadcast(ConstellationBroadcast::ConstellationReady)=> {
                        let info = StarInfo {
                            key: self.star_key.as_ref().unwrap().clone(),
                            kind: self.kind.clone(),
                        };

                        let (core_tx, core_rx) = mpsc::channel(16);

                        let data_access = self.data_access.with_path(format!("stars/{}", info.key.to_string()))?;

                        let resource_registry: Option<Arc<dyn ResourceRegistryBacking>> =
                            if info.kind.is_resource_manager() {
                                Option::Some(Arc::new(
                                    ResourceRegistryBackingSqLite::new(info.clone(), data_access.path() ).await?,
                                ))
                            } else {
                                Option::None
                            };

                        let star_handler: Option<StarHandleBacking> =
                            if !info.kind.handles().is_empty() {
                                Option::Some(StarHandleBacking::new(self.star_tx.clone()).await)
                            } else {
                                Option::None
                            };


                        let skel = StarSkel {
                            info: info,
                            sequence: self.sequence.clone(),
                            star_tx: self.star_tx.clone(),
                            core_tx: core_tx.clone(),
                            logger: self.logger.clone(),
                            flags: self.flags.clone(),
                            auth_token_source: AuthTokenSource {},
                            registry: resource_registry,
                            star_handler: star_handler,
                            persistence: Persistence::Memory,
                            data_access: data_access,
                            caches: self.caches.clone(),
                        };

                        let variant = self.star_manager_factory.create(skel.clone()).await;

                        self.core_runner
                            .send(CoreRunnerCommand::Core {
                                skel: skel.clone(),
                                rx: core_rx,
                            })
                            .await;

                        return Ok(Star::from_proto(
                            skel.clone(),
                            self.star_rx,
                            core_tx,
                            self.lanes,
                            self.connector_ctrls,
                            self.frame_hold,
                            variant,
                        )
                        .await);
                    }
                    StarCommand::AddLaneEndpoint(lane) => {
                          let remote_star = lane.remote_star.clone();
                            self.lanes.insert(lane.remote_star.clone(), LaneMeta::new(lane));

                            if let Option::Some(frames) = self.frame_hold.release(&remote_star) {
                                for frame in frames {
                                    self.send_frame(&remote_star, frame).await;
                                }
                            }
                    }
                    StarCommand::AddProtoLaneEndpoint(lane) => {
                        if( self.star_key.is_some() )
                        {
                            lane.outgoing.out_tx.send(LaneCommand::Frame(Frame::Proto(ProtoFrame::ReportStarKey(self.star_key.clone().unwrap())))).await?;
                        }
                        self.proto_lanes.push(lane);
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::AddLogger(logger) => {
                        //                        self.logger =
                    }
                    StarCommand::Frame(frame) => {

                        self.tracker.process(&frame);
                        match frame {
                            Frame::Proto(proto_frame) => {
                                match proto_frame
                                {
                                    ProtoFrame::ReportStarKey(remote_star) => {
                                        if let LaneId::ProtoLane(index) = lane {
                                            let mut lane = self.proto_lanes.remove(index);
                                            lane.remote_star = Option::Some(remote_star);
                                            let lane: LaneEndpoint = lane.try_into()?;
                                            self.star_tx.send(StarCommand::AddLaneEndpoint(lane)).await;
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            _ => {
                                println!("{} frame unsupported by ProtoStar: {}", self.kind, frame);
                            }
                        }
                    }

                    StarCommand::FrameTimeout(timeout) => {
                        eprintln!(
                            "frame timeout: {}.  resending {} retry.",
                            timeout.frame, timeout.retries
                        );
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

    async fn send_expansion_request(&mut self) {
        unimplemented!();
        /*
        let frame = Frame::Proto(ProtoFrame::GatewaySelect);
        self.tracker.track(frame.clone(), |frame| {
            if let Frame::Proto(ProtoFrame::GatewayAssign(_)) = frame {
                return true;
            } else {
                return false;
            }
        });

        self.broadcast(frame, &Option::None).await;

         */
    }

    async fn resend(&mut self, frame: Frame) {
        match frame {
            Frame::Proto(ProtoFrame::GatewaySelect) => {
                self.broadcast_no_hold(frame, &Option::None).await;
            }
            Frame::StarMessage(message) => {
                self.send_no_hold(message).await;
            }
            _ => {
                eprintln!("no rule to resend frame of type: {}", frame);
            }
        }
    }

    async fn broadcast(&mut self, frame: Frame, exclude: &Option<HashSet<StarKey>>) {
        let mut stars = vec![];
        for star in self.lanes.keys() {
            if exclude.is_none() || !exclude.as_ref().unwrap().contains(star) {
                stars.push(star.clone());
            }
        }
        for star in stars {
            self.send_frame(&star, frame.clone()).await;
        }
    }

    async fn broadcast_no_hold(&mut self, frame: Frame, exclude: &Option<HashSet<StarKey>>) {
        let mut stars = vec![];
        for star in self.lanes.keys() {
            if exclude.is_none() || !exclude.as_ref().unwrap().contains(star) {
                stars.push(star.clone());
            }
        }
        for star in stars {
            self.send_frame_no_hold(&star, frame.clone()).await;
        }
    }

    async fn send_no_hold(&mut self, message: StarMessage) {
        self.send_frame_no_hold(&message.to.clone(), Frame::StarMessage(message))
            .await;
    }

    async fn send_frame_no_hold(&mut self, star: &StarKey, frame: Frame) {
        let lane = self.lane_with_shortest_path_to_star(star);
        if let Option::Some(lane) = lane {
            lane.lane.outgoing.out_tx.send(LaneCommand::Frame(frame)).await;
        } else {
            eprintln!("could not find lane for {}", star.to_string());
        }
    }

    async fn send(&mut self, message: StarMessage) {
        self.send_frame(&message.to.clone(), Frame::StarMessage(message))
            .await;
    }

    async fn send_frame(&mut self, star: &StarKey, frame: Frame) {
        let lane = self.lane_with_shortest_path_to_star(star);
        if let Option::Some(lane) = lane {
            lane.lane.outgoing.out_tx.send(LaneCommand::Frame(frame)).await;
        } else {
            self.frame_hold.add(star, frame);
        }
    }

    fn lane_with_shortest_path_to_star(&mut self, star: &StarKey) -> Option<&mut LaneMeta> {
        let mut min_hops = usize::MAX;
        let mut rtn = Option::None;

        for (_, lane) in &mut self.lanes {
            if let Option::Some(hops) = lane.get_hops_to_star(star) {
                if hops < min_hops {
                    rtn = Option::Some(lane);
                }
            }
        }

        rtn
    }
    fn shortest_path_star_key(&mut self, to: &StarKey) -> Option<ShortestPathStarKey> {
        let mut rtn = Option::None;

        for (_, lane) in &mut self.lanes {
            if let Option::Some(hops) = lane.get_hops_to_star(to) {
//                if lane.lane.remote_star.is_some() {
                    if let Option::None = rtn {
                        rtn = Option::Some(ShortestPathStarKey {
                            to: to.clone(),
                            next_lane: lane.lane.remote_star.clone(),
                            hops,
                        });
                    } else if let Option::Some(min) = &rtn {
                        if hops < min.hops {
                            rtn = Option::Some(ShortestPathStarKey {
                                to: to.clone(),
                                next_lane: lane.lane.remote_star.clone(),
                                hops,
                            });
                        }
                    }
                //}
            }
        }

        rtn
    }

    fn get_hops_to_star(&mut self, star: &StarKey) -> Option<usize> {
        let mut rtn = Option::None;

        for (_, lane) in &mut self.lanes {
            if let Option::Some(hops) = lane.get_hops_to_star(star) {
                if rtn.is_none() {
                    rtn = Option::Some(hops);
                } else if let Option::Some(min_hops) = rtn {
                    if hops < min_hops {
                        rtn = Option::Some(hops);
                    }
                }
            }
        }

        rtn
    }

    async fn process_frame(&mut self, frame: Frame, lane: &mut LaneMeta) {
        match frame {
            _ => {
                eprintln!("star does not handle frame: {}", frame)
            }
        }
    }
}

pub struct ProtoStarEvolution {
    pub star: StarKey,
    pub controller: StarController,
}

pub struct ProtoStarController {
    command_tx: Sender<StarCommand>,
}

#[derive(Clone)]
pub enum ProtoStarKernel {
    Central,
    Mesh,
    Supervisor,
    Server,
    Gateway,
}

impl ProtoStarKernel {
    fn evolve(&self) -> Result<Box<dyn StarKernel>, Error> {
        Ok(Box::new(PlaceholderKernel::new()))
    }
}

pub struct PlaceholderKernel {}

impl PlaceholderKernel {
    pub fn new() -> Self {
        PlaceholderKernel {}
    }
}

impl StarKernel for PlaceholderKernel {}

pub struct ProtoTunnel {
    pub star: Option<StarKey>,
    pub tx: Sender<Frame>,
    pub rx: Receiver<Frame>,
}

impl ProtoTunnel {
    pub async fn evolve(mut self) -> Result<(TunnelOut, TunnelIn), Error> {
        self.tx
            .send(Frame::Proto(ProtoFrame::StarLaneProtocolVersion(
                STARLANE_PROTOCOL_VERSION,
            )))
            .await;

        if let Option::Some(star) = self.star {
            self.tx
                .send(Frame::Proto(ProtoFrame::ReportStarKey(star)))
                .await;
        }

        // first we confirm that the version is as expected
        if let Option::Some(Frame::Proto(recv)) = self.rx.recv().await {
            match recv {
                ProtoFrame::StarLaneProtocolVersion(version)
                    if version == STARLANE_PROTOCOL_VERSION =>
                {
                    // do nothing... we move onto the next step
                    return Ok((
                        TunnelOut {
//                            remote_star: remote_star_key.clone(),
                            tx: self.tx,
                        },
                        TunnelIn {
//                            remote_star: remote_star_key.clone(),
                            rx: self.rx,
                        },
                    ));

                }
                ProtoFrame::StarLaneProtocolVersion(version) => {
                    return Err(format!("wrong version: {}", version).into());
                }
                gram => {
                    return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)", gram).into());
                }
            }
        } else {
            return Err("disconnected".into());
        }

        if let Option::Some(Frame::Proto(recv)) = self.rx.recv().await {
            match recv {
                ProtoFrame::ReportStarKey(remote_star_key) => {
                    return Ok((
                        TunnelOut {
//                            remote_star: remote_star_key.clone(),
                            tx: self.tx,
                        },
                        TunnelIn {
//                            remote_star: remote_star_key.clone(),
                            rx: self.rx,
                        },
                    ));
                }
                frame => {
                    return Err(format!(
                        "unexpected star gram: {} (expected to receive ReportStarKey next)",
                        frame
                    )
                    .into());
                }
            };
        } else {
            return Err("disconnected!".into());
        }
    }
}

pub fn local_tunnels(high: Option<StarKey>, low: Option<StarKey>) -> (ProtoTunnel, ProtoTunnel) {
    let (atx, arx) = mpsc::channel::<Frame>(32);
    let (btx, brx) = mpsc::channel::<Frame>(32);

    (
        ProtoTunnel {
            star: high,
            tx: atx,
            rx: brx,
        },
        ProtoTunnel {
            star: low,
            tx: btx,
            rx: arx,
        },
    )
}

struct ProtoTrackerCase {
    frame: Frame,
    instant: Instant,
    expect: fn(&Frame) -> bool,
    retries: usize,
}

impl ProtoTrackerCase {
    pub fn reset(&mut self) {
        self.instant = Instant::now();
    }
}

struct ProtoTracker {
    case: Option<ProtoTrackerCase>,
}

impl ProtoTracker {
    pub fn new() -> Self {
        ProtoTracker { case: Option::None }
    }

    pub fn track(&mut self, frame: Frame, expect: fn(&Frame) -> bool) {
        self.case = Option::Some(ProtoTrackerCase {
            frame: frame,
            instant: Instant::now(),
            expect: expect,
            retries: 0,
        });
    }

    pub fn process(&mut self, frame: &Frame) {
        if let Option::Some(case) = &self.case {
            if (case.expect)(frame) {
                self.case = Option::None;
            }
        }
    }

    pub fn has_expectation(&self) -> bool {
        return self.case.is_some();
    }

    pub async fn check(&mut self) -> Option<StarCommand> {
        if let Option::Some(case) = &mut self.case {
            let now = Instant::now();
            let seconds = 5 - (now.duration_since(case.instant).as_secs() as i64);
            if seconds > 0 {
                let duration = Duration::from_secs(seconds as u64);
                tokio::time::sleep(duration).await;
            }

            case.retries = case.retries + 1;

            case.reset();

            Option::Some(StarCommand::FrameTimeout(FrameTimeoutInner {
                frame: case.frame.clone(),
                retries: case.retries,
            }))
        } else {
            Option::None
        }
    }
}

pub enum LaneToCentralState {
    Found(LaneToCentral),
    None,
}

pub struct LaneToCentral {
    remote_star: StarKey,
    hops: usize,
}
