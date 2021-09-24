use std::{cmp, fmt};
use std::cmp::{min, Ordering};
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::fmt::{Debug, Formatter};
use std::iter::FromIterator;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{Duration, Instant};

use futures::future::select_all;
use futures::FutureExt;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use shell::pledge::{StarConscript, StarConscriptionSatisfaction, StarWranglerBacking};
use shell::search::{SearchCommit, SearchHits, SearchInit, StarSearchTransaction, TransactionResult};
use starlane_resources::{Resource, ResourceIdentifier, ResourceSelector };
use starlane_resources::message::{Fail, MessageId};

use crate::cache::ProtoArtifactCachesFactory;
use crate::constellation::ConstellationStatus;
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::{
    ActorLookup, Frame, ProtoFrame, ResourceRegistryRequest, Reply, SearchHit, SearchResults,
    SearchTraversal, SearchWindDown, SearchWindUp, SimpleReply, StarMessage, StarMessagePayload, StarPattern,
    TraversalAction, WatchInfo,
};
use crate::id::Id;
use crate::lane::{
    ConnectorController, LaneCommand, LaneEnd, LaneIndex, LaneMeta, LaneWrapper, ProtoLaneEnd,
    UltimaLaneKey,
};
use crate::logger::{Flags, Logger, LogInfo};
use crate::message::{
    MessageReplyTracker, MessageResult, MessageUpdate, ProtoStarMessage,
    ProtoStarMessageTo, TrackerJob,
};
use crate::resource::{ActorKey, Registry, RegistryReservation, RegistryUniqueSrc, ResourceAddress, ResourceKey, ResourceNamesReservationRequest, ResourceRecord, ResourceRegistration, ResourceRegistryAction, ResourceRegistryCommand, ResourceRegistryResult, ResourceType, UniqueSrc};
use crate::star::core::message::CoreMessageCall;
use crate::star::shell::golden::GoldenPathApi;
use crate::star::shell::lanes::LaneMuxerApi;
use crate::star::shell::locator::ResourceLocatorApi;
use crate::star::shell::message::MessagingApi;
use crate::star::shell::router::RouterApi;
use crate::star::shell::search::{StarSearchApi, StarSearchCall};
use crate::star::shell::watch::WatchApi;
use crate::star::surface::SurfaceApi;
use crate::star::variant::{FrameVerdict, VariantApi};
use crate::starlane::StarlaneMachine;
use starlane_resources::status::Status;
use crate::template::StarTemplateHandle;
use crate::watch::{Change, Notification, Property, Topic, WatchSelector};
use starlane_resources::property::{ResourcePropertyAssignment, ResourceRegistryPropertyAssignment};

pub mod core;
pub mod shell;
pub mod surface;
pub mod variant;

#[derive(
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Hash,
    strum_macros::EnumString,
    strum_macros::Display,
)]
pub enum StarKind {
    Central,
    Space,
    Mesh,
    App,
    Mechtron,
    FileStore,
    ArtifactStore,
    Gateway,
    Link,
    Client,
    Web,
    K8s,
}

impl StarKind {
    pub fn is_resource_manager(&self) -> bool {
        match self {
            StarKind::Central => true,
            StarKind::Space => true,
            StarKind::Mesh => false,
            StarKind::App => true,
            StarKind::Mechtron => false,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => false,
            StarKind::Web => false,
            StarKind::ArtifactStore => true,
            StarKind::K8s => true,
        }
    }

    pub fn is_resource_host(&self) -> bool {
        match self {
            StarKind::Central => false,
            StarKind::Space => true,
            StarKind::Mesh => false,
            StarKind::App => true,
            StarKind::Mechtron => true,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => true,
            StarKind::Web => true,
            StarKind::ArtifactStore => true,
            StarKind::K8s => true,
        }
    }

    pub fn conscripts(&self) -> HashSet<StarConscriptKind> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![StarConscriptKind::req(StarKind::Space)],
                StarKind::Space => {
                    vec![
                        StarConscriptKind::req(StarKind::FileStore),
                        StarConscriptKind::req(StarKind::Web),
                        StarConscriptKind::req(StarKind::ArtifactStore),
                        StarConscriptKind::opt(StarKind::K8s),
                        StarConscriptKind::opt(StarKind::App),
                    ]
                }
                StarKind::Mesh => vec![],
                StarKind::App => vec![StarConscriptKind::req(StarKind::Mechtron), StarConscriptKind::req(StarKind::FileStore)],
                StarKind::Mechtron => vec![],
                StarKind::FileStore => vec![],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![],
                StarKind::Web => vec![],
                StarKind::ArtifactStore => vec![],
                StarKind::K8s => vec![],
            }
            .iter()
            .cloned(),
        )
    }

    pub fn manages(&self) -> HashSet<ResourceType> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![ResourceType::Space],
                StarKind::Space => vec![
                    ResourceType::App,
                    ResourceType::FileSystem,
                    ResourceType::Proxy,
                    ResourceType::Database,
                ],
                StarKind::Mesh => vec![],
                StarKind::App => vec![
                    ResourceType::Mechtron,
                    ResourceType::FileSystem,
                    ResourceType::Database,
                ],
                StarKind::Mechtron => vec![],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![],
                StarKind::Web => vec![],
                StarKind::FileStore => vec![ResourceType::File],
                StarKind::ArtifactStore => vec![ResourceType::Artifact],
                StarKind::K8s => vec![ResourceType::Database],
            }
            .iter()
            .cloned(),
        )
    }

    pub fn registry(rt: &ResourceType) -> StarKind {
        match rt {
            ResourceType::Root => Self::Central,
            ResourceType::Space => Self::Central,
            ResourceType::User => Self::Space,
            ResourceType::UserBase => Self::Space,
            ResourceType::App => Self::Space,
            ResourceType::Mechtron => Self::App,
            ResourceType::FileSystem => Self::Space,
            ResourceType::File => Self::Space,
            ResourceType::Database => Self::K8s,
            ResourceType::Authenticator=> Self::K8s,
            ResourceType::ArtifactBundleSeries => Self::Space,
            ResourceType::ArtifactBundle => Self::ArtifactStore,
            ResourceType::Artifact => Self::ArtifactStore,
            ResourceType::Proxy => Self::Space,
            ResourceType::Credentials=> Self::Space,
        }
    }

    pub fn hosts(rt: &ResourceType) -> StarKind {
        match rt {
            ResourceType::Root => Self::Central,
            ResourceType::Space => Self::Space,
            ResourceType::User => Self::Space,
            ResourceType::App => Self::App,
            ResourceType::Mechtron => Self::Mechtron,
            ResourceType::FileSystem => Self::FileStore,
            ResourceType::File => Self::FileStore,
            ResourceType::Database => Self::K8s,
            ResourceType::Authenticator=> Self::K8s,
            ResourceType::UserBase => Self::Space,
            ResourceType::ArtifactBundleSeries => Self::ArtifactStore,
            ResourceType::ArtifactBundle => Self::ArtifactStore,
            ResourceType::Artifact => Self::ArtifactStore,
            ResourceType::Proxy => Self::Space,
            ResourceType::Credentials => Self::Space,
        }
    }

    pub fn hosted(&self) -> HashSet<ResourceType> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![ResourceType::Root],
                StarKind::Space => vec![
                    ResourceType::Space,
                    ResourceType::User,
                    ResourceType::UserBase,
                    ResourceType::Proxy,
                ],
                StarKind::Mesh => vec![],
                StarKind::App => vec![ResourceType::App],
                StarKind::Mechtron => vec![ResourceType::Mechtron],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![ResourceType::Mechtron],
                StarKind::Web => vec![],
                StarKind::FileStore => vec![ResourceType::FileSystem, ResourceType::File],
                StarKind::ArtifactStore => {
                    vec![
                        ResourceType::ArtifactBundleSeries,
                        ResourceType::ArtifactBundle,
                        ResourceType::Artifact,
                    ]
                }
                StarKind::K8s => vec![ResourceType::Database],
            }
            .iter()
            .cloned(),
        )
    }
}

#[derive(Clone,Hash,Eq,PartialEq)]
pub struct StarConscriptKind{
    pub kind: StarKind,
    pub required: bool
}

impl StarConscriptKind {
    pub fn req(kind:StarKind)->Self {
        Self {
            kind,
            required: true
        }
    }

    pub fn opt(kind:StarKind)->Self {
        Self {
            kind,
            required: false
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ServerKindExt {
    pub name: String,
}

impl ServerKindExt {
    pub fn new(name: String) -> Self {
        ServerKindExt { name: name }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct StoreKindExt {
    pub name: String,
}

impl StoreKindExt {
    pub fn new(name: String) -> Self {
        StoreKindExt { name: name }
    }
}

impl StarKind {
    pub fn is_central(&self) -> bool {
        if let StarKind::Central = self {
            return true;
        } else {
            return false;
        }
    }

    pub fn is_supervisor(&self) -> bool {
        if let StarKind::App = self {
            return true;
        } else {
            return false;
        }
    }

    pub fn is_client(&self) -> bool {
        if let StarKind::Client = self {
            return true;
        } else {
            return false;
        }
    }

    pub fn central_result(&self) -> Result<(), Error> {
        if let StarKind::Central = self {
            Ok(())
        } else {
            Err("not central".into())
        }
    }

    pub fn supervisor_result(&self) -> Result<(), Error> {
        if let StarKind::App = self {
            Ok(())
        } else {
            Err("not supervisor".into())
        }
    }

    pub fn server_result(&self) -> Result<(), Error> {
        if let StarKind::Mechtron = self {
            Ok(())
        } else {
            Err("not server".into())
        }
    }

    pub fn client_result(&self) -> Result<(), Error> {
        if let StarKind::Client = self {
            Ok(())
        } else {
            Err("not client".into())
        }
    }

    pub fn relay(&self) -> bool {
        match self {
            StarKind::Central => false,
            StarKind::Mesh => true,
            StarKind::App => false,
            StarKind::Mechtron => true,
            StarKind::Gateway => true,
            StarKind::Client => true,
            StarKind::Link => true,
            StarKind::Space => false,
            StarKind::Web => false,
            StarKind::FileStore => false,
            StarKind::ArtifactStore => false,
            StarKind::K8s => false,
        }
    }
}



pub static MAX_HOPS: usize = 32;

pub struct Star {
    skel: StarSkel,
    star_rx: mpsc::Receiver<StarCommand>,
    core_tx: mpsc::Sender<CoreMessageCall>,
    lanes: HashMap<StarKey, LaneWrapper>,
    proto_lanes: Vec<LaneWrapper>,
    connector_ctrls: Vec<ConnectorController>,
    frame_hold: FrameHold,
    messages_received: HashMap<MessageId, Instant>,
    message_reply_trackers: HashMap<MessageId, MessageReplyTracker>,
    star_subgraph_expansion_seq: AtomicU64,

    status: StarStatus,
    status_broadcast: broadcast::Sender<StarStatus>,
}

impl Debug for Star {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        f.write_str(self.skel.info.to_string().as_str());
        Ok(())
    }
}

impl Star {
    pub async fn from_proto(
        data: StarSkel,
        star_rx: mpsc::Receiver<StarCommand>,
        core_tx: mpsc::Sender<CoreMessageCall>,
        lanes: HashMap<StarKey, LaneWrapper>,
        proto_lanes: Vec<LaneWrapper>,
        connector_ctrls: Vec<ConnectorController>,
        frame_hold: FrameHold,
    ) -> Self {
        let (status_broadcast, _) = broadcast::channel(8);
        Star {
            skel: data,
            star_rx: star_rx,
            lanes: lanes,
            proto_lanes: proto_lanes,
            connector_ctrls: connector_ctrls,
            frame_hold: frame_hold,
            messages_received: HashMap::new(),
            message_reply_trackers: HashMap::new(),
            star_subgraph_expansion_seq: AtomicU64::new(0),
            status: StarStatus::Unknown,
            status_broadcast: status_broadcast,
            core_tx,
        }
    }

    pub fn info(&self) -> StarInfo {
        self.skel.info.clone()
    }

    #[instrument]
    pub async fn run(mut self) {
        loop {

            let command = self.star_rx.recv().await;

            if let Some(command) = command {
//                let instructions = self.variant.filter(&command, &mut lane);

                    match command {
                        StarCommand::Init => {
                            self.init().await;
                        }
                        StarCommand::GetStarInfo(tx) => {
                            tx.send(Option::Some(self.skel.info.clone()));
                        }
                        StarCommand::SetFlags(set_flags) => {
                            self.skel.flags = set_flags.flags;
                            set_flags.tx.send(());
                        }
                        StarCommand::AddConnectorController(connector_ctrl) => {
                            self.connector_ctrls.push(connector_ctrl);
                        }
                        StarCommand::ReleaseHold(star) => {
                            unimplemented!()
/*                            if let Option::Some(frames) = self.frame_hold.release(&star) {
                                let lane = self.lane_with_shortest_path_to_star(&star);
                                if let Option::Some(lane) = lane {
                                    for frame in frames {
                                        lane.outgoing()
                                            .out_tx
                                            .send(LaneCommand::Frame(frame))
                                            .await;
                                    }
                                } else {
                                    eprintln!("release hold called on star that is not ready!")
                                }
                            }

 */
                        }

                        StarCommand::AddLogger(_tx) => {
                            //                        self.logger.tx.push(tx);
                        }
                        StarCommand::Test(_test) => {
                            /*                        match test
                                                   {
                                                       StarTest::StarSearchForStarKey(star) => {
                                                           let search = Search{
                                                               pattern: StarSearchPattern::StarKey(star),
                                                               tx: (),
                                                               max_hops: 0
                                                           };
                                                           self.do_search(star).await;
                                                       }
                                                   }

                            */
                        }

                        StarCommand::CheckStatus => {
                            self.check_status().await;
                        }
                        StarCommand::SetStatus(status) => {
                            self.set_status(status.clone());
                            //                            println!("{} {}", &self.skel.info.kind, &self.status.to_string());
                        }
                        StarCommand::Diagnose(diagnose) => {
                            self.diagnose(diagnose).await;
                        }
                        StarCommand::GetStatusListener(tx) => {
                            tx.send(self.status_broadcast.subscribe());
                            self.status_broadcast.send(self.status.clone());
                        }

                        StarCommand::GetSkel(tx) => {
                            tx.send(self.skel.clone()).unwrap_or_default();
                        }
                        StarCommand::AddProtoLaneEndpoint(lane) => {
                                   lane.outgoing
                                        .out_tx
                                        .try_send(LaneCommand::Frame(Frame::Proto(
                                            ProtoFrame::ReportStarKey(self.skel.info.key.clone()),
                                        ))).unwrap_or_default();

                            self.skel.lane_muxer_api.add_proto_lane(lane, StarPattern::Any );
                        }
                       StarCommand::Shutdown => {
                            for (_, lane) in &mut self.lanes {
                                lane.outgoing().out_tx.try_send(LaneCommand::Shutdown);
                            }
                            for lane in &mut self.proto_lanes {
                                lane.outgoing().out_tx.try_send(LaneCommand::Shutdown);
                            }

                            self.lanes.clear();
                            self.proto_lanes.clear();

                            break;
                        }
                        _ => {
                            unimplemented!("cannot process command: {}", command.to_string());
                        }
                    }
                }
        }
    }

    async fn init(&mut self) {
        self.refresh_conscript_wrangles().await;
        self.check_status().await;
    }

    fn set_status(&mut self, status: StarStatus) {
        self.status = status.clone();
        self.status_broadcast.send(status.clone() );

        let notification = Notification{
            selector: WatchSelector { topic: Topic::Star(self.skel.info.key.clone()), property: Property::Status },
            changes: vec![Change::Status(status)]
        };
        self.skel.watch_api.fire(notification);
    }

    async fn refresh_conscript_wrangles(&mut self) {
        if self.status == StarStatus::Unknown {
            self.set_status(StarStatus::Pending);
        }

        if let Option::Some(star_handler) = &self.skel.star_handler {

            for conscript_kind in self.skel.info.kind.conscripts() {
                let search = SearchInit::new(StarPattern::StarKind(conscript_kind.kind.clone()), TraversalAction::SearchHits);
                let (tx,rx) = oneshot::channel();
                self.skel.star_search_api.tx.try_send(StarSearchCall::Search {init:search, tx} ).unwrap_or_default();
                let star_handler = star_handler.clone();
                let kind = conscript_kind.kind.clone();
                let skel = self.skel.clone();
                tokio::spawn(async move {
                    let result = tokio::time::timeout(Duration::from_secs(15), rx).await;
                    match result {
                        Ok(Ok(hits)) => {
                            for (star, hops) in hits.hits {
                                let handle = StarConscript {
                                    key: star,
                                    kind: kind.clone(),
                                    hops: Option::Some(hops),
                                };
                                let result = star_handler.add_star_handle(handle).await;
                                match result {
                                    Ok(_) => {
                                        skel.star_tx.send(StarCommand::CheckStatus).await;
                                    }
                                    Err(error) => {
                                        eprintln!(
                                            "error when adding star handle: {}",
                                            error.to_string()
                                        )
                                    }
                                }
                            }
                        }
                        Err(error) => {
                            error!(
                            "error encountered when attempting to get a handle for: {} TIMEOUT: {}",
                            kind.to_string(), error.to_string()
                        );
                        }
                        Ok(Err(error)) => {
                            error!(
                                "error encountered when attempting to get a handle for: {} ERROR: {}",
                                kind.to_string(), error.to_string()
                            );
                        }
                    }
                });
            }
        }
    }

    async fn check_status(&mut self) {
        if self.status == StarStatus::Pending {
            if let Option::Some(star_handler) = &self.skel.star_handler {
                let satisfied = star_handler
                    .satisfied(self.skel.info.kind.conscripts())
                    .await;
                if let Result::Ok(StarConscriptionSatisfaction::Ok) = satisfied {
                    self.set_status(StarStatus::Initializing);
                    let skel = self.skel.clone();
                    tokio::spawn(async move {
                        let result = skel.variant_api.init().await;
                        match result {
                            Ok(_) => {
                                skel.star_tx
                                    .try_send(StarCommand::SetStatus(StarStatus::Ready)).unwrap_or_default();
                            }
                            Err(error) => {
                                skel.star_tx
                                    .try_send(StarCommand::SetStatus(StarStatus::Panic)).unwrap_or_default();
                                error!("{}",error.to_string())
                            }
                        }

                    });
                } else if let Result::Ok(StarConscriptionSatisfaction::Lacking(lacking)) = satisfied {
                    let mut s = String::new();
                    for lack in lacking {
                        s.push_str(lack.to_string().as_str());
                        s.push_str(", ");
                    }
//                    eprintln!("handles not satisfied for : {} Lacking: [ {}]", self.skel.info.kind.to_string(), s);
                }
            } else {
                self.set_status(StarStatus::Initializing);
                let skel = self.skel.clone();
                tokio::spawn(async move {
                    let result = skel.variant_api.init().await;
                    match result {
                        Ok(_) => {
                            skel.star_tx
                                .try_send(StarCommand::SetStatus(StarStatus::Ready)).unwrap_or_default();
                        }
                        Err(error) => {
                            skel.star_tx
                                .try_send(StarCommand::SetStatus(StarStatus::Panic)).unwrap_or_default();
                            error!("{}",error.to_string())
                        }
                    }
                });
            }
        }
    }

    pub async fn wait_for_it<R>(rx: oneshot::Receiver<Result<R, Fail>>) -> Result<R, Fail> {
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(result) => match result {
                Ok(result) => result,
                Err(_err) => Err(Fail::ChannelRecvErr),
            },
            Err(_) => Err(Fail::Timeout),
        }
    }

    pub fn star_key(&self) -> &StarKey {
        &self.skel.info.key
    }

    pub fn star_tx(&self) -> mpsc::Sender<StarCommand> {
        self.skel.star_tx.clone()
    }

    pub fn surface_api(&self) -> SurfaceApi {
        self.skel.surface_api.clone()
    }

    async fn diagnose(&self, diagnose: Diagnose) {
        match diagnose {
            Diagnose::HandlersSatisfied(satisfied) => {
                if let Option::Some(star_handler) = &self.skel.star_handler {
                    if let Result::Ok(satisfaction) = star_handler
                        .satisfied(self.skel.info.kind.conscripts())
                        .await
                    {
                        satisfied.tx.send(satisfaction);
                    } else {
                        // let satisfied.tx drop since we can't give it an answer
                    }
                } else {
                    satisfied.tx.send(StarConscriptionSatisfaction::Ok);
                }
            }
        }
    }

    pub fn log<T>(&self, sub: LogId<T>, method: &str, message: &str)
    where
        LogId<T>: ToString,
    {
        println!(
            "{} => {} : {} | {}",
            LogId(self).to_string(),
            sub.to_string(),
            method,
            message
        );
    }
}

pub trait StarKernel: Send {}

#[derive(strum_macros::Display)]
pub enum StarCommand {
    InvokeProtoStarEvolution,
    GetStatusListener(oneshot::Sender<broadcast::Receiver<StarStatus>>),
    AddProtoLaneEndpoint(ProtoLaneEnd),
    ConstellationBroadcast(ConstellationBroadcast),
    Init,
    AddConnectorController(ConnectorController),
    AddLogger(broadcast::Sender<Logger>),
    SetFlags(SetFlags),
    ReleaseHold(StarKey),
    GetStarInfo(oneshot::Sender<Option<StarInfo>>),

    Test(StarTest),

    Frame(Frame),
    ForwardFrame(ForwardFrame),
    FrameTimeout(FrameTimeoutInner),
    FrameError(FrameErrorInner),

    Diagnose(Diagnose),
    CheckStatus,
    SetStatus(StarStatus),
    RefreshHandles,

    GetCaches(oneshot::Sender<Arc<ProtoArtifactCachesFactory>>),
    GetLaneForStar {
        star: StarKey,
        tx: oneshot::Sender<Result<UltimaLaneKey, Error>>,
    },
    Shutdown,
    GetSkel(oneshot::Sender<StarSkel>),
    Broadcast { frame: Frame, exclude: Option<HashSet<UltimaLaneKey>> },
    LaneKeys(oneshot::Sender<Vec<UltimaLaneKey>>),
    LaneWithShortestPathToStar { star: StarKey, tx: oneshot::Sender<Option<UltimaLaneKey>> },
    GatewayAssign(Vec<StarSubGraphKey>)
}

#[derive(Clone)]
pub enum ConstellationBroadcast {
    Status(ConstellationStatus),
}

pub enum Diagnose {
    HandlersSatisfied(YesNo<StarConscriptionSatisfaction>),
}

pub struct SetFlags {
    pub flags: Flags,
    pub tx: oneshot::Sender<()>,
}

pub struct ForwardFrame {
    pub to: StarKey,
    pub frame: Frame,
}

pub struct AddResourceLocation {
    pub tx: mpsc::Sender<()>,
    pub resource_location: ResourceRecord,
}

pub struct Request<P: Debug, R> {
    pub payload: P,
    pub tx: oneshot::Sender<Result<R, Error>>,
    pub log: bool,
}

impl<P: Debug, R> Debug for Request<P, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.payload.fmt(f)
    }
}

impl<P: Debug, R> Request<P, R> {
    pub fn new(payload: P) -> (Self, oneshot::Receiver<Result<R, Error>>) {
        let (tx, rx) = oneshot::channel();
        (
            Request {
                payload: payload,
                tx: tx,
                log: false,
            },
            rx,
        )
    }
}

pub struct Query<P, R> {
    pub payload: P,
    pub tx: oneshot::Sender<R>,
}

impl<P, R> Query<P, R> {
    pub fn new(payload: P) -> (Self, oneshot::Receiver<R>) {
        let (tx, rx) = oneshot::channel();
        (
            Query {
                payload: payload,
                tx: tx,
            },
            rx,
        )
    }
}

pub struct YesNo<R> {
    pub tx: oneshot::Sender<R>,
}

impl<R> YesNo<R> {
    pub fn new() -> (Self, oneshot::Receiver<R>) {
        let (tx, rx) = oneshot::channel();
        (YesNo { tx: tx }, rx)
    }
}

pub struct Set<P> {
    pub payload: P,
    pub tx: oneshot::Sender<P>,
}

impl<P> Set<P> {
    pub fn new(payload: P) -> (Self, oneshot::Receiver<P>) {
        let (tx, rx) = oneshot::channel();
        (
            Set {
                payload: payload,
                tx: tx,
            },
            rx,
        )
    }

    pub fn commit(self) {
        self.tx.send(self.payload);
    }
}

pub struct Empty {}

impl Empty {
    pub fn new() -> Self {
        Empty {}
    }
}

pub struct FrameTimeoutInner {
    pub frame: Frame,
    pub retries: usize,
}

pub struct FrameErrorInner {
    pub frame: Frame,
    pub message: String,
}

pub enum StarTest {
    StarSearchForStarKey(StarKey),
}

#[derive(Clone)]
pub struct StarController {
    pub star_tx: mpsc::Sender<StarCommand>,
    pub surface_api: SurfaceApi,
}

impl StarController {
    pub async fn set_flags(&self, flags: Flags) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();

        let set_flags = SetFlags {
            flags: flags,
            tx: tx,
        };

        self.star_tx.send(StarCommand::SetFlags(set_flags)).await;
        rx
    }

    pub async fn diagnose_handlers_satisfaction(&self) -> Result<StarConscriptionSatisfaction, Error> {
        let (yesno, rx) = YesNo::new();
        self.star_tx
            .send(StarCommand::Diagnose(Diagnose::HandlersSatisfied(yesno)))
            .await;
        Ok(tokio::time::timeout(Duration::from_secs(5), rx).await??)
    }

    pub async fn get_star_info(&self) -> Result<Option<StarInfo>, Error> {
        let (tx, rx) = oneshot::channel();
        self.star_tx.send(StarCommand::GetStarInfo(tx)).await;
        Ok(rx.await?)
    }
}

pub struct ResourceLocationRequestTransaction {
    pub tx: mpsc::Sender<()>,
}

impl ResourceLocationRequestTransaction {
    pub fn new() -> (Self, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel(1);
        (ResourceLocationRequestTransaction { tx: tx }, rx)
    }
}

pub struct FrameHold {
    hold: HashMap<StarKey, Vec<Frame>>,
}

impl FrameHold {
    pub fn new() -> Self {
        FrameHold {
            hold: HashMap::new(),
        }
    }

    pub fn add(&mut self, star: &StarKey, frame: Frame) {
        if !self.hold.contains_key(star) {
            self.hold.insert(star.clone(), vec![]);
        }
        if let Option::Some(frames) = self.hold.get_mut(star) {
            frames.push(frame);
        }
    }

    pub fn release(&mut self, star: &StarKey) -> Option<Vec<Frame>> {
        self.hold.remove(star)
    }

    pub fn has_hold(&self, star: &StarKey) -> bool {
        return self.hold.contains_key(star);
    }
}

#[derive(PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarSubGraphKey {
    Big(u64),
    Small(u16),
}

impl ToString for StarSubGraphKey {
    fn to_string(&self) -> String {
        match self {
            StarSubGraphKey::Big(n) => n.to_string(),
            StarSubGraphKey::Small(n) => n.to_string(),
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKey {
    pub subgraph: Vec<StarSubGraphKey>,
    pub index: u16,
}

impl StarKey {
    pub fn central() -> Self {
        StarKey {
            subgraph: vec![],
            index: 0,
        }
    }
}

impl StarKey {
    pub fn bin(&self) -> Result<Vec<u8>, Error> {
        let bin = bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn from_bin(bin: Vec<u8>) -> Result<StarKey, Error> {
        let key = bincode::deserialize::<StarKey>(bin.as_slice())?;
        Ok(key)
    }
}

impl cmp::Ord for StarKey {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.subgraph.len() > other.subgraph.len() {
            Ordering::Greater
        } else if self.subgraph.len() < other.subgraph.len() {
            Ordering::Less
        } else if self.subgraph.cmp(&other.subgraph) != Ordering::Equal {
            return self.subgraph.cmp(&other.subgraph);
        } else {
            return self.index.cmp(&other.index);
        }
    }
}

impl ToString for StarKey {
    fn to_string(&self) -> String {
        if self.subgraph.len() > 0 {
            let mut string = String::new();
            for (index, node) in self.subgraph.iter().enumerate() {
                if index != 0 {
                    string.push_str("-");
                }
                string.push_str(node.to_string().as_str());
            }
            format!("{}-{}", string, self.index)
        } else {
            self.index.to_string()
        }
    }
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct StarTemplateId {
    pub constellation: String,
    pub handle: StarTemplateHandle,
}

impl StarKey {
    pub fn new(index: u16) -> Self {
        StarKey {
            subgraph: vec![],
            index: index,
        }
    }

    pub fn new_with_subgraph(subgraph: Vec<StarSubGraphKey>, index: u16) -> Self {
        StarKey {
            subgraph,
            index: index,
        }
    }

    pub fn with_index(&self, index: u16) -> Self {
        StarKey {
            subgraph: self.subgraph.clone(),
            index: index,
        }
    }

    // highest to lowest
    pub fn sort(a: StarKey, b: StarKey) -> Result<(Self, Self), Error> {
        if a == b {
            Err(format!(
                "both StarKeys are equal. {}=={}",
                a.to_string(),
                b.to_string()
            )
            .into())
        } else if a.cmp(&b) == Ordering::Greater {
            Ok((a, b))
        } else {
            Ok((b, a))
        }
    }

    pub fn child_subgraph(&self) -> Vec<StarSubGraphKey> {
        let mut subgraph = self.subgraph.clone();
        subgraph.push(StarSubGraphKey::Small(self.index));
        subgraph
    }
}

#[derive(Clone)]
pub enum Persistence {
    Memory,
}

#[derive(Clone)]
pub struct StarSkel {
    pub info: StarInfo,
    pub star_tx: mpsc::Sender<StarCommand>,
    pub core_messaging_endpoint_tx: mpsc::Sender<CoreMessageCall>,
    pub resource_locator_api: ResourceLocatorApi,
    pub star_search_api: StarSearchApi,
    pub router_api: RouterApi,
    pub surface_api: SurfaceApi,
    pub messaging_api: MessagingApi,
    pub golden_path_api: GoldenPathApi,
    pub lane_muxer_api: LaneMuxerApi,
    pub variant_api: VariantApi,
    pub watch_api: WatchApi,
    pub flags: Flags,
    pub logger: Logger,
    pub sequence: Arc<AtomicU64>,
    pub registry: Option<Arc<dyn ResourceRegistryBacking>>,
    pub star_handler: Option<StarWranglerBacking>,
    pub persistence: Persistence,
    pub data_access: FileAccess,
    pub machine: StarlaneMachine,
}

impl Debug for StarSkel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.info.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct StarInfo {
    pub key: StarKey,
    pub kind: StarKind,
}

impl StarInfo {
    pub fn new(star: StarKey, kind: StarKind) -> Self {
        StarInfo {
            key: star,
            kind: kind,
        }
    }

    pub fn mock() -> Self {
        StarInfo {
            key: StarKey {
                subgraph: vec![],
                index: 0,
            },
            kind: StarKind::Central,
        }
    }
}

impl LogInfo for StarInfo {
    fn log_identifier(&self) -> String {
        self.key.to_string()
    }

    fn log_kind(&self) -> String {
        self.kind.to_string()
    }

    fn log_object(&self) -> String {
        "StarInfo".to_string()
    }
}

impl LogInfo for Star {
    fn log_identifier(&self) -> String {
        self.skel.info.key.to_string()
    }

    fn log_kind(&self) -> String {
        self.skel.info.kind.to_string()
    }

    fn log_object(&self) -> String {
        "Star".to_string()
    }
}

impl ToString for StarInfo {
    fn to_string(&self) -> String {
        format!("<{}>::[{}]", self.kind.to_string(), self.key.to_string())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct StarNotify {
    pub star: StarKey,
    pub transaction: Id,
}

impl StarNotify {
    pub fn new(star: StarKey, transaction: Id) -> Self {
        StarNotify {
            star: star,
            transaction: transaction,
        }
    }
}

#[async_trait]
pub trait ResourceRegistryBacking: Sync + Send {
    async fn reserve(
        &self,
        request: ResourceNamesReservationRequest,
    ) -> Result<RegistryReservation, Error>;
    async fn register(&self, registration: ResourceRegistration) -> Result<(), Error>;
    async fn select(&self, select: ResourceSelector) -> Result<Vec<ResourceRecord>, Error>;
    async fn set_location(&self, location: ResourceRecord) -> Result<(), Error>;
    async fn get(&self, identifier: ResourceIdentifier) -> Result<Option<ResourceRecord>, Error>;
    async fn unique_src(&self, resource_type: ResourceType, key: ResourceIdentifier) -> Box<dyn UniqueSrc>;
    async fn update(&self, assignment: ResourceRegistryPropertyAssignment ) -> Result<(),Error>;
}

pub struct ResourceRegistryBackingSqLite {
    registry: tokio::sync::mpsc::Sender<ResourceRegistryAction>,
}

impl ResourceRegistryBackingSqLite {
    pub async fn new(star_info: StarInfo, star_data_path: String) -> Result<Self, Error> {
        let rtn = ResourceRegistryBackingSqLite {
            registry: Registry::new(star_info, star_data_path).await,
        };

        Ok(rtn)
    }

    async fn timeout<X>(rx: oneshot::Receiver<X>) -> Result<X, Error> {
        Ok(tokio::time::timeout(Duration::from_secs(25), rx).await??)
    }
}

#[async_trait]
impl ResourceRegistryBacking for ResourceRegistryBackingSqLite {
    async fn reserve(
        &self,
        request: ResourceNamesReservationRequest,
    ) -> Result<RegistryReservation, Error> {
        let (action, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Reserve(request));
        self.registry.send(action).await?;

        match Self::timeout(rx).await? {
            ResourceRegistryResult::Reservation(reservation) => Result::Ok(reservation),
            _ => Result::Err(Fail::expected("ResourceRegistryResult::Reservation(_)").into()),
        }

        /*        match tokio::time::timeout(Duration::from_secs(5), rx).await?? {
                   ResourceRegistryResult::Reservation(reservation) => Result::Ok(reservation),
                   _ => Result::Err(Fail::Timeout),
               }
        */
    }

    async fn register(&self, registration: ResourceRegistration) -> Result<(), Error> {
        let (request, rx) =
            ResourceRegistryAction::new(ResourceRegistryCommand::Commit(registration));
        self.registry.send(request).await?;
        //        tokio::time::timeout(Duration::from_secs(5), rx).await??;
        Self::timeout(rx).await?;
        Ok(())
    }

    async fn select(&self, selector: ResourceSelector) -> Result<Vec<ResourceRecord>, Error> {
        let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
        self.registry.send(request).await?;
        // match tokio::time::timeout(Duration::from_secs(5), rx).await?? {
        match Self::timeout(rx).await? {
            ResourceRegistryResult::Resources(resources) => Result::Ok(resources),
            _ => Result::Err(Fail::Timeout.into()),
        }
    }

    async fn set_location(&self, location: ResourceRecord) -> Result<(), Error> {
        let (request, rx) =
            ResourceRegistryAction::new(ResourceRegistryCommand::SetLocation(location));
        self.registry.send(request).await;
        //tokio::time::timeout(Duration::from_secs(5), rx).await??;
        Self::timeout(rx).await?;
        Ok(())
    }

    async fn get(&self, identifier: ResourceIdentifier) -> Result<Option<ResourceRecord>, Error> {
        let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Get(identifier));
        self.registry.send(request).await;
        //match tokio::time::timeout(Duration::from_secs(5), rx).await?? {
        match Self::timeout(rx).await? {
            ResourceRegistryResult::Resource(resource) => {
                Ok(resource)
            }
            _ => Err(Fail::expected("ResourceRegistryResult::Resource(_)").into()),
        }
    }

    async fn unique_src(&self, resource_type: ResourceType, id: ResourceIdentifier) -> Box<dyn UniqueSrc> {
        Box::new(RegistryUniqueSrc::new(resource_type, id, self.registry.clone()))
    }

    async fn update(&self, assignment: ResourceRegistryPropertyAssignment ) -> Result<(),Error>
    {
        let (request, rx) =
            ResourceRegistryAction::new(ResourceRegistryCommand::Update(assignment));
        self.registry.send(request).await;
        Self::timeout(rx).await?;
        Ok(())
    }
}

pub type StarStatus = Status;

impl Into<LogId<String>> for &'static ResourceIdentifier {
    fn into(self) -> LogId<String> {
        match self {
            ResourceIdentifier::Key(key) => LogId(format!("[{}]", key.to_string())),
            ResourceIdentifier::Address(address) => LogId(format!("'{}'", address.to_string())),
        }
    }
}

impl Into<LogId<String>> for &'static Star {
    fn into(self) -> LogId<String> {
        LogId(self.skel.info.to_string())
    }
}

impl Into<LogId<String>> for &'static StarMessage {
    fn into(self) -> LogId<String> {
        LogId(format!("<Message>[{}]", self.id.to_string()))
    }
}

impl Into<LogId<String>> for &'static ProtoStarMessage {
    fn into(self) -> LogId<String> {
        LogId("<proto>".to_string())
    }
}

pub struct LogId<T>(T);

impl<T> ToString for LogId<T> {
    fn to_string(&self) -> String {
        "log-id".to_string()
    }
}
