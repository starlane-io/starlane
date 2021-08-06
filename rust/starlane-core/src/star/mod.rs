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

use shell::pledge::{Satisfaction, StarHandle, StarHandleBacking};
use shell::search::{SearchHits, SearchInit, StarSearchTransaction, TransactionResult, SearchCommit};
use starlane_resources::ResourceIdentifier;

use crate::cache::ProtoArtifactCachesFactory;
use crate::constellation::ConstellationStatus;
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::{
    ActorLookup, Frame, ProtoFrame, RegistryAction, Reply, SearchResults, SearchTraversal,
    SearchWindDown, SearchWindUp, SimpleReply, StarMessage, StarMessagePayload, StarPattern, TraversalAction, Watch,
    WatchInfo, SearchHit,
};
use crate::id::Id;
use crate::lane::{
    ConnectorController, LaneCommand, LaneEndpoint, LaneIndex, LaneKey, LaneMeta, LaneWrapper,
    ProtoLaneEndpoint,
};
use crate::logger::{Flags, Logger, LogInfo};
use crate::message::{
    Fail, MessageId, MessageReplyTracker, MessageResult, MessageUpdate, ProtoStarMessage,
    ProtoStarMessageTo, TrackerJob,
};
use crate::resource::{
    ActorKey, Registry, RegistryReservation, RegistryUniqueSrc, Resource, ResourceAddress,
    ResourceKey, ResourceNamesReservationRequest, ResourceRecord, ResourceRegistration,
    ResourceRegistryAction, ResourceRegistryCommand, ResourceRegistryResult, ResourceSelector,
    ResourceType, UniqueSrc,
};
use crate::star::core::message::CoreMessageCall;
use crate::star::shell::golden::GoldenPathApi;
use crate::star::shell::lanes::LanesApi;
use crate::star::shell::locator::ResourceLocatorApi;
use crate::star::shell::message::MessagingApi;
use crate::star::shell::router::RouterApi;
use crate::star::shell::search::{StarSearchApi, StarSearchCall};
use crate::star::surface::SurfaceApi;
use crate::star::variant::{StarShellInstructions, StarVariant};
use crate::starlane::StarlaneMachine;
use crate::template::StarTemplateHandle;

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
    Actor,
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
            StarKind::Actor => false,
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
            StarKind::Actor => true,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => true,
            StarKind::Web => true,
            StarKind::ArtifactStore => true,
            StarKind::K8s => true,
        }
    }

    pub fn distributes_to(&self) -> HashSet<StarKind> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![StarKind::Space],
                StarKind::Space => {
                    vec![
                        StarKind::FileStore,
                        StarKind::Web,
                        StarKind::ArtifactStore,
                        StarKind::K8s,
                    ]
                }
                StarKind::Mesh => vec![],
                StarKind::App => vec![StarKind::Actor, StarKind::FileStore],
                StarKind::Actor => vec![],
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
                    ResourceType::SubSpace,
                    ResourceType::App,
                    ResourceType::FileSystem,
                    ResourceType::Proxy,
                    ResourceType::Database,
                ],
                StarKind::Mesh => vec![],
                StarKind::App => vec![
                    ResourceType::Actor,
                    ResourceType::FileSystem,
                    ResourceType::Database,
                ],
                StarKind::Actor => vec![],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![],
                StarKind::Web => vec![ResourceType::Domain],
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
            ResourceType::SubSpace => Self::Space,
            ResourceType::User => Self::Space,
            ResourceType::App => Self::Space,
            ResourceType::Actor => Self::App,
            ResourceType::FileSystem => Self::Space,
            ResourceType::File => Self::Space,
            ResourceType::Database => Self::K8s,
            ResourceType::ArtifactBundleVersions => Self::Space,
            ResourceType::ArtifactBundle => Self::ArtifactStore,
            ResourceType::Artifact => Self::ArtifactStore,
            ResourceType::Proxy => Self::Space,
            ResourceType::Domain => Self::Space,
        }
    }

    pub fn hosts(rt: &ResourceType) -> StarKind {
        match rt {
            ResourceType::Root => Self::Central,
            ResourceType::Space => Self::Space,
            ResourceType::SubSpace => Self::Space,
            ResourceType::User => Self::Space,
            ResourceType::App => Self::App,
            ResourceType::Actor => Self::Actor,
            ResourceType::FileSystem => Self::FileStore,
            ResourceType::File => Self::FileStore,
            ResourceType::Database => Self::K8s,
            ResourceType::ArtifactBundleVersions => Self::ArtifactStore,
            ResourceType::ArtifactBundle => Self::ArtifactStore,
            ResourceType::Artifact => Self::ArtifactStore,
            ResourceType::Proxy => Self::Space,
            ResourceType::Domain => Self::Space,
        }
    }

    pub fn hosted(&self) -> HashSet<ResourceType> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![ResourceType::Root],
                StarKind::Space => vec![
                    ResourceType::Space,
                    ResourceType::SubSpace,
                    ResourceType::User,
                    ResourceType::Domain,
                    ResourceType::Proxy,
                ],
                StarKind::Mesh => vec![],
                StarKind::App => vec![ResourceType::App],
                StarKind::Actor => vec![ResourceType::Actor],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![ResourceType::Actor],
                StarKind::Web => vec![],
                StarKind::FileStore => vec![ResourceType::FileSystem, ResourceType::File],
                StarKind::ArtifactStore => {
                    vec![
                        ResourceType::ArtifactBundleVersions,
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
        if let StarKind::Actor = self {
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
            StarKind::Actor => true,
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

impl fmt::Display for ActorLookup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ActorLookup::Key(entity) => format!("Key({})", entity.to_string()).to_string(),
        };
        write!(f, "{}", r)
    }
}

pub static MAX_HOPS: usize = 32;

pub struct Star {
    skel: StarSkel,
    star_rx: mpsc::Receiver<StarCommand>,
    core_tx: mpsc::Sender<CoreMessageCall>,
    variant: Box<dyn StarVariant>,
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
        variant: Box<dyn StarVariant>,
    ) -> Self {
        let (status_broadcast, _) = broadcast::channel(8);
        Star {
            skel: data,
            variant: variant,
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
            let mut futures = vec![];
            let mut lanes = vec![];
            for (key, lane) in &mut self.lanes {
                futures.push(lane.incoming().recv().boxed());
                lanes.push(key.clone())
            }
            let mut proto_lane_index = vec![];

            for (index, lane) in &mut self.proto_lanes.iter_mut().enumerate() {
                futures.push(lane.incoming().recv().boxed());
                proto_lane_index.push(index);
            }

            futures.push(self.star_rx.recv().boxed());

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

            let mut lane = if future_index < lanes.len() {
                Option::Some(
                    self.lanes
                        .get_mut(lanes.get(future_index).as_ref().unwrap())
                        .expect("expected to get lane"),
                )
            } else if future_index < lanes.len() + proto_lane_index.len() {
                Option::Some(
                    self.proto_lanes
                        .get_mut(future_index - lanes.len())
                        .expect("expected to get proto_lane"),
                )
            } else {
                Option::None
            };

            if let Some(command) = command {
                let instructions = self.variant.filter(&command, &mut lane);

                if let StarShellInstructions::Handle = instructions {
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
                        StarCommand::AddProtoLaneEndpoint(lane) => {
                            let _result =
                                lane.outgoing
                                    .out_tx
                                    .try_send(LaneCommand::Frame(Frame::Proto(
                                        ProtoFrame::ReportStarKey(self.skel.info.key.clone()),
                                    )));
                            self.proto_lanes
                                .push(LaneWrapper::Proto(LaneMeta::new(lane)));
                        }
                        StarCommand::AddLaneEndpoint(lane) => {
                            self.lanes.insert(
                                lane.remote_star.clone(),
                                LaneWrapper::Lane(LaneMeta::new(lane)),
                            );
                        }
                        StarCommand::AddConnectorController(connector_ctrl) => {
                            self.connector_ctrls.push(connector_ctrl);
                        }
                        StarCommand::ReleaseHold(star) => {
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

                        StarCommand::Frame(frame) => {
                            if let Frame::Close = frame {
                                match lane_index {
                                    LaneIndex::None => {}
                                    LaneIndex::Lane(key) => {
                                        self.lanes.remove(&key);
                                        self.on_lane_closed(&key).await;
                                    }
                                    LaneIndex::ProtoLane(index) => {
                                        self.proto_lanes.remove(index);
                                    }
                                }
                            } else if let Frame::Proto(ProtoFrame::ReportStarKey(remote_star)) =
                                frame
                            {
                                match lane_index.expect_proto_lane() {
                                    Ok(proto_lane_index) => {
                                        let mut lane = self
                                            .proto_lanes
                                            .remove(proto_lane_index)
                                            .expect_proto_lane()
                                            .unwrap();
                                        lane.remote_star = Option::Some(remote_star);
                                        let lane = match lane.try_into() {
                                            Ok(lane) => lane,
                                            Err(error) => {
                                                error!(
                                                    "error converting proto_lane into lane: {}",
                                                    error
                                                );
                                                continue;
                                            }
                                        };
                                        self.skel
                                            .star_tx
                                            .send(StarCommand::AddLaneEndpoint(lane))
                                            .await;
                                    }
                                    Err(err) => {
                                        error!("{}", err)
                                    }
                                }
                            } else if let Frame::SearchTraversal(traversal) = &frame {
                                self.skel.star_search_api.on_traversal(traversal.clone(), lane.unwrap().get_remote_star().unwrap() );
                            } else {
                                if lane_index.is_lane() {
                                    self.process_frame(
                                        frame,
                                        Option::Some(&lane_index.expect_lane().unwrap()),
                                    )
                                    .await;
                                }
                            }




                        }
                        StarCommand::ForwardFrame(forward) => {
                            self.send_frame(forward.to.clone(), forward.frame).await;
                        }
                        StarCommand::CheckStatus => {
                            self.check_status().await;
                        }
                        StarCommand::SetStatus(status) => {
                            self.set_status(status.clone());
                            //                            println!("{} {}", &self.skel.info.kind, &self.status.to_string());
                        }
                        StarCommand::GetCaches(tx) => {
                            //                            tx.send(self.skel.caches.clone());
                            unimplemented!()
                        }
                        StarCommand::Diagnose(diagnose) => {
                            self.diagnose(diagnose).await;
                        }
                        StarCommand::GetStatusListener(tx) => {
                            tx.send(self.status_broadcast.subscribe());
                            self.status_broadcast.send(self.status.clone());
                        }
                        StarCommand::GetLaneForStar { star, tx } => {
//                            self.find_lane_for_star(star, tx).await;
                        }
                        StarCommand::GetSkel(tx) => {
                            tx.send(self.skel.clone()).unwrap_or_default();
                        }
                        StarCommand::Broadcast { frame, exclude } =>{
                            self.broadcast_excluding(frame,&exclude).await;
                        }
                        StarCommand::LaneKeys(tx) => {
                            let mut keys = vec!();
                            for (k,_) in &self.lanes {
                                keys.push(k.clone());
                            }
                            tx.send(keys);
                        }

                        StarCommand::LaneWithShortestPathToStar { star, tx } => {

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
                            eprintln!("cannot process command: {}", command.to_string());
                        }
                    }
                }
            } else {
                println!("command_rx has been disconnected");
                return;
            }
        }
    }

    async fn init(&mut self) {
        self.refresh_handles().await;
        self.check_status().await;
    }

    fn set_status(&mut self, status: StarStatus) {
        self.status = status.clone();
        self.status_broadcast.send(status);
    }

    async fn refresh_handles(&mut self) {
        if self.status == StarStatus::Unknown {
            self.set_status(StarStatus::Pending)
        }

        if let Option::Some(star_handler) = &self.skel.star_handler {
            for kind in self.skel.info.kind.distributes_to() {
                let search = SearchInit::new(StarPattern::StarKind(kind.clone()), TraversalAction::SearchHits);
                let (tx,rx) = oneshot::channel();
                self.skel.star_search_api.tx.try_send(StarSearchCall::Search {init:search, tx} ).unwrap_or_default();
                let star_handler = star_handler.clone();
                let kind = kind.clone();
                let skel = self.skel.clone();
                tokio::spawn(async move {
                    let result = tokio::time::timeout(Duration::from_secs(15), rx).await;
                    match result {
                        Ok(Ok(hits)) => {
                            for (star, hops) in hits.hits {
                                let handle = StarHandle {
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
                    .satisfied(self.skel.info.kind.distributes_to())
                    .await;
                if let Result::Ok(Satisfaction::Ok) = satisfied {
                    self.set_status(StarStatus::Initializing);
                    let (tx, rx) = oneshot::channel();
                    self.variant.init(tx);
                    let star_tx = self.skel.star_tx.clone();
                    tokio::spawn(async move {
                        // don't really have a mechanism to panic if init fails ... need to add that
                        rx.await.unwrap();
                        star_tx
                            .send(StarCommand::SetStatus(StarStatus::Ready))
                            .await;
                    });
                } else if let Result::Ok(Satisfaction::Lacking(lacking)) = satisfied {
                    let mut s = String::new();
                    for lack in lacking {
                        s.push_str(lack.to_string().as_str());
                        s.push_str(", ");
                    }
                    //                    eprintln!("handles not satisfied for : {} Lacking: [ {}]", self.skel.info.kind, s);
                }
            } else {
                self.set_status(StarStatus::Initializing);
                let (tx, rx) = oneshot::channel();
                self.variant.init(tx);
                let star_tx = self.skel.star_tx.clone();
                tokio::spawn(async move {
                    rx.await;
                    star_tx
                        .send(StarCommand::SetStatus(StarStatus::Ready))
                        .await;
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

    /*
        async fn send_resource_message( &mut self, mut builder: ProtoMessage)
        {

            if let Err(errors) = builder.validate() {
                eprintln!("resource message is not valid cannot send: {}", errors);
                return;
            }

            let tx = builder.sender();
            let message = if let Ok(message) = builder.build()
            {
                message
            } else {
                eprintln!("errors when trying to extract resource message builder...");
                return;
            };

            let (request,rx) = Request::new(message.to.key.clone().into());
            self.skel.star_tx.send( StarCommand::ResourceRecordRequest(request)).await;
            let skel = self.skel.clone();

            tokio::spawn( async move {
                match Star::wait_for_it(rx).await{
                    Ok(result) => {
                        let mut proto = ProtoStarMessage::new();
                        proto.to(result.location.host.clone().into());
                        proto.payload = StarMessagePayload::ResourceRequestMessage(message);
    println!("SEND PROTO MESSAGE FOR RESOURCE MESSAGE....");
                        let result = proto.get_ok_result().await;
                        skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
                        match util::wait_for_it_whatever(result).await
                        {
                            Ok(result) => {
                                println!("WHAT WEVE BEEN WAITING FOR RESULT: {}",result );
                            }
                            Err(error) => {
                                println!("Resource Message response Error: {}",error );
                            }
                        }
                    }
                    Err(fail) => {
                        eprintln!("Star failed to find resource record: {}", fail.to_string() );
                    }
                }

            } );

        }

         */


    pub fn star_key(&self) -> &StarKey {
        &self.skel.info.key
    }

    pub fn star_tx(&self) -> mpsc::Sender<StarCommand> {
        self.skel.star_tx.clone()
    }

    pub fn surface_api(&self) -> SurfaceApi {
        self.skel.surface_api.clone()
    }

    async fn broadcast(&mut self, frame: Frame) {
        self.broadcast_excluding(frame, &Option::None).await;
    }

    async fn broadcast_excluding(&mut self, frame: Frame, exclude: &Option<HashSet<StarKey>>) {
        let mut stars = vec![];
        for star in self.lanes.keys() {
            if exclude.is_none() || !exclude.as_ref().unwrap().contains(star) {
                stars.push(star.clone());
            }
        }
        for star in stars {
            self.send_frame(star, frame.clone()).await;
        }
    }

    /*    async fn message(&mut self, delivery: StarMessageDeliveryInsurance) {
           let message = delivery.message.clone();
           if !delivery.message.payload.is_ack() {
               let tracker = MessageReplyTracker {
                   reply_to: delivery.message.id.clone(),
                   tx: delivery.tx.clone(),
               };

               self.message_reply_trackers
                   .insert(delivery.message.id.clone(), tracker);

               let star_tx = self.skel.star_tx.clone();
               tokio::spawn(async move {
                   let mut delivery = delivery;
                   delivery.retries = delivery.expect.retries();

                   loop {
                       let wait = if delivery.retries == 0 && delivery.expect.retry_forever() {
                           // take a 2 minute break if retry_forever to be sure that all messages have expired
                           120 as u64
                       } else {
                           delivery.expect.wait_seconds()
                       };
                       let result = tokio::time::timeout(Duration::from_secs(wait), delivery.rx.recv()).await;
                       match result {
                           Ok(result) => {
                               match result {
                                   Ok(update) => {
                                       match update {
                                           MessageUpdate::Result(_) => {
                                               // the result will have been captured on another
                                               // rx as this is a broadcast.  no longer need to wait.
                                               break;
                                           }
                                           _ => {}
                                       }
                                   }
                                   Err(_) => {
                                       // probably the TX got dropped. no point in sticking around.
                                       break;
                                   }
                               }
                           }
                           Err(_elapsed) => {
                               delivery.retries = delivery.retries - 1;
                               if delivery.retries == 0 {
                                   if delivery.expect.retry_forever() {
                                       // we have to keep trying with a new message Id since the old one is now expired
                                       let proto = delivery.message.resubmit(
                                           delivery.expect,
                                           delivery.tx.clone(),
                                           delivery.tx.subscribe(),
                                       );
                                       star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                       break;
                                   } else {
                                       // out of retries, this
                                       delivery
                                           .tx
                                           .send(MessageUpdate::Result(MessageResult::Timeout));
                                       break;
                                   }
                               } else {
                                   // we resend the message and hope it arrives this time
                                   star_tx
                                       .send(StarCommand::ForwardFrame(ForwardFrame {
                                           to: delivery.message.to.clone(),
                                           frame: Frame::StarMessage(delivery.message.clone()),
                                       }))
                                       .await;
                               }
                           }
                       }
                   }
               });
           }
           if message.to != self.skel.info.key {
               self.send_frame(message.to.clone(), Frame::StarMessage(message))
                   .await;
           } else {
               // a special exception for sending a message to ourselves
               self.process_frame(Frame::StarMessage(message), Option::None)
                   .await;
           }
       }

    */


    async fn send_frame(&mut self, lane_key: LaneKey, frame: Frame) {

        if let Option::Some(lane) = self.lanes.get_mut(&lane_key) {
            lane.outgoing().out_tx.send( LaneCommand::Frame(frame)).await;
        } else {
error!("dropped frame could not find laneKey: {}",lane_key.to_string() );
        }
        /*
        if let Option::Some(lane) = lane {
            lane.outgoing().out_tx.send(LaneCommand::Frame(frame)).await;
        } else {
            self.frame_hold.add(&star, frame);
            let (tx, rx) = oneshot::channel();

            self.search_for_star(star.clone(), tx).await;
            let command_tx = self.skel.star_tx.clone();
            tokio::spawn(async move {
                match rx.await {
                    Ok(_) => {
                        command_tx.send(StarCommand::ReleaseHold(star)).await;
                    }
                    Err(error) => {
                        eprintln!("RELEASE HOLD RX ERROR : {}", error);
                    }
                }
            });
        }
         */
    }

    async fn lane_with_shortest_path_to_star(&mut self, star: &StarKey) -> Option<&mut LaneWrapper> {

        self.skel.golden_path_api.golden_lane_leading_to_star(star.clone() ).await;



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

    /*
    async fn search( &mut self, pattern: StarSearchPattern )->Result<StarSearchCompletion,Canceled>
    {
        let search_id = self.info.sequence.next();
        let (search_transaction,rx) = StarSearchTransaction::new(StarSearchPattern::StarKey(self.info.star_key.clone()));

        self.star_search_transactions.insert(search_id, search_transaction );

        let search = StarSearchInner{
            from: self.info.star_key.clone(),
            pattern: pattern,
            hops: vec![self.star_key.clone()],
            transactions: vec![search_id],
            max_hops: MAX_HOPS
        };

        self.broadcast(Frame::StarSearch(search) ).await;

        rx.await
    }

     */

    /*
    async fn search_for_star( &mut self, star: StarKey )
    {

        let search_id = self.transaction_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed );
        let (search_transaction,_) = StarSearchTransaction::new(StarSearchPattern::StarKey(self.star_key.clone()));
        self.star_search_transactions.insert(search_id, search_transaction );

        let search = StarSearchInner{
            from: self.star_key.clone(),
            pattern: StarSearchPattern::StarKey(star),
            hops: vec![self.star_key.clone()],
            transactions: vec![search_id],
            max_hops: MAX_HOPS,
        };

        self.logger.log2(StarLog::StarSearchInitialized(search.clone()));
        for (star,lane) in &self.lanes
        {
            lane.lane.outgoing.tx.send( LaneCommand::Frame( Frame::StarSearch(search.clone()))).await;
        }
    }*/

    async fn on_wind_down(&mut self, _search_result: SearchWindDown, _lane_key: StarKey) {
        //        println!("ON STAR SEARCH RESULTS");
    }
    /*
    async fn on_star_search_result( &mut self, mut search_result: StarSearchResultInner, lane_key: StarKey )
    {

        self.logger.log2(StarLog::StarSearchResult(search_result.clone()));
        if let Some(search_id) = search_result.transactions.last()
        {
            if let Some(search_trans) = self.star_search_transactions.get_mut(search_id)
            {
                for hit in &search_result.hits
                {
                    search_trans.hits.insert( hit.star.clone(), hit.clone() );
                    let lane = self.lanes.get_mut(&lane_key).unwrap();
                    lane.star_paths.insert( hit.star.clone(), hit.hops.clone() as _ );
                    if let Some(frames) = self.frame_hold.release( &hit.star )
                    {
                        for frame in frames
                        {
                            lane.lane.outgoing.tx.send( LaneCommand::Frame(frame) ).await;
                        }
                    }
                }
                search_trans.reported_lane_count = search_trans.reported_lane_count+1;

                if search_trans.reported_lane_count >= (self.lanes.len() as i32)-1
                {
                    // this means all lanes have been searched and the search result can be reported to the next node
                    if let Some(search_trans) = self.star_search_transactions.remove(search_id)
                    {
                        search_result.pop();
                        if let Some(next)=search_result.hops.last()
                        {
                            if let Some(lane)=self.lanes.get_mut(next)
                            {
                                search_result.hits = search_trans.hits.values().map(|a|a.clone()).collect();
                                lane.lane.outgoing.tx.send( LaneCommand::Frame(Frame::StarSearchResult(search_result))).await;
                            }
                        }

                        search_trans.complete();
                    }
                }
            }
        }
    }
     */



    async fn on_lane_closed(&mut self, key: &StarKey) {
        // we should notify any waiting WIND transactions that this lane is no longer participating
        /*
        let mut remove = HashSet::new();
        for (tid, transaction) in self.transactions.iter_mut() {
            if let TransactionResult::Done = transaction.on_lane_closed(key).await {
                remove.insert(tid.clone());
            }
        }

        self.transactions.retain(|k, _| !remove.contains(k));
        */
    }

    async fn process_message_reply(&mut self, message: &StarMessage) {
        if message.reply_to.is_some()
            && self
                .message_reply_trackers
                .contains_key(message.reply_to.as_ref().unwrap())
        {
            if let Some(tracker) = self
                .message_reply_trackers
                .get(message.reply_to.as_ref().unwrap())
            {
                if let TrackerJob::Done = tracker.on_message(message) {
                    self.message_reply_trackers
                        .remove(message.reply_to.as_ref().unwrap());
                }
            }
        }
    }

    async fn process_frame(&mut self, frame: Frame, lane_key: Option<&StarKey>) {
//        self.process_transactions(&frame, lane_key).await;
        match frame {
            Frame::SearchTraversal(traversal) => {
                self.skel.star_search_api.on_traversal(traversal, lane_key.expect("Expected a LaneKey").clone() );
            },
            Frame::StarMessage(message) => {
                self.skel.router_api.route(message).unwrap_or_default();
            }
            /*            Frame::StarMessage(message) => match self.on_message(message).await {
                           Ok(_messages) => {}
                           Err(error) => {
                               error!("X error: {}", error)
                           }
                       },
            */
            _ => {
                error!("star does not handle frame: {}", frame)
            }
        }
    }

    /*
        async fn on_message(&mut self, message: StarMessage) -> Result<(), Error> {

    println!("STAR ON MESSAGE");

            if message.log {
                info!(
                    "{} => {} : {}",
                    self.skel.info.to_string(),
                    LogId(&message).to_string(),
                    "on_message"
                );
            }
            if message.to != self.skel.info.key {
                if self.skel.info.kind.relay() || message.from == self.skel.info.key {
                    //forward the message
                    self.send_frame(message.to.clone(), Frame::StarMessage(message))
                        .await;
                    return Ok(());
                } else {
                    error!("this star does not relay Messages");
                    return Err(
                        format!("this star {} does not relay Messages", self.skel.info.kind.to_string()).into(),
                    );
                }
            } else {
    println!("Star on_message() -> message.reply_to.is_some(): {}", message.reply_to.is_some() );
                if message.reply_to.is_some() {
                    self.skel.messaging_api.on_reply(message);
                } else {
                    self.skel.core_messaging_endpoint_tx.try_send(CoreMessageCall::Message(message)).unwrap_or_default();
                }
                Ok(())
            }
        }
         */

    async fn diagnose(&self, diagnose: Diagnose) {
        match diagnose {
            Diagnose::HandlersSatisfied(satisfied) => {
                if let Option::Some(star_handler) = &self.skel.star_handler {
                    if let Result::Ok(satisfaction) = star_handler
                        .satisfied(self.skel.info.kind.distributes_to())
                        .await
                    {
                        satisfied.tx.send(satisfaction);
                    } else {
                        // let satisfied.tx drop since we can't give it an answer
                    }
                } else {
                    satisfied.tx.send(Satisfaction::Ok);
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
    AddLaneEndpoint(LaneEndpoint),
    AddProtoLaneEndpoint(ProtoLaneEndpoint),
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
        tx: oneshot::Sender<Result<LaneKey, Error>>,
    },
    Shutdown,
    GetSkel(oneshot::Sender<StarSkel>),
    Broadcast { frame: Frame, exclude: Option<HashSet<LaneKey>> },
    LaneKeys(oneshot::Sender<Vec<LaneKey>>),
    LaneWithShortestPathToStar { star: StarKey, tx: oneshot::Sender<Option<LaneKey>> }
}

#[derive(Clone)]
pub enum ConstellationBroadcast {
    Status(ConstellationStatus),
}

pub enum Diagnose {
    HandlersSatisfied(YesNo<Satisfaction>),
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
    pub tx: oneshot::Sender<Result<R, Fail>>,
    pub log: bool,
}

impl<P: Debug, R> Debug for Request<P, R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.payload.fmt(f)
    }
}

impl<P: Debug, R> Request<P, R> {
    pub fn new(payload: P) -> (Self, oneshot::Receiver<Result<R, Fail>>) {
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

    pub async fn diagnose_handlers_satisfaction(&self) -> Result<Satisfaction, Error> {
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
    pub lanes_api: LanesApi,
    pub flags: Flags,
    pub logger: Logger,
    pub sequence: Arc<AtomicU64>,
    pub registry: Option<Arc<dyn ResourceRegistryBacking>>,
    pub star_handler: Option<StarHandleBacking>,
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
    ) -> Result<RegistryReservation, Fail>;
    async fn register(&self, registration: ResourceRegistration) -> Result<(), Fail>;
    async fn select(&self, select: ResourceSelector) -> Result<Vec<ResourceRecord>, Fail>;
    async fn set_location(&self, location: ResourceRecord) -> Result<(), Fail>;
    async fn get(&self, identifier: ResourceIdentifier) -> Result<Option<ResourceRecord>, Fail>;
    async fn unique_src(&self, key: ResourceIdentifier) -> Box<dyn UniqueSrc>;
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

    async fn timeout<X>(rx: oneshot::Receiver<X>) -> Result<X, Fail> {
        Ok(tokio::time::timeout(Duration::from_secs(25), rx).await??)
    }
}

#[async_trait]
impl ResourceRegistryBacking for ResourceRegistryBackingSqLite {
    async fn reserve(
        &self,
        request: ResourceNamesReservationRequest,
    ) -> Result<RegistryReservation, Fail> {
        let (action, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Reserve(request));
        self.registry.send(action).await?;

        match Self::timeout(rx).await? {
            ResourceRegistryResult::Reservation(reservation) => Result::Ok(reservation),
            _ => Result::Err(Fail::expected("ResourceRegistryResult::Reservation(_)")),
        }

        /*        match tokio::time::timeout(Duration::from_secs(5), rx).await?? {
                   ResourceRegistryResult::Reservation(reservation) => Result::Ok(reservation),
                   _ => Result::Err(Fail::Timeout),
               }
        */
    }

    async fn register(&self, registration: ResourceRegistration) -> Result<(), Fail> {
        let (request, rx) =
            ResourceRegistryAction::new(ResourceRegistryCommand::Commit(registration));
        self.registry.send(request).await?;
        //        tokio::time::timeout(Duration::from_secs(5), rx).await??;
        Self::timeout(rx).await?;
        Ok(())
    }

    async fn select(&self, selector: ResourceSelector) -> Result<Vec<ResourceRecord>, Fail> {
        let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
        self.registry.send(request).await?;
        // match tokio::time::timeout(Duration::from_secs(5), rx).await?? {
        match Self::timeout(rx).await? {
            ResourceRegistryResult::Resources(resources) => Result::Ok(resources),
            _ => Result::Err(Fail::Timeout),
        }
    }

    async fn set_location(&self, location: ResourceRecord) -> Result<(), Fail> {
        let (request, rx) =
            ResourceRegistryAction::new(ResourceRegistryCommand::SetLocation(location));
        self.registry.send(request).await;
        //tokio::time::timeout(Duration::from_secs(5), rx).await??;
        Self::timeout(rx).await?;
        Ok(())
    }

    async fn get(&self, identifier: ResourceIdentifier) -> Result<Option<ResourceRecord>, Fail> {
        let (request, rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Get(identifier));
        self.registry.send(request).await;
        //match tokio::time::timeout(Duration::from_secs(5), rx).await?? {
        match Self::timeout(rx).await? {
            ResourceRegistryResult::Resource(resource) => {
                Ok(resource)
            }
            _ => Err(Fail::expected("ResourceRegistryResult::Resource(_)")),
        }
    }

    async fn unique_src(&self, id: ResourceIdentifier) -> Box<dyn UniqueSrc> {
        Box::new(RegistryUniqueSrc::new(id, self.registry.clone()))
    }
}

#[derive(Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum StarStatus {
    Unknown,
    Pending,
    Initializing,
    Ready,
    Panic,
}

impl ToString for StarStatus {
    fn to_string(&self) -> String {
        match self {
            StarStatus::Unknown => "Unknown".to_string(),
            StarStatus::Pending => "Pending".to_string(),
            StarStatus::Ready => "Ready".to_string(),
            StarStatus::Panic => "Panic".to_string(),
            StarStatus::Initializing => "Initializing".to_string(),
        }
    }
}

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
