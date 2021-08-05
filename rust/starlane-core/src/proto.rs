use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use futures::future::select_all;
use futures::prelude::*;
use futures::FutureExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, Instant};

use crate::cache::ProtoArtifactCachesFactory;
use crate::constellation::ConstellationStatus;
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::{
    Frame, ProtoFrame, SequenceMessage, StarMessage, StarMessagePayload, StarPattern, WindDown,
    WindHit, WindUp,
};
use crate::lane::{
    ConnectorController, LaneCommand, LaneEndpoint, LaneIndex, LaneMeta, LaneWrapper,
    ProtoLaneEndpoint, TunnelConnector, TunnelIn, TunnelOut, TunnelOutState,
    STARLANE_PROTOCOL_VERSION,
};
use crate::logger::{Flag, Flags, Log, Logger, ProtoStarLog, ProtoStarLogPayload, StarFlag};
use crate::permissions::AuthTokenSource;
use crate::star::core::message::MessagingEndpointComponent;
use crate::star::shell::lanes::{LanesApi, LanesComponent};
use crate::star::shell::search::{StarSearchApi, StarSearchComponent, StarSearchTransaction, ShortestPathStarKey};
use crate::star::shell::message::{MessagingApi, MessagingComponent};
use crate::star::shell::pledge::StarHandleBacking;
use crate::star::shell::router::{RouterApi, RouterComponent};
use crate::star::surface::{SurfaceApi, SurfaceCall, SurfaceComponent};
use crate::star::variant::StarVariantFactory;
use crate::star::{
    ConstellationBroadcast, FrameHold, FrameTimeoutInner, Persistence, ResourceRegistryBacking,
    ResourceRegistryBackingSqLite, Star, StarCommand, StarController,
    StarInfo, StarKernel, StarKey, StarKind, StarSkel,
};
use crate::starlane::StarlaneMachine;
use crate::template::StarKeyConstellationIndex;
use crate::star::shell::locator::{ResourceLocatorApi, ResourceLocatorComponent};
use crate::star::shell::golden::{GoldenPathApi, GoldenPathComponent};


pub struct ProtoStar {
    star_key: ProtoStarKey,
    sequence: Arc<AtomicU64>,
    kind: StarKind,
    star_tx: mpsc::Sender<StarCommand>,
    star_rx: mpsc::Receiver<StarCommand>,
    surface_api: SurfaceApi,
    surface_rx: mpsc::Receiver<SurfaceCall>,
    lanes: HashMap<StarKey, LaneWrapper>,
    proto_lanes: Vec<LaneWrapper>,
    connector_ctrls: Vec<ConnectorController>,
    star_manager_factory: Arc<dyn StarVariantFactory>,
    //  star_core_ext_factory: Arc<dyn StarCoreExtFactory>,
    logger: Logger,
    frame_hold: FrameHold,
    data_access: FileAccess,
    proto_constellation_broadcast: Cell<Option<broadcast::Receiver<ConstellationBroadcast>>>,
    constellation_status: ConstellationStatus,
    flags: Flags,
    tracker: ProtoTracker,
    machine: StarlaneMachine,
}

impl ProtoStar {
    pub fn new(
        key: ProtoStarKey,
        kind: StarKind,
        star_tx: Sender<StarCommand>,
        star_rx: Receiver<StarCommand>,
        surface_api: SurfaceApi,
        surface_rx: mpsc::Receiver<SurfaceCall>,
        data_access: FileAccess,
        star_manager_factory: Arc<dyn StarVariantFactory>,
        proto_constellation_broadcast: broadcast::Receiver<ConstellationBroadcast>,
        flags: Flags,
        logger: Logger,
        machine: StarlaneMachine,
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
                logger: logger,
                frame_hold: FrameHold::new(),
                data_access: data_access,
                proto_constellation_broadcast: Cell::new(Option::Some(
                    proto_constellation_broadcast,
                )),
                tracker: ProtoTracker::new(),
                flags: flags,
                constellation_status: ConstellationStatus::Unknown,
                machine: machine,
                surface_api: surface_api.clone(),
                surface_rx,
            },
            StarController {
                star_tx,
                surface_api,
            },
        )
    }

    pub async fn evolve(mut self) -> Result<Star, Error> {
        let mut proto_constellation_broadcast = self
            .proto_constellation_broadcast
            .replace(Option::None)
            .ok_or("expected proto_constellation_broadcast to be Option::Some()")?;

        let star_tx = self.star_tx.clone();
        tokio::spawn(async move {
            while let Result::Ok(broadcast) = proto_constellation_broadcast.recv().await {
                star_tx
                    .send(StarCommand::ConstellationBroadcast(broadcast))
                    .await;
            }
        });

        loop {
            // request a sequence from central
            let mut futures = vec![];

            let mut lanes = vec![];
            for (key, lane) in &mut self.lanes {
                futures.push(lane.incoming().recv().boxed());
                lanes.push(key.clone())
            }
            let mut proto_lane_index = vec![];
            //println!("adding proto lane to futures....{}", self.proto_lanes.len() );
            for (index, lane) in &mut self.proto_lanes.iter_mut().enumerate() {
                futures.push(lane.incoming().recv().boxed());
                proto_lane_index.push(index);
            }

            futures.push(self.star_rx.recv().boxed());

            if self.tracker.has_expectation() {
                futures.push(self.tracker.check().boxed())
            }

            let (command, future_index, _) = select_all(futures).await;

            let lane_index = if future_index < lanes.len() {
                LaneIndex::Lane(
                    lanes
                        .get(future_index)
                        .expect("expected a lane at this index")
                        .clone(),
                )
            } else if future_index < lanes.len() + proto_lane_index.len() {
                LaneIndex::ProtoLane(future_index - lanes.len())
            } else {
                LaneIndex::None
            };

            let _lane = if future_index < lanes.len() {
                Option::Some(
                    self.lanes
                        .get_mut(lanes.get(future_index).as_ref().unwrap())
                        .expect("expected to get lane"),
                )
            } else if future_index < lanes.len() + proto_lane_index.len() {
                Option::Some(
                    self.proto_lanes
                        .get_mut(future_index - lanes.len())
                        .unwrap(),
                )
            } else {
                Option::None
            };

            if let Some(command) = command {
                match command {
                    StarCommand::GetStarInfo(tx) => match &self.star_key {
                        ProtoStarKey::Key(key) => {
                            tx.send(Option::Some(StarInfo {
                                key: key.clone(),
                                kind: self.kind.clone(),
                            }));
                        }
                        ProtoStarKey::RequestSubKeyExpansion(_) => {
                            tx.send(Option::None);
                        }
                    },
                    StarCommand::ConstellationBroadcast(broadcast) => match broadcast {
                        ConstellationBroadcast::Status(constellation_status) => {
                            self.constellation_status = constellation_status;
                            self.check_ready();
                        }
                    },
                    StarCommand::InvokeProtoStarEvolution => {
                        let star_key = match self.star_key
                        {
                            ProtoStarKey::Key(star_key) => star_key,
                            _ => panic!("proto star not ready for proto star evolution because it does not have a star_key yet assigned")
                        };

                        let info = StarInfo {
                            key: star_key,
                            kind: self.kind.clone(),
                        };

                        let (core_messaging_endpoint_tx, core_messaging_endpoint_rx) =
                            mpsc::channel(1024);
                        let (resource_locator_tx, resource_locator_rx) = mpsc::channel(1024);
                        let (star_locator_tx, star_locator_rx) = mpsc::channel(1024);
                        let (router_tx, router_rx) = mpsc::channel(1024);
                        let (messaging_tx, messaging_rx) = mpsc::channel(1024);
                        let (lanes_tx, lanes_rx) = mpsc::channel(1024);
                        let (golden_path_tx, golden_path_rx) = mpsc::channel(1024);

                        let resource_locator_api = ResourceLocatorApi::new(resource_locator_tx);
                        let star_search_api = StarSearchApi::new(star_locator_tx);
                        let router_api = RouterApi::new(router_tx);
                        let messaging_api = MessagingApi::new(messaging_tx);
                        let lanes_api = LanesApi::new(lanes_tx);
                        let golden_path_api = GoldenPathApi::new(golden_path_tx);

                        let data_access = self
                            .data_access
                            .with_path(format!("stars/{}", info.key.to_string()))?;

                        let resource_registry: Option<Arc<dyn ResourceRegistryBacking>> =
                            if info.kind.is_resource_manager() {
                                Option::Some(Arc::new(
                                    ResourceRegistryBackingSqLite::new(
                                        info.clone(),
                                        data_access.path(),
                                    )
                                    .await?,
                                ))
                            } else {
                                Option::None
                            };

                        let star_handler: Option<StarHandleBacking> =
                            if !info.kind.distributes_to().is_empty() {
                                Option::Some(StarHandleBacking::new(self.star_tx.clone()).await)
                            } else {
                                Option::None
                            };

                        let skel = StarSkel {
                            info: info,
                            sequence: self.sequence.clone(),
                            star_tx: self.star_tx.clone(),
                            core_messaging_endpoint_tx: core_messaging_endpoint_tx.clone(),
                            logger: self.logger.clone(),
                            flags: self.flags.clone(),
                            registry: resource_registry,
                            star_handler: star_handler,
                            persistence: Persistence::Memory,
                            data_access: data_access,
                            machine: self.machine.clone(),
                            surface_api: self.surface_api,
                            resource_locator_api,
                            star_search_api,
                            router_api,
                            messaging_api,
                            lanes_api,
                            golden_path_api
                        };

                        let variant = self.star_manager_factory.create(skel.clone()).await;

                        MessagingEndpointComponent::start(skel.clone(), core_messaging_endpoint_rx);
                        ResourceLocatorComponent::start(skel.clone(), resource_locator_rx);
                        StarSearchComponent::start(skel.clone(), star_locator_rx);
                        RouterComponent::start(skel.clone(), router_rx);
                        MessagingComponent::start(skel.clone(), messaging_rx);
                        LanesComponent::start(skel.clone(), lanes_rx);
                        SurfaceComponent::start(skel.clone(), self.surface_rx);
                        GoldenPathComponent::start(skel.clone(), golden_path_rx);

                        return Ok(Star::from_proto(
                            skel,
                            self.star_rx,
                            core_messaging_endpoint_tx,
                            self.lanes,
                            self.proto_lanes,
                            self.connector_ctrls,
                            self.frame_hold,
                            variant,
                        )
                        .await);
                    }
                    StarCommand::AddLaneEndpoint(lane) => {
                        let remote_star = lane.remote_star.clone();
                        self.lanes.insert(
                            lane.remote_star.clone(),
                            LaneWrapper::Lane(LaneMeta::new(lane)),
                        );

                        if let Option::Some(frames) = self.frame_hold.release(&remote_star) {
                            for frame in frames {
                                self.send_frame(&remote_star, frame).await;
                            }
                        }
                    }
                    StarCommand::AddProtoLaneEndpoint(lane) => {
                        match &self.star_key {
                            ProtoStarKey::Key(star_key) => {
                                lane.outgoing
                                    .out_tx
                                    .send(LaneCommand::Frame(Frame::Proto(
                                        ProtoFrame::ReportStarKey(star_key.clone()),
                                    )))
                                    .await?;
                            }
                            ProtoStarKey::RequestSubKeyExpansion(_index) => {
                                lane.outgoing
                                    .out_tx
                                    .send(LaneCommand::Frame(Frame::Proto(
                                        ProtoFrame::GatewaySelect,
                                    )))
                                    .await?;
                            }
                        }
                        self.proto_lanes
                            .push(LaneWrapper::Proto(LaneMeta::new(lane)));
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::AddLogger(_logger) => {
                        //                        self.logger =
                    }
                    StarCommand::Frame(frame) => {
                        self.tracker.process(&frame);
                        match frame {
                            Frame::Proto(proto_frame) => match proto_frame {
                                ProtoFrame::ReportStarKey(remote_star) => {
                                    if let LaneIndex::ProtoLane(index) = lane_index {
                                        let mut lane = self
                                            .proto_lanes
                                            .remove(index)
                                            .expect_proto_lane()
                                            .unwrap();
                                        lane.remote_star = Option::Some(remote_star);
                                        let lane: LaneEndpoint = lane.try_into()?;
                                        self.star_tx.send(StarCommand::AddLaneEndpoint(lane)).await;
                                    }
                                }
                                ProtoFrame::GatewayAssign(subgraph) => match self.star_key {
                                    ProtoStarKey::Key(_) => {
                                        warn!("should not receive a subgraph for starkey as starkey is already assigned");
                                    }
                                    ProtoStarKey::RequestSubKeyExpansion(index) => {
                                        let star_key = StarKey::new_with_subgraph(subgraph, index);
                                        self.star_key = ProtoStarKey::Key(star_key.clone());
                                        self.broadcast(
                                            Frame::Proto(ProtoFrame::ReportStarKey(star_key)),
                                            &Option::None,
                                        )
                                        .await;
                                        self.check_ready();
                                    }
                                },

                                _ => {}
                            },
                            _ => {
                                println!(
                                    "{} frame unsupported by ProtoStar: {}",
                                    self.kind.to_string(),
                                    frame
                                );
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

    fn check_ready(&mut self) {
        if self.constellation_status == ConstellationStatus::Assembled && self.star_key.is_some() {
            let star_tx = self.star_tx.clone();
            tokio::spawn(async move {
                star_tx.send(StarCommand::InvokeProtoStarEvolution).await;
            });
        }
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

        for proto_lane in &mut self.proto_lanes {
            proto_lane
                .outgoing()
                .out_tx
                .send(LaneCommand::Frame(frame.clone()))
                .await;
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

        for proto_lane in &mut self.proto_lanes {
            proto_lane
                .outgoing()
                .out_tx
                .send(LaneCommand::Frame(frame.clone()))
                .await;
        }
    }

    async fn send_no_hold(&mut self, message: StarMessage) {
        self.send_frame_no_hold(&message.to.clone(), Frame::StarMessage(message))
            .await;
    }

    async fn send_frame_no_hold(&mut self, star: &StarKey, frame: Frame) {
        let lane = self.lane_with_shortest_path_to_star(star);
        if let Option::Some(lane) = lane {
            lane.outgoing().out_tx.send(LaneCommand::Frame(frame)).await;
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
            lane.outgoing().out_tx.send(LaneCommand::Frame(frame)).await;
        } else {
            self.frame_hold.add(star, frame);
        }
    }

    fn lane_with_shortest_path_to_star(&mut self, star: &StarKey) -> Option<&mut LaneWrapper> {
        let min_hops = usize::MAX;
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
                        next_lane: lane.get_remote_star().as_ref().unwrap().clone(),
                        hops,
                    });
                } else if let Option::Some(min) = &rtn {
                    if hops < min.hops {
                        rtn = Option::Some(ShortestPathStarKey {
                            to: to.clone(),
                            next_lane: lane.get_remote_star().as_ref().unwrap().clone(),
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

    async fn process_frame(&mut self, frame: Frame, _lane: &mut LaneWrapper) {
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
                ProtoFrame::ReportStarKey(_remote_star_key) => {
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

pub fn local_tunnels() -> (ProtoTunnel, ProtoTunnel) {
    let (atx, arx) = mpsc::channel::<Frame>(32);
    let (btx, brx) = mpsc::channel::<Frame>(32);

    (
        ProtoTunnel { tx: atx, rx: brx },
        ProtoTunnel { tx: btx, rx: arx },
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

#[derive(Clone)]
pub enum ProtoStarKey {
    Key(StarKey),
    RequestSubKeyExpansion(StarKeyConstellationIndex),
}

impl ProtoStarKey {
    pub fn is_some(&self) -> bool {
        match self {
            ProtoStarKey::Key(_) => true,
            ProtoStarKey::RequestSubKeyExpansion(_) => false,
        }
    }
}
