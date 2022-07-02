use std::cmp::{min, Ordering};
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::fmt::{Debug, Formatter};
use std::iter::FromIterator;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::select_all;
use futures::FutureExt;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use shell::search::{
    SearchCommit, SearchHits, SearchInit, StarSearchTransaction, TransactionResult,
};

use crate::cache::ProtoArtifactCachesFactory;
use crate::constellation::ConstellationStatus;
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::{Frame, ProtoFrame, StarMessage, StarPattern, TraversalAction};
use crate::id::Id;
use crate::lane::{
    ConnectorController, LaneCommand, LaneEnd, LaneIndex, LaneMeta, LaneWrapper, ProtoLaneEnd,
    UltimaLaneKey,
};

use crate::message::{
    MessageId, MessageReplyTracker, MessageResult, MessageUpdate, ProtoStarMessage,
    ProtoStarMessageTo, TrackerJob,
};
use crate::star::core::message::CoreMessageCall;
use crate::star::shell::golden::GoldenPathApi;
use crate::star::shell::lanes::LaneMuxerApi;
use crate::star::shell::message::MessagingApi;
use crate::star::shell::router::RouterApi;
use crate::star::shell::search::{StarSearchApi, StarSearchCall};
use crate::star::shell::watch::WatchApi;
use crate::star::surface::SurfaceApi;
use crate::star::variant::{FrameVerdict, VariantApi};
use crate::starlane::StarlaneMachine;
use crate::template::StarHandle;
use crate::watch::{Change, Notification, Property, Topic, WatchSelector};
use std::cmp;
use std::fmt;
use std::future::Future;
use std::num::ParseIntError;
use crate::star::core::particle::driver::ResourceCoreDriverApi;
use std::str::FromStr;
use mesh_portal::version::latest::id::{Point, Port};
use mesh_portal::version::latest::log::{PointLogger, RootLogger};
use mesh_portal::version::latest::particle::Status;
use cosmic_api::version::v0_0_1::parse::error::result;
use mysql::prelude::FromRow;
use mysql::{FromRowError, Row};
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::multi::many0;
use nom::bytes::complete::tag;
use nom::character::complete::digit1;
use nom::branch::alt;
use nom::combinator::all_consuming;
use nom::error::{ErrorKind, ParseError, VerboseError};
use nom::Parser;
use nom_supreme::error::ErrorTree;
use sqlx::postgres::PgTypeInfo;
use cosmic_nom::{new_span, Res, Span};
use cosmic_api::version::v0_0_1::id::{ConstellationName, StarKey};
use cosmic_api::version::v0_0_1::id::id::{BaseKind, RouteSeg, ToPoint, ToPort};
use cosmic_api::version::v0_0_1::parse::lowercase_alphanumeric;
use cosmic_api::version::v0_0_1::sys::ParticleRecord;
use crate::registry::RegistryApi;
use crate::logger::{Flags, Logger, LogInfo};
use crate::star::shell::db::{StarDB, StarDBApi, StarWrangle, StarWrangleSatisfaction};
use crate::star::shell::sys::SysApi;

pub mod core;
pub mod surface;
pub mod variant;
pub mod shell;

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
    sqlx::Decode,
    sqlx::Encode,
)]
pub enum StarKind {
    Central,
    Space,
    Relay,
    App,
    Exe,
    FileStore,
    ArtifactStore,
    Gateway,
    Link,
    Client,
    Web,
    K8s,
    Portal
}

impl FromRow for StarKind {
    fn from_row_opt(row: Row) -> Result<Self, FromRowError> where Self: Sized {
        Ok(StarKind::Link)
    }
}

impl StarKind {
    pub fn is_resource_manager(&self) -> bool {
        match self {
            StarKind::Central => true,
            StarKind::Space => true,
            StarKind::Relay => false,
            StarKind::App => true,
            StarKind::Exe => false,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => false,
            StarKind::Web => false,
            StarKind::ArtifactStore => true,
            StarKind::K8s => true,
            StarKind::Portal => true
        }
    }

    pub fn is_resource_host(&self) -> bool {
        match self {
            StarKind::Central => false,
            StarKind::Space => true,
            StarKind::Relay => false,
            StarKind::App => true,
            StarKind::Exe => true,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => true,
            StarKind::Web => true,
            StarKind::ArtifactStore => true,
            StarKind::K8s => true,
            StarKind::Portal => true
        }
    }

    pub fn wrangles(&self) -> HashSet<StarWrangleKind> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![StarWrangleKind::req(StarKind::Space)],
                StarKind::Space => {
                    vec![
                        StarWrangleKind::req(StarKind::FileStore),
                        StarWrangleKind::req(StarKind::Web),
                        StarWrangleKind::req(StarKind::ArtifactStore),
                        StarWrangleKind::opt(StarKind::K8s),
                        StarWrangleKind::opt(StarKind::App),
                    ]
                }
                StarKind::Relay => vec![],
                StarKind::App => vec![
                    StarWrangleKind::req(StarKind::Exe),
                    StarWrangleKind::req(StarKind::FileStore),
                ],
                StarKind::Exe => vec![],
                StarKind::FileStore => vec![],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![],
                StarKind::Web => vec![],
                StarKind::ArtifactStore => vec![],
                StarKind::K8s => vec![],
                StarKind::Portal => vec![]
            }
            .iter()
            .cloned(),
        )
    }

    pub fn manages(&self) -> HashSet<BaseKind> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![BaseKind::Space],
                StarKind::Space => vec![
                    BaseKind::App,
                    BaseKind::FileSystem,
                    BaseKind::Database,
                ],
                StarKind::Relay => vec![],
                StarKind::App => vec![
                    BaseKind::Mechtron,
                    BaseKind::FileSystem,
                    BaseKind::Database,
                ],
                StarKind::Exe => vec![],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![],
                StarKind::Web => vec![],
                StarKind::FileStore => vec![BaseKind::File],
                StarKind::ArtifactStore => vec![BaseKind::Artifact],
                StarKind::K8s => vec![BaseKind::Database],
                StarKind::Portal => vec![BaseKind::Control]
            }
            .iter()
            .cloned(),
        )
    }

    pub fn registry(rt: &BaseKind) -> StarKind {
        match rt {
            BaseKind::Root => Self::Central,
            BaseKind::Space => Self::Central,
            BaseKind::User => Self::Space,
            BaseKind::App => Self::Space,
            BaseKind::Mechtron => Self::App,
            BaseKind::FileSystem => Self::Space,
            BaseKind::File => Self::Space,
            BaseKind::Database => Self::K8s,
            BaseKind::BundleSeries => Self::Space,
            BaseKind::Bundle => Self::ArtifactStore,
            BaseKind::Artifact => Self::ArtifactStore,
            BaseKind::Base => Self::Space,
            BaseKind::Control => Self::Portal,
            BaseKind::UserBase => Self::Space
        }
    }

    pub fn hosts(rt: &BaseKind) -> StarKind {
        match rt {
            BaseKind::Root => Self::Central,
            BaseKind::Space => Self::Space,
            BaseKind::User => Self::Space,
            BaseKind::App => Self::App,
            BaseKind::Mechtron => Self::Exe,
            BaseKind::FileSystem => Self::FileStore,
            BaseKind::File => Self::FileStore,
            BaseKind::Database => Self::K8s,
            BaseKind::BundleSeries => Self::ArtifactStore,
            BaseKind::Bundle => Self::ArtifactStore,
            BaseKind::Artifact => Self::ArtifactStore,
            BaseKind::Base => Self::Space,
            BaseKind::Control => Self::Portal,
            BaseKind::UserBase => Self::Space
        }
    }

    pub fn hosted(&self) -> HashSet<BaseKind> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![BaseKind::Root],
                StarKind::Space => vec![
                    BaseKind::Space,
                    BaseKind::User,
                    BaseKind::Base,
                    BaseKind::UserBase,
                ],
                StarKind::Relay => vec![],
                StarKind::App => vec![BaseKind::App],
                StarKind::Exe => vec![BaseKind::Mechtron],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![BaseKind::Mechtron],
                StarKind::Web => vec![],
                StarKind::FileStore => vec![BaseKind::FileSystem, BaseKind::File],
                StarKind::ArtifactStore => {
                    vec![
                        BaseKind::BundleSeries,
                        BaseKind::Bundle,
                        BaseKind::Artifact,
                    ]
                }
                StarKind::K8s => vec![BaseKind::Database],
                StarKind::Portal => vec![BaseKind::Control]
            }
            .iter()
            .cloned(),
        )
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct StarWrangleKind {
    pub kind: StarKind,
    pub required: bool,
}

impl StarWrangleKind {
    pub fn req(kind: StarKind) -> Self {
        Self {
            kind,
            required: true,
        }
    }

    pub fn opt(kind: StarKind) -> Self {
        Self {
            kind,
            required: false,
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
        if let StarKind::Exe = self {
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
            StarKind::Relay => true,
            StarKind::App => false,
            StarKind::Exe => true,
            StarKind::Gateway => true,
            StarKind::Client => true,
            StarKind::Link => true,
            StarKind::Space => false,
            StarKind::Web => false,
            StarKind::FileStore => false,
            StarKind::ArtifactStore => false,
            StarKind::K8s => false,
            StarKind::Portal => false
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
                            .try_send(LaneCommand::Frame(Frame::Proto(ProtoFrame::ReportStarKey(
                                self.skel.info.key.clone(),
                            ))))
                            .unwrap_or_default();

                        self.skel
                            .lane_muxer_api
                            .add_proto_lane(lane, StarPattern::Any);
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
                    StarCommand::GetCaches(tx)=> {
                        match self.skel.machine.get_proto_artifact_caches_factory().await {
                            Ok(caches) => {
                                tx.send(caches);
                            }
                            Err(err) => {
                                error!("{}",err.to_string());
                            }
                        }
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
        self.status_broadcast.send(status.clone());

        let notification = Notification {
            selector: WatchSelector {
                topic: Topic::Star(self.skel.info.key.clone()),
                property: Property::Status,
            },
            changes: vec![Change::Status(status)],
        };
        self.skel.watch_api.fire(notification);
    }

    async fn refresh_conscript_wrangles(&mut self) {
        if self.status == StarStatus::Unknown {
            self.set_status(StarStatus::Pending);
        }

        for conscript_kind in self.skel.info.kind.wrangles() {
            let search = SearchInit::new(
                StarPattern::StarKind(conscript_kind.kind.clone()),
                TraversalAction::SearchHits,
            );
            let skel = self.skel.clone();
            let kind = conscript_kind.kind.clone();
            tokio::spawn(async move {
             let mut timeout = 1u64;
             loop {
                 let (tx2,rx2) = oneshot::channel();
                 skel
                     .star_search_api
                     .tx
                     .try_send(StarSearchCall::Search { init: search.clone(), tx: tx2 })
                     .unwrap_or_default();

                 let result = tokio::time::timeout(Duration::from_secs(timeout), rx2).await;
                 match result {
                     Ok(Ok(hits)) => {
                         for (star, hops) in hits.hits {
                             let wrangle = StarWrangle {
                                 key: star,
                                 kind: kind.clone(),
                                 hops: hops
                             };
                             let result = skel.star_db.set_wrangle(wrangle).await;
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
                         break;
                     }
                     Err(error) => {
                         error!(
                            "error encountered when attempting to wrangle a handle for: {} TIMEOUT: {}",
                            kind.to_string(),
                            error.to_string()
                        );
                     }
                     Ok(Err(error)) => {
                         error!(
                            "error encountered when attempting to wrangle a handle for: {} ERROR: {}",
                            kind.to_string(),
                            error.to_string()
                        );
                     }
                 }
                 info!("attempting wrangle again in 5 seconds...");
                 tokio::time::sleep(Duration::from_secs(5));
                 timeout = timeout + 5;
                 if timeout > 30 {
                     timeout = 30;
                 }
             }
            });
        }
    }

    async fn check_status(&mut self) {
        if self.status == StarStatus::Pending {
                let satisfied = self.skel.star_db
                    .wrangle_satisfaction(self.skel.info.kind.wrangles())
                    .await;
                if let Result::Ok(StarWrangleSatisfaction::Ok) = satisfied {
                    self.set_status(StarStatus::Pending);
                    let skel = self.skel.clone();
                    tokio::spawn(async move {
                        let result = skel.variant_api.init().await;
                        match result {
                            Ok(_) => {
                                skel.star_tx
                                    .try_send(StarCommand::SetStatus(StarStatus::Ready))
                                    .unwrap_or_default();
                            }
                            Err(error) => {
                                let err_msg = format!("{}", error.to_string());
                                skel.star_tx
                                    .try_send(StarCommand::SetStatus(StarStatus::Panic))
                                    .unwrap_or_default();
                                error!("{}", error.to_string())
                            }
                        }
                    });
                } else if let Result::Ok(StarWrangleSatisfaction::Lacking(lacking)) = satisfied {
                    let mut s = String::new();
                    for lack in lacking {
                        s.push_str(lack.to_string().as_str());
                        s.push_str(", ");
                    }
                    //                    eprintln!("handles not satisfied for : {} Lacking: [ {}]", self.skel.info.kind.to_string(), s);
                }

        }
    }

    pub async fn wait_for_it<R>(rx: oneshot::Receiver<Result<R, Error>>) -> Result<R, Error> {
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(result) => match result {
                Ok(result) => result,
                Err(_err) => Err("Fail::ChannelRecvErr".into()),
            },
            Err(_) => Err("Fail::Timeout".into()),
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
                    if let Result::Ok(satisfaction) = self.skel.star_db
                        .wrangle_satisfaction(self.skel.info.kind.wrangles())
                        .await
                    {
                        satisfied.tx.send(satisfaction);
                    } else {
                        // let satisfied.tx drop since we can't give it an answer
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
    Broadcast {
        frame: Frame,
        exclude: Option<HashSet<UltimaLaneKey>>,
    },
    LaneKeys(oneshot::Sender<Vec<UltimaLaneKey>>),
    LaneWithShortestPathToStar {
        star: StarKey,
        tx: oneshot::Sender<Option<UltimaLaneKey>>,
    },
}

#[derive(Clone)]
pub enum ConstellationBroadcast {
    Status(ConstellationStatus),
}

pub enum Diagnose {
    HandlersSatisfied(YesNo<StarWrangleSatisfaction>),
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
    pub resource_location: ParticleRecord,
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

    pub async fn diagnose_handlers_satisfaction(&self) -> Result<StarWrangleSatisfaction, Error> {
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


fn parse_star_key<I:Span>( input: I) -> Res<I,StarKey> {
    let (next,(_,constelation,_,name,index)) = tuple((tag("STAR::"),lowercase_alphanumeric,tag(":"),lowercase_alphanumeric,delimited(tag("["),digit1,tag("]"))) )(input.clone())?;
    let constelation = constelation.to_string();
    let name = name.to_string();
    let index = match index.to_string().parse::<u16>() {
        Ok(index) => index,
        Err(err) => {
            return Err(nom::Err::Failure(ErrorTree::from_error_kind(input, ErrorKind::Digit )))
        }
    };

    Ok((next, StarKey {
        constellation: constelation,
        name,
        index
    }))
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct StarTemplateId {
    pub constellation: String,
    pub handle: StarHandle,
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
    pub sys_api: SysApi,
    pub registry_api: RegistryApi,
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
    pub star_db: StarDBApi,
    pub persistence: Persistence,
    pub data_access: FileAccess,
    pub machine: StarlaneMachine,
    pub particle_logger: RootLogger
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
    pub point: Point
}

impl StarInfo {
    pub fn new(star: StarKey, kind: StarKind) -> Self {
        let point = Point::from_str(format!("<<{}>>::star", star.to_string()).as_str() ).expect("expect to be able to create a simple star address");
        StarInfo {
            key:star,
            kind ,
            point
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

pub type StarStatus = Status;

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

