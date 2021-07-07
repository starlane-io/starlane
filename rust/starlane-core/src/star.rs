use std::borrow::Borrow;
use std::cell::Cell;
use std::cmp::{min, Ordering};
use std::collections::hash_map::RandomState;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;
use std::future::Future;
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU64};
use std::sync::Arc;
use std::{cmp, fmt};

use futures::future::select_all;
use futures::future::{join_all, BoxFuture, Map};
use futures::prelude::future::FusedFuture;
use futures::FutureExt;
use lru::LruCache;
use serde::de::Unexpected;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::error::{RecvError, SendError};
use tokio::sync::mpsc;
use tokio::sync::oneshot::Sender;
use tokio::sync::{broadcast, oneshot};
use tokio::time::error::Elapsed;
use tokio::time::{timeout, Duration, Instant};
use url::Url;

use variant::StarVariant;

use crate::actor::{ActorKey, ActorKind};
use crate::cache::ProtoArtifactCachesFactory;
use crate::core::{StarCoreAction, StarCoreCommand, StarCoreResult};
use crate::crypt::{Encrypted, HashEncrypted, HashId, PublicKey, UniqueHash};
use crate::error::Error;
use crate::file_access::FileAccess;
use crate::frame::WindAction::SearchHits;
use crate::frame::{
    ActorLookup, ChildManagerResourceAction, Event, Frame, FromReply, MessagePayload, ProtoFrame,
    Reply, ResourceHostAction, SimpleReply, SpaceMessage, StarMessage, StarMessagePayload,
    StarPattern, StarWind, Watch, WatchInfo, WindAction, WindDown, WindHit, WindResults, WindUp,
};
use crate::id::{Id, IdSeq};
use crate::keys::{
    AppKey, GatheringKey, MessageId, ResourceId, ResourceKey, SpaceKey, Unique, UniqueSrc, UserKey,
};
use crate::lane::{ConnectionInfo, ConnectorController, LaneEndpoint, LaneCommand, LaneMeta, OutgoingSide, TunnelConnector, TunnelConnectorFactory, ProtoLaneEndpoint, LaneIndex, LaneWrapper};
use crate::logger::{
    Flag, Flags, Log, LogInfo, Logger, ProtoStarLog, ProtoStarLogPayload, StarFlag, StaticLogInfo,
};
use crate::message::resource::{
    Delivery, Message, ProtoMessage, ResourceRequestMessage, ResourceResponseMessage,
};
use crate::message::{
    Fail, MessageExpect, MessageExpectWait, MessageReplyTracker, MessageResult, MessageUpdate,
    ProtoStarMessage, ProtoStarMessageTo, StarMessageDeliveryInsurance, TrackerJob,
};
use crate::permissions::{AuthToken, AuthTokenSource, Authentication, Credentials};
use crate::proto::{PlaceholderKernel, ProtoStar, ProtoTunnel};
use crate::resource::space::SpaceState;
use crate::resource::sub_space::SubSpaceState;
use crate::resource::user::UserState;
use crate::resource::{
    AddressCreationSrc, AssignResourceStateSrc, FieldSelection, HostedResourceStore,
    KeyCreationSrc, Labels, LocalDataSrc, LocalHostedResource, LocalResourceHost,
    MemoryDataTransfer, Parent, ParentCore, Registry, RegistryReservation, RegistryUniqueSrc,
    RemoteResourceManager, Resource, ResourceAddress, ResourceArchetype, ResourceAssign,
    ResourceBinding, ResourceCreate, ResourceHost, ResourceIdentifier, ResourceKind,
    ResourceLocation, ResourceManager, ResourceManagerKey, ResourceNamesReservationRequest,
    ResourceParent, ResourceRecord, ResourceRegistration, ResourceRegistryAction,
    ResourceRegistryCommand, ResourceRegistryInfo, ResourceRegistryResult, ResourceSelector,
    ResourceStateSrc, ResourceStub, ResourceType,
};
use crate::star::pledge::{ResourceHostSelector, Satisfaction, StarHandle, StarHandleBacking};
use crate::star::variant::web::WebVariant;
use crate::util;
use crate::util::AsyncHashMap;
use crate::template::StarTemplateHandle;
use std::fmt::{Debug, Formatter};
use tracing::field::{Field, Visit};
use crate::constellation::ConstellationStatus;
use crate::star::variant::StarShellInstructions;

pub mod filestore;
pub mod pledge;
pub mod variant;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize, Hash)]
pub enum StarKind {
    Central,
    SpaceHost,
    Mesh,
    AppHost,
    ActorHost,
    FileStore,
    ArtifactStore,
    Gateway,
    Link,
    Client,
    Web,
    Kube,
}

impl StarKind {
    pub fn is_resource_manager(&self) -> bool {
        match self {
            StarKind::Central => true,
            StarKind::SpaceHost => true,
            StarKind::Mesh => false,
            StarKind::AppHost => true,
            StarKind::ActorHost => false,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => false,
            StarKind::Web => false,
            StarKind::ArtifactStore => true,
            StarKind::Kube => true,
        }
    }

    pub fn is_resource_host(&self) -> bool {
        match self {
            StarKind::Central => false,
            StarKind::SpaceHost => true,
            StarKind::Mesh => false,
            StarKind::AppHost => true,
            StarKind::ActorHost => true,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => true,
            StarKind::Web => true,
            StarKind::ArtifactStore => true,
            StarKind::Kube => true,
        }
    }

    pub fn handles(&self) -> HashSet<StarKind> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![StarKind::SpaceHost],
                StarKind::SpaceHost => {
                    vec![StarKind::FileStore, StarKind::Web, StarKind::ArtifactStore]
                }
                StarKind::Mesh => vec![],
                StarKind::AppHost => vec![StarKind::ActorHost, StarKind::FileStore],
                StarKind::ActorHost => vec![],
                StarKind::FileStore => vec![],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![],
                StarKind::Web => vec![],
                StarKind::ArtifactStore => vec![],
                StarKind::Kube => vec![],
            }
            .iter()
            .cloned(),
        )
    }

    pub fn manages(&self) -> HashSet<ResourceType> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![ResourceType::Space],
                StarKind::SpaceHost => vec![
                    ResourceType::SubSpace,
                    ResourceType::App,
                    ResourceType::FileSystem,
                    ResourceType::Proxy,
                    ResourceType::Database,
                ],
                StarKind::Mesh => vec![],
                StarKind::AppHost => vec![
                    ResourceType::Actor,
                    ResourceType::FileSystem,
                    ResourceType::Database,
                ],
                StarKind::ActorHost => vec![],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![],
                StarKind::Web => vec![ResourceType::Domain, ResourceType::UrlPathPattern],
                StarKind::FileStore => vec![ResourceType::File],
                StarKind::ArtifactStore => vec![ResourceType::Artifact],
                StarKind::Kube => vec![ResourceType::Database],
            }
            .iter()
            .cloned(),
        )
    }

    pub fn hosts(&self) -> HashSet<ResourceType> {
        HashSet::from_iter(
            match self {
                StarKind::Central => vec![ResourceType::Root],
                StarKind::SpaceHost => vec![
                    ResourceType::Space,
                    ResourceType::SubSpace,
                    ResourceType::User,
                    ResourceType::Domain,
                    ResourceType::UrlPathPattern,
                    ResourceType::Proxy,
                ],
                StarKind::Mesh => vec![],
                StarKind::AppHost => vec![ResourceType::App],
                StarKind::ActorHost => vec![ResourceType::Actor],
                StarKind::Gateway => vec![],
                StarKind::Link => vec![],
                StarKind::Client => vec![ResourceType::Actor],
                StarKind::Web => vec![],
                StarKind::FileStore => vec![ResourceType::FileSystem, ResourceType::File],
                StarKind::ArtifactStore => {
                    vec![ResourceType::ArtifactBundle, ResourceType::Artifact]
                }
                StarKind::Kube => vec![ResourceType::Database],
            }
            .iter()
            .cloned(),
        )
    }
}

impl FromStr for StarKind {
    type Err = ();

    fn from_str(input: &str) -> Result<StarKind, Self::Err> {
        match input {
            "Central" => Ok(StarKind::Central),
            "Mesh" => Ok(StarKind::Mesh),
            "AppHost" => Ok(StarKind::AppHost),
            "ActorHost" => Ok(StarKind::ActorHost),
            "Gateway" => Ok(StarKind::Gateway),
            "Link" => Ok(StarKind::Link),
            "Client" => Ok(StarKind::Client),
            "SpaceHost" => Ok(StarKind::SpaceHost),
            "Web" => Ok(StarKind::Web),
            "FileStore" => Ok(StarKind::FileStore),
            "ArtifactStore" => Ok(StarKind::ArtifactStore),
            "Database" => Ok(StarKind::Kube),
            _ => Err(()),
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
        if let StarKind::AppHost = self {
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
        if let StarKind::AppHost = self {
            Ok(())
        } else {
            Err("not supervisor".into())
        }
    }

    pub fn server_result(&self) -> Result<(), Error> {
        if let StarKind::ActorHost = self {
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
            StarKind::AppHost => false,
            StarKind::ActorHost => true,
            StarKind::Gateway => true,
            StarKind::Client => true,
            StarKind::Link => true,
            StarKind::SpaceHost => false,
            StarKind::Web => false,
            StarKind::FileStore => false,
            StarKind::ArtifactStore => false,
            StarKind::Kube => false,
        }
    }
}

impl fmt::Display for StarKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                StarKind::Central => "Central".to_string(),
                StarKind::Mesh => "Mesh".to_string(),
                StarKind::AppHost => "AppHost".to_string(),
                StarKind::ActorHost => "ActorHost".to_string(),
                StarKind::Gateway => "Gateway".to_string(),
                StarKind::Link => "Link".to_string(),
                StarKind::Client => "Client".to_string(),
                StarKind::SpaceHost => "SpaceHost".to_string(),
                StarKind::Web => "Web".to_string(),
                StarKind::FileStore => "FileStore".to_string(),
                StarKind::ArtifactStore => "ArtifactStore".to_string(),
                StarKind::Kube => "Database".to_string(),
            }
        )
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
    core_tx: mpsc::Sender<StarCoreAction>,
    variant: Box<dyn StarVariant>,
    lanes: HashMap<StarKey, LaneWrapper>,
    proto_lanes: Vec<LaneWrapper>,
    connector_ctrls: Vec<ConnectorController>,
    transactions: HashMap<u64, Box<dyn Transaction>>,
    frame_hold: FrameHold,
    watches: HashMap<ActorKey, HashMap<Id, StarWatchInfo>>,
    messages_received: HashMap<MessageId, Instant>,
    message_reply_trackers: HashMap<MessageId, MessageReplyTracker>,
    star_subgraph_expansion_seq: AtomicU64,
    resource_record_cache: LruCache<ResourceKey, ResourceRecord>,
    resource_address_to_key: LruCache<ResourceAddress, ResourceKey>,
    status: StarStatus,
    status_broadcast: broadcast::Sender<StarStatus>
}

impl Debug for Star{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(),std::fmt::Error> {
        f.write_str(self.skel.info.to_string().as_str() );
        Ok(())
    }
}

impl Star {
    pub async fn from_proto(
        data: StarSkel,
        star_rx: mpsc::Receiver<StarCommand>,
        core_tx: mpsc::Sender<StarCoreAction>,
        lanes: HashMap<StarKey, LaneWrapper>,
        proto_lanes: Vec<LaneWrapper>,
        connector_ctrls: Vec<ConnectorController>,
        frame_hold: FrameHold,
        variant: Box<dyn StarVariant>,
    ) -> Self {
        let (status_broadcast,_) = broadcast::channel(8);
        Star {
            skel: data,
            variant: variant,
            star_rx: star_rx,
            core_tx: core_tx,
            lanes: lanes,
            proto_lanes: proto_lanes,
            connector_ctrls: connector_ctrls,
            transactions: HashMap::new(),
            frame_hold: frame_hold,
            watches: HashMap::new(),
            messages_received: HashMap::new(),
            message_reply_trackers: HashMap::new(),
            star_subgraph_expansion_seq: AtomicU64::new(0),
            resource_record_cache: LruCache::new(16 * 1024),
            resource_address_to_key: LruCache::new(16 * 1024),
            status: StarStatus::Unknown,
            status_broadcast: status_broadcast
        }
    }

    pub fn info(&self) -> StarInfo {
        self.skel.info.clone()
    }

    pub fn has_resource_record(&mut self, identifier: &ResourceIdentifier) -> bool {
        match identifier {
            ResourceIdentifier::Key(key) => self.resource_record_cache.contains(key),
            ResourceIdentifier::Address(address) => {
                let key = self.resource_address_to_key.get(address);
                match key {
                    None => false,
                    Some(key) => self.resource_record_cache.contains(key),
                }
            }
        }
    }

    pub fn get_resource_record(
        &mut self,
        identifier: &ResourceIdentifier,
    ) -> Option<ResourceRecord> {
        match identifier {
            ResourceIdentifier::Key(key) => self.resource_record_cache.get(key).cloned(),
            ResourceIdentifier::Address(address) => {
                let key = self.resource_address_to_key.get(address);
                match key {
                    None => Option::None,
                    Some(key) => self.resource_record_cache.get(key).cloned(),
                }
            }
        }
    }

    pub async fn has_resource(&self, key: &ResourceKey) -> Result<bool, Fail> {
        Ok(self.get_resource(key).await?.is_some())
    }

    pub async fn get_resource(&self, key: &ResourceKey) -> Result<Option<Resource>, Fail> {
        let (action, rx) = StarCoreAction::new(StarCoreCommand::Get(key.clone().into()));
if self.skel.core_tx.is_closed() {
    error!("core_tx CLOSED");
}
        self.skel.core_tx.send(action).await?;
        match rx.await?? {
            StarCoreResult::Resource(has) => Ok(has),
            _ => Err("unexpected StarCoreResult".into()),
        }
    }

    /*
    pub async fn fetch_resource_stub(&mut self, key: ResourceStub) -> oneshot::Receiver<Result<ResourceAddress,Fail>>
    {
        let (tx,rx) = oneshot::channel();

        if self.resource_record_cache.contains(&key) {
            tx.send(Ok(self.resource_record_cache.get(&key).cloned().unwrap()) ).unwrap_or_default();
        } else {
            let skel = self.skel.clone();
            tokio::spawn( async move {
                let managing_star = match key.parent() {
                    None => {
                        // this must be a Space, meaning it's data is held in Central
                        StarKey::central()
                    }
                    Some(parent) => {
                        let (request,locate_parent_rx) = Request::new(parent);
                        skel.star_tx.send( StarCommand::ResourceRecordRequest(request)).await.unwrap_or_default();
                        if let Result::Ok(Result::Ok(record)) = locate_parent_rx.await {
                            record.location.host
                        } else {
                            tx.send(Err(Fail::Unexpected));
                            return;
                        }
                    }
                };

                // now request Address from the managing_star
                let mut proto = ProtoMessage::new();
                proto.to(managing_star);
                proto.payload = StarMessagePayload::ResourceManager(ChildResourceAction::GetAddress(key.clone()));
                let reply = proto.get_ok_result().await;
                skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
                let reply = reply.await;
                match reply{
                    Ok( StarMessagePayload::Reply(SimpleReply::Ok(Reply::Address(address)))) => {
                        skel.star_tx.send(StarCommand::SetResourceAddress{key:key.clone(),address:address.clone()} ).await;
                        tx.send(Ok(address));
                    }
                    Ok( StarMessagePayload::Reply(SimpleReply::Fail(fail))) => {
                        tx.send((Err(fail)));
                    }
                    _ => {
                        tx.send(Err(Fail::Unexpected));
                    }
                }
            } );

        }

        rx
    }

     */

    /*
    pub async fn fetch_resource_address(&mut self, key: ResourceStub) -> oneshot::Receiver<Result<ResourceAddress,Fail>>
    {
        let (tx,rx) = oneshot::channel();

        if self.resource_record_cache.contains(&key) {
            tx.send(Ok(self.resource_record_cache.get(&key).cloned().unwrap()) ).unwrap_or_default();
        } else {
            let skel = self.skel.clone();
            tokio::spawn( async move {
                let managing_star = match key.parent() {
                    None => {
                        // this must be a Space, meaning it's data is held in Central
                        StarKey::central()
                    }
                    Some(parent) => {
                        let (request,locate_parent_rx) = Request::new(parent);
                        skel.star_tx.send( StarCommand::ResourceRecordRequest(request)).await.unwrap_or_default();
                        if let Result::Ok(Result::Ok(record)) = locate_parent_rx.await {
                            record.location.host
                        } else {
                            tx.send(Err(Fail::Unexpected));
                            return;
                        }
                    }
                };

                // now request Address from the managing_star
                let mut proto = ProtoMessage::new();
                proto.to(managing_star);
                proto.payload = StarMessagePayload::ResourceManager(ChildResourceAction::GetAddress(key.clone()));
                let reply = proto.get_ok_result().await;
                skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
                let reply = reply.await;
                match reply{
                    Ok( StarMessagePayload::Reply(SimpleReply::Ok(Reply::Address(address)))) => {
                        skel.star_tx.send(StarCommand::SetResourceAddress{key:key.clone(),address:address.clone()} ).await;
                        tx.send(Ok(address));
                    }
                    Ok( StarMessagePayload::Reply(SimpleReply::Fail(fail))) => {
                        tx.send((Err(fail)));
                    }
                    _ => {
                        tx.send(Err(Fail::Unexpected));
                    }
                }
            } );

        }

        rx
    }

     */

    /*
        pub async fn fetch_resource_key( &mut self, address: ResourceAddress )-> oneshot::Receiver<Result<ResourceStub,Fail>>
        {
            let (tx,rx) = oneshot::channel();

            if self.resource_address_to_stub.contains(&address ) {
                tx.send(Ok(self.resource_address_to_stub.get(&address ).cloned().unwrap()) ).unwrap_or_default();
            } else {
                let skel = self.skel.clone();
                tokio::spawn( async move {
                    let managing_star = match address.parent() {
                        None => {
                            // this must be a Space, meaning it's data is held in Central
                            StarKey::central()
                        }
                        Some(parent) => {
                            let (request,locate_parent_rx) = Request::new(parent);
                            skel.star_tx.send( StarCommand::LookupResourceKeyByAddress(request)).await.unwrap_or_default();
                            if let Result::Ok(Result::Ok(key)) = locate_parent_rx.await {
                                tx.send( Ok(key) ).unwrap_or_default();
                            } else {
                                tx.send(Err(Fail::Unexpected)).unwrap_or_default();
                            }
                            return;
                        }
                    };

                    // now request ResourceKey from the managing_star
                    let mut proto = ProtoMessage::new();
                    proto.to(managing_star);
                    proto.payload = StarMessagePayload::ResourceManager(ChildResourceAction::GetKey(address.clone()));
    println!("SENDING GetKey({})",address.clone().to_string());
                    let reply = proto.get_ok_result().await;
                    skel.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
    println!("sent____");
                    let reply = reply.await;
    println!("GOT REPLY from GetKey");
                    match reply{
                        Ok( StarMessagePayload::Reply(SimpleReply::Ok(Reply::Key(key)))) => {

                            skel.star_tx.send(StarCommand::SetResourceAddress{key:key.clone(),address:address.clone()} ).await;
                            tx.send(Ok(key));
                        }
                        Ok( StarMessagePayload::Reply(SimpleReply::Fail(fail))) => {
                            tx.send((Err(fail)));
                        }
                        _ => {
                            tx.send(Err(Fail::Unexpected));
                        }
                    }
                } );

            }

            rx
        }
         */

    #[instrument]
    pub async fn run(mut self) {

        loop {
            let mut futures = vec![];
            let mut lanes = vec![];
            for (key, mut lane) in &mut self.lanes {
                futures.push(lane.incoming().recv().boxed());
                lanes.push(key.clone())
            }
            let mut proto_lane_index = vec![];

            for (index,lane) in &mut self.proto_lanes.iter_mut().enumerate() {
                futures.push(lane.incoming().recv().boxed());
                proto_lane_index.push(index);
            }

            futures.push(self.star_rx.recv().boxed());

            let (command, future_index, _) = select_all(futures).await;

            let lane_index = if future_index < lanes.len() {
                LaneIndex::Lane(lanes.get(future_index).expect("expected a lane at this index").clone())
            } else if future_index < lanes.len()+ proto_lane_index.len() {
                LaneIndex::ProtoLane(future_index-lanes.len())
            } else {
                LaneIndex::None
            };

            let mut lane = if future_index < lanes.len() {
                Option::Some(self.lanes.get_mut(lanes.get(future_index).as_ref().unwrap() ).expect("expected to get lane"))
            } else if future_index < lanes.len()+ proto_lane_index.len() {
                Option::Some(self.proto_lanes.get_mut( future_index-lanes.len()).expect("expected to get proto_lane"))
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
                            let result = lane.outgoing.out_tx.try_send( LaneCommand::Frame(Frame::Proto(ProtoFrame::ReportStarKey(self.skel.info.key.clone()))));
                            self.proto_lanes.push(LaneWrapper::Proto(LaneMeta::new(lane)));
                        }
                        StarCommand::AddLaneEndpoint(lane) => {
                            self.lanes.insert(lane.remote_star.clone(), LaneWrapper::Lane(LaneMeta::new(lane)));
                        }
                        StarCommand::AddConnectorController(connector_ctrl) => {
                            self.connector_ctrls.push(connector_ctrl);
                        }
                        StarCommand::SendProtoMessage(message) => {
                            self.send_proto_message(message).await;
                        }
                        StarCommand::ReleaseHold(star) => {
                            if let Option::Some(frames) = self.frame_hold.release(&star) {
                                let lane = self.lane_with_shortest_path_to_star(&star);
                                if let Option::Some(lane) = lane {
                                    for frame in frames {
                                        lane.outgoing().out_tx.send(LaneCommand::Frame(frame)).await;
                                    }
                                } else {
                                    eprintln!("release hold called on star that is not ready!")
                                }
                            }
                        }

                        StarCommand::AddLogger(tx) => {
                            //                        self.logger.tx.push(tx);
                        }
                        StarCommand::Test(test) => {
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
                        StarCommand::WindInit(search) => {
                            self.do_wind(search).await;
                        }
                        StarCommand::WindCommit(commit) => {
                            for lane in commit.result.lane_hits.keys() {
                                let hits = commit.result.lane_hits.get(lane).unwrap();
                                for (star, size) in hits {
                                    self.lanes
                                        .get_mut(lane)
                                        .unwrap()
                                        .star_paths()
                                        .put(star.clone(), size.clone());
                                }
                            }
                            commit.tx.send(commit.result);
                        }
                        StarCommand::WindDown(result) => {
                            let lane = result.hops.last().unwrap();
                            self.send_frame(lane.clone(), Frame::StarWind(StarWind::Down(result)))
                                .await;
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
                            } else if let Frame::Proto(ProtoFrame::ReportStarKey(remote_star)) = frame {
                                match lane_index.expect_proto_lane()
                                {
                                    Ok(proto_lane_index) => {
                                        let mut lane = self.proto_lanes.remove(proto_lane_index).expect_proto_lane().unwrap();
                                        lane.remote_star = Option::Some(remote_star);
                                        let lane = match lane.try_into() {
                                            Ok(lane) => {
                                                lane
                                            }
                                            Err(error) => {
                                                error!("error converting proto_lane into lane: {}", error);
                                                continue;
                                            }
                                        };
                                        self.skel.star_tx.send(StarCommand::AddLaneEndpoint(lane)).await;
                                    }
                                    Err(err) => {
                                        error!("{}", err)
                                    }
                                }
                            } else {
                                if lane_index.is_lane()
                                {
                                    self.process_frame(frame, Option::Some(&lane_index.expect_lane().unwrap())).await;
                                }
                            }
                        }
                        StarCommand::ForwardFrame(forward) => {
                            self.send_frame(forward.to.clone(), forward.frame).await;
                        }

                        StarCommand::ResourceRecordRequest(request) => {
                            self.locate_resource_record(request).await;
                        }
                        StarCommand::ResourceRecordRequestFromStar(request) => {
                            self.request_resource_record_from_star(request).await;
                        }
                        StarCommand::ResourceRecordSet(set) => {
                            self.resource_record_cache
                                .put(set.payload.stub.key.clone(), set.payload.clone());
                            set.commit();
                        }
                        StarCommand::CheckStatus => {
                            self.check_status().await;
                        }
                        StarCommand::SetStatus(status) => {
                            self.set_status(status.clone());
//                            println!("{} {}", &self.skel.info.kind, &self.status.to_string());
                        }
                        StarCommand::GetCaches(tx) => {
                            tx.send(self.skel.caches.clone());
                        }
                        StarCommand::Diagnose(diagnose) => {
                            self.diagnose(diagnose).await;
                        },
                        StarCommand::GetStatusListener(tx) => {
                            tx.send(self.status_broadcast.subscribe());
                            self.status_broadcast.send(self.status.clone());
                        }
                        StarCommand::Shutdown => {
                            for (_,lane) in &mut self.lanes {
                                lane.outgoing().out_tx.try_send(LaneCommand::Shutdown );
                            }
                            for lane in &mut self.proto_lanes{
                                lane.outgoing().out_tx.try_send(LaneCommand::Shutdown );
                            }

                            self.lanes.clear();
                            self.proto_lanes.clear();

                            self.skel.core_tx.send( StarCoreAction::new(StarCoreCommand::Shutdown).0 ).await;
                            break;
                        },
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

    fn set_status( &mut self, status: StarStatus ) {
        self.status = status.clone();
        self.status_broadcast.send(status);
    }

    async fn refresh_handles(&mut self) {
        if self.status == StarStatus::Unknown {
            self.set_status(StarStatus::Pending)
        }

        if let Option::Some(star_handler) = &self.skel.star_handler {
            for kind in self.skel.info.kind.handles() {
                let (search, rx) =
                    Wind::new(StarPattern::StarKind(kind.clone()), WindAction::SearchHits);
                self.skel.star_tx.send(StarCommand::WindInit(search)).await;
                let star_handler = star_handler.clone();
                let kind = kind.clone();
                let skel = self.skel.clone();
                tokio::spawn(async move {
                    let result = tokio::time::timeout(Duration::from_secs(5), rx).await;
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
                            kind, error.to_string()
                        );
                    }
                    Ok(Err(error)) => {
                            error!(
                                "error encountered when attempting to get a handle for: {} ERROR: {}",
                                kind, error.to_string()
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
                let satisfied = star_handler.satisfied(self.skel.info.kind.handles()).await;
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
                Err(err) => Err(Fail::ChannelRecvErr),
            },
            Err(_) => Err(Fail::Timeout),
        }
    }


    #[instrument]
    async fn locate_resource_record(
        &mut self,
        request: Request<ResourceIdentifier, ResourceRecord>,
    ) {
        if request.log {
            self.log(
                LogId(request.payload.to_string()),
                "locate_resource_record()",
                "locating...",
            );
        }

        if self.has_resource_record(&request.payload) {
            if request.log {
                self.log(
                    LogId(request.payload.to_string()),
                    "locate_resource_record()",
                    "found in cache.",
                );
            }

            request
                .tx
                .send(Ok(self.get_resource_record(&request.payload).unwrap()));
            return;
        } else if request
            .payload
            .resource_type()
            .star_manager()
            .contains(&self.skel.info.kind)
        {
            if request.log {
                self.log(
                    LogId(request.payload.to_string()),
                    "locate_resource_record()",
                    format!(
                        "<{}> star manager is contained in star kind <{}>.",
                        request.payload.resource_type().to_string(),
                        self.skel.info.kind.to_string()
                    )
                    .as_str(),
                );
            }

            match self
                .skel
                .registry
                .as_ref()
                .unwrap()
                .get(request.payload.clone())
                .await
            {
                Ok(record) => match record {
                    Some(record) => {
                        request.tx.send(Ok(record));
                    }
                    None => {
                        error!("resource not found");
                        request
                            .tx
                            .send(Err(Fail::ResourceNotFound(request.payload)));
                    }
                },
                Err(fail) => {
                    request.tx.send(Err(fail));
                }
            }
        } else if request.payload.resource_type() == ResourceType::Root {
            if request.log {
                self.log(
                    LogId(request.payload.to_string()),
                    "locate_resource_record()",
                    "resource is <Root>",
                );
            }

            let (mut new_request, rx) = Request::new((request.payload.clone(), StarKey::central()));
            new_request.log = request.log;
            self.request_resource_record_from_star(new_request).await;
            tokio::spawn(async move {
                match Star::wait_for_it(rx).await {
                    Ok(record) => {
                        request.tx.send(Ok(record));
                    }
                    Err(fail) => {
                        request.tx.send(Err(fail));
                    }
                }
            });
        } else if request.payload.parent().is_some() {
            if request.log {
                self.log(
                    LogId(request.payload.to_string()),
                    "locate_resource_record()",
                    format!(
                        "locating parent: [{}]",
                        request.payload.parent().unwrap().to_string()
                    )
                    .as_str(),
                );
            }

            let (mut new_request, rx) = Request::new(request.payload.parent().unwrap().clone());
            new_request.log = request.log;
            self.skel
                .star_tx
                .send(StarCommand::ResourceRecordRequest(new_request))
                .await;
            let skel = self.skel.clone();
            tokio::spawn(async move {
                match Star::wait_for_it(rx).await {
                    Ok(parent_record) => {
                        let (final_request, rx) =
                            Request::new((request.payload.clone(), parent_record.location.host));
                        skel.star_tx
                            .send(StarCommand::ResourceRecordRequestFromStar(final_request))
                            .await;
                        request.tx.send(Star::wait_for_it(rx).await);
                    }
                    Err(fail) => {
                        request.tx.send(Err(fail));
                    }
                }
            });
        } else {
            self.log(
                LogId(request.payload.to_string()),
                "locate_resource_record()",
                "FATAL: failed to find resource.",
            );
            request.tx.send(Err(Fail::Error(
                format!(
                    "cannot find resource_type {} has parent? {}",
                    request.payload.to_string(),
                    request.payload.parent().is_some()
                )
                .to_string(),
            )));
        }
    }

    async fn request_resource_record_from_star(
        &mut self,
        locate: Request<(ResourceIdentifier, StarKey), ResourceRecord>,
    ) {
        let (identifier, star) = locate.payload.clone();
        let mut proto = ProtoStarMessage::new();
        proto.to = star.into();
        proto.payload =
            StarMessagePayload::ResourceManager(ChildManagerResourceAction::Find(identifier));
        proto.log = locate.log;
        let reply = proto.get_ok_result().await;
        self.send_proto_message(proto).await;
        let star_tx = self.skel.star_tx.clone();
        tokio::spawn(async move {
            let result = reply.await;

            if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Resource(record)))) =
                result
            {
                let (set, rx) = Set::new(record);
                star_tx.send(StarCommand::ResourceRecordSet(set)).await;
                tokio::spawn(async move {
                    if let Result::Ok(record) = rx.await {
                        locate.tx.send(Ok(record));
                    } else {
                        locate.tx.send(Err(Fail::expected("ResourceRecord")));
                    }
                });
            } else if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Fail(fail))) = result {
                locate.tx.send(Err(fail));
            } else {
                match result {
                    Ok(StarMessagePayload::Reply(SimpleReply::Fail(Fail::ResourceNotFound(id)))) => {
                        error!("resource not found : {}", id.to_string());
                        locate.tx.send(Err(Fail::ResourceNotFound(id) ) );
                    }

                    Ok(result) => {
                        error!("payload: {}", result );
                        locate.tx.send(Err(Fail::unexpected("Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Resource(record))))", format!("{}",result.to_string()))));

                    }
                    Err(error) => {
                        error!("{}",error.to_string());
                        locate.tx.send(Err(Fail::expected("Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Resource(record))))")));
                    }
                }
            }
        });
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

    async fn send_proto_message(&mut self, mut proto: ProtoStarMessage) {
        if proto.log {
            println!(
                "{} => {} : {}",
                self.skel.info.to_string(),
                LogId(&proto).to_string(),
                "send_proto_message()"
            );
        }
        let id = MessageId::new_v4();

        let star = match proto.to.clone() {
            ProtoStarMessageTo::None => {
                eprintln!("ProtoStarMessage to address cannot be None");

                return;
            }
            ProtoStarMessageTo::Star(star) => {
                if proto.log {
                    println!(
                        "{} => {} : send_proto_message() => heading to star [{}]",
                        self.skel.info.to_string(),
                        LogId(&proto).to_string(),
                        star.to_string()
                    );
                }
                star
            }
            ProtoStarMessageTo::Resource(resource) => {
                let (mut request, rx) = Request::new(resource.clone());
                request.log = proto.log;
                self.skel
                    .star_tx
                    .send(StarCommand::ResourceRecordRequest(request))
                    .await;
                let skel = self.skel.clone();

                tokio::spawn(async move {
                    if proto.log {
                        println!(
                            "{} => {} : send_proto_message() => fetching star for resource {}",
                            skel.info.to_string(),
                            LogId(&proto).to_string(),
                            resource.to_string()
                        );
                    }

                    let result = Star::wait_for_it(rx).await;

                    match result {
                        Ok(result) => {
                            if proto.log {
                                println!(
                                    "{} => {} : send_proto_message() => found star: {}",
                                    skel.info.to_string(),
                                    LogId(&proto).to_string(),
                                    result.location.host.to_string()
                                );
                            }
                            proto.to = result.location.host.into();
                            skel.star_tx
                                .send(StarCommand::SendProtoMessage(proto))
                                .await;
                        }
                        Err(fail) => {
                            eprintln!("Star failed to find resource record: {}", fail.to_string());

                            if proto.log {
                                println!("{} => {} : send_proto_message() => FATAL: failed to fetch star for resource. ERROR: {}", skel.info.to_string(), LogId(&proto).to_string(), fail.to_string());
                            }
                        }
                    }
                });
                return;
            }
        };

        if let Err(errors) = proto.validate() {
            println!(
                "{} => {} : send_proto_message() => FATAL: proto not valid. ERROR: {}",
                self.skel.info.to_string(),
                LogId(&proto).to_string(),
                errors.to_string()
            );
            return;
        }

        let message = StarMessage {
            id: id,
            from: self.skel.info.key.clone(),
            to: star.clone(),
            payload: proto.payload.clone(),
            reply_to: proto.reply_to.clone(),
            trace: false,
            log: proto.log,
        };

        let delivery = StarMessageDeliveryInsurance::with_txrx(
            message,
            proto.expect.clone(),
            proto.tx.clone(),
            proto.tx.subscribe(),
        );
        self.message(delivery).await;

        if proto.log {
            println!(
                "{} => {} : send_proto_message() => SENT",
                self.skel.info.to_string(),
                LogId(&proto).to_string()
            );
        }
    }

    async fn search_for_star(&mut self, star: StarKey, tx: oneshot::Sender<WindHits>) {
        let wind = Wind {
            pattern: StarPattern::StarKey(star),
            tx: tx,
            max_hops: 16,
            action: WindAction::SearchHits,
        };
        self.skel.star_tx.send(StarCommand::WindInit(wind)).await;
    }

    async fn do_wind(&mut self, wind: Wind) {
        let tx = wind.tx;
        let wind_up = WindUp::new(self.skel.info.key.clone(), wind.pattern, wind.action);
        self.do_wind_up(wind_up, tx, Option::None).await;
    }

    async fn do_wind_up(
        &mut self,
        mut wind: WindUp,
        tx: oneshot::Sender<WindHits>,
        exclude: Option<HashSet<StarKey>>,
    ) {
        let tid = self
            .skel
            .sequence
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);


        let local_hit = match wind.pattern.is_match(&self.skel.info) {
            true => Option::Some(self.skel.info.key.clone()),
            false => Option::None,
        };

        let mut lanes = HashSet::from_iter(self.lanes.keys().cloned().into_iter());

        match &exclude {
            None => {},
            Some(exclude) => {
                lanes.retain( |k| !exclude.contains(k));
            },
        }

        let transaction = Box::new(StarSearchTransaction::new(
            wind.pattern.clone(),
            self.skel.star_tx.clone(),
            tx,
            lanes,
            local_hit,
        ));
        self.transactions.insert(tid.clone(), transaction);

        wind.transactions.push(tid.clone());
        wind.hops.push(self.skel.info.key.clone());

        self.broadcast_excluding(Frame::StarWind(StarWind::Up(wind)), &exclude)
            .await;
    }

    async fn on_wind_up_hop(&mut self, mut wind_up: WindUp, lane_key: StarKey) {
        if wind_up.pattern.is_match(&self.skel.info) {
            if wind_up.pattern.is_single_match() {
                let hit = WindHit {
                    star: self.skel.info.key.clone(),
                    hops: wind_up.hops.len() + 1,
                };

                match wind_up.action.update(vec![hit], WindResults::None) {
                    Ok(result) => {
                        let wind_down = WindDown {
                            missed: None,
                            hops: wind_up.hops.clone(),
                            transactions: wind_up.transactions.clone(),
                            wind_up: wind_up,
                            result: result,
                        };

                        let wind = Frame::StarWind(StarWind::Down(wind_down));

                        let lane = self.lanes.get_mut(&lane_key).unwrap();
                        lane.outgoing().out_tx.send(LaneCommand::Frame(wind)).await;
                    }
                    Err(error) => {
                        eprintln!(
                            "error when attempting to update wind_down results {}",
                            error
                        );
                    }
                }

                return;
            } else {
                // need to create a new transaction here which gathers 'self' as a HIT
            }
        }

        let hit = wind_up.pattern.is_match(&self.skel.info);

        if wind_up.hops.len() + 1 > min(wind_up.max_hops, MAX_HOPS)
            || self.lanes.len() <= 1
            || !self.skel.info.kind.relay()
        {
            let hits = match hit {
                true => {
                    vec![WindHit {
                        star: self.skel.info.key.clone(),
                        hops: wind_up.hops.len().clone() + 1,
                    }]
                }
                false => {
                    vec![]
                }
            };

            match wind_up.action.update(hits, WindResults::None) {
                Ok(result) => {
                    let wind_down = WindDown {
                        missed: None,
                        hops: wind_up.hops.clone(),
                        transactions: wind_up.transactions.clone(),
                        wind_up: wind_up,
                        result: result,
                    };

                    let wind = Frame::StarWind(StarWind::Down(wind_down));

                    let lane = self.lanes.get_mut(&lane_key).unwrap();
                    lane.outgoing().out_tx.send(LaneCommand::Frame(wind)).await;
                }
                Err(error) => {
                    eprintln!(
                        "error encountered when trying to update WindResult: {}",
                        error
                    );
                }
            }

            return;
        }

        let mut exclude = HashSet::new();
        exclude.insert(lane_key);

        let (tx, rx) = oneshot::channel();

        let relay_wind_up = wind_up.clone();

        let command_tx = self.skel.star_tx.clone();
        self.do_wind_up(relay_wind_up, tx, Option::Some(exclude))
            .await;

        tokio::spawn(async move {
            //            result.hits.iter().map(|(star,hops)| SearchHit{ star: star.clone(), hops: hops.clone()+1} ).collect()

            let wind_result = rx.await;

            match wind_result {
                Ok(wind_result) => {
                    let hits = wind_result
                        .hits
                        .iter()
                        .map(|(star, hops)| WindHit {
                            star: star.clone(),
                            hops: hops.clone() + 1,
                        })
                        .collect();
                    match wind_up.action.update(hits, WindResults::None) {
                        Ok(result) => {
                            let mut wind_down = WindDown {
                                missed: None,
                                hops: wind_up.hops.clone(),
                                wind_up: wind_up.clone(),
                                transactions: wind_up.transactions.clone(),
                                result: result,
                            };
                            command_tx.send(StarCommand::WindDown(wind_down)).await;
                        }
                        Err(error) => {
                            eprintln!("{}", error);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("{}", error);
                }
            }
        });
    }

    pub fn star_key(&self) -> &StarKey {
        &self.skel.info.key
    }

    pub fn star_tx(&self) -> mpsc::Sender<StarCommand> {
        self.skel.star_tx.clone()
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

    async fn message(&mut self, delivery: StarMessageDeliveryInsurance) {
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
                    let result = timeout(Duration::from_secs(wait), delivery.rx.recv()).await;
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
                        Err(elapsed) => {
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

    async fn send_frame(&mut self, star: StarKey, frame: Frame) {
        let lane = self.lane_with_shortest_path_to_star(&star);
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
    }

    fn lane_with_shortest_path_to_star(&mut self, star: &StarKey) -> Option<&mut LaneWrapper> {
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

    async fn on_wind_down(&mut self, mut search_result: WindDown, lane_key: StarKey) {
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

    async fn process_transactions(&mut self, frame: &Frame, lane_key: Option<&StarKey>) {
        let tid = match frame {
            /*            Frame::StarMessage(message) => {
                           message.transaction
                       },

            */
            Frame::StarWind(wind) => match wind {
                StarWind::Down(wind_down) => wind_down.transactions.last().cloned(),
                _ => Option::None,
            },
            _ => Option::None,
        };

        if let Option::Some(tid) = tid {
            let transaction = self.transactions.get_mut(&tid);
            if let Option::Some(transaction) = transaction {
                let lane = match lane_key {
                    None => Option::None,
                    Some(lane_key) => self.lanes.get_mut(lane_key),
                };

                match transaction
                    .on_frame(frame, lane, &mut self.skel.star_tx)
                    .await
                {
                    TransactionResult::Continue => {}
                    TransactionResult::Done => {
                        self.transactions.remove(&tid);
                    }
                }
            }
        }
    }

    async fn on_lane_closed( &mut self, key: &StarKey ) {
        // we should notify any waiting WIND transactions that this lane is no longer participating
        let mut remove = HashSet::new();
        for (tid,transaction) in self.transactions.iter_mut() {
            if let TransactionResult::Done = transaction.on_lane_closed(key).await {
                remove.insert(tid.clone() );
            }
        }

        self.transactions.retain( |k,_| !remove.contains(k) );
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
        self.process_transactions(&frame, lane_key).await;
        match frame {
            Frame::StarWind(wind) => match wind {
                StarWind::Up(wind_up) => {
                    if let Option::Some(lane_key) = lane_key {
                        self.on_wind_up_hop(wind_up, lane_key.clone()).await;
                    } else {
                        error!("missing lanekey on WindUp");
                    }
                }
                StarWind::Down(wind_down) => {
                    if let Option::Some(lane_key) = lane_key {
                        self.on_wind_down(wind_down, lane_key.clone()).await;
                    } else {
                        error!("missing lanekey on WindDown");
                    }
                }
            },
            Frame::StarMessage(message) => match self.on_message(message).await {
                Ok(messages) => {}
                Err(error) => {
                    error!("X error: {}", error)
                }
            },
            _ => {
                error!("star does not handle frame: {}", frame)
            }
        }
    }

    async fn on_event(&mut self, event: Event, lane_key: StarKey) {
        unimplemented!()
        /*
        let watches = self.watches.get(&event.actor );

        if watches.is_some()
        {
            let watches = watches.unwrap();
            let mut stars: HashSet<StarKey> = watches.values().map( |info| info.lane.clone() ).collect();
            // just in case! we want to avoid a loop condition
            stars.remove( &lane_key );

            for lane in stars
            {
                self.send_frame( lane.clone(), Frame::Event(event.clone()));
            }
        }
         */
    }

    async fn on_watch(&mut self, watch: Watch, lane_key: StarKey) {
        match &watch {
            Watch::Add(info) => {
                self.watch_add_renew(info, &lane_key);
                self.forward_watch(watch).await;
            }
            Watch::Remove(info) => {
                if let Option::Some(watches) = self.watches.get_mut(&info.actor) {
                    watches.remove(&info.id);
                    if watches.is_empty() {
                        self.watches.remove(&info.actor);
                    }
                }
                self.forward_watch(watch).await;
            }
        }
    }

    fn watch_add_renew(&mut self, watch_info: &WatchInfo, lane_key: &StarKey) {
        let star_watch = StarWatchInfo {
            id: watch_info.id.clone(),
            lane: lane_key.clone(),
            timestamp: Instant::now(),
        };
        match self.watches.get_mut(&watch_info.actor) {
            None => {
                let mut watches = HashMap::new();
                watches.insert(watch_info.id.clone(), star_watch);
                self.watches.insert(watch_info.actor.clone(), watches);
            }
            Some(mut watches) => {
                watches.insert(watch_info.id.clone(), star_watch);
            }
        }
    }

    async fn forward_watch(&mut self, watch: Watch) {
        unimplemented!()
        /*
        let has_entity = match &watch
        {
            Watch::Add(info) => {
                self.has_resource(&ResourceKey::Actor(info.actor.clone()))
            }
            Watch::Remove(info) => {
                self.has_resource(&ResourceKey::Actor(info.actor.clone()))
            }
        };

        let entity = match &watch
        {
            Watch::Add(info) => {
                &info.actor
            }
            Watch::Remove(info) => {
                &info.actor
            }
        };

        if has_entity
        {
            self.core_tx.send(StarCoreCommand::Watch(watch)).await;
        }
        else
        {
            let lookup = ActorLookup::Key(entity.clone());
            let location = self.get_entity_location(lookup.clone() );


            if let Some(location) = location.cloned()
            {
                self.send_frame(location.host.clone(), Frame::Watch(watch)).await;
            }
            else
            {
                let mut rx = self.find_actor_location(lookup).await;
                let command_tx = self.skel.star_tx.clone();
                tokio::spawn( async move {
                    if let Option::Some(_) = rx.recv().await
                    {
                        command_tx.send(StarCommand::Frame(Frame::Watch(watch))).await;
                    }
                });
            }
        }

         */
    }

    async fn on_message(&mut self, mut message: StarMessage) -> Result<(), Error> {
        if message.log {
/*            info!(
                "{} => {} : {}",
                self.skel.info.to_string(),
                LogId(&message).to_string(),
                "on_message"
            );
 */
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
                    format!("this star {} does not relay Messages", self.skel.info.kind).into(),
                );
            }
        } else {
            self.process_message_reply(&message).await;
            self.process_resource_message(message).await?;
            Ok(())
        }
    }

    async fn process_resource_message(
        &mut self,
        mut star_message: StarMessage,
    ) -> Result<(), Error> {
        //println!("process_message---> {}", message.payload.to_string() );
        match &star_message.payload {
            StarMessagePayload::ResourceManager(action) => {
                self.process_child_manager_resource_action(star_message.clone(), action.clone())
                    .await?
            }
            StarMessagePayload::ResourceHost(action) => {
                self.process_resource_host_action(star_message.clone(), action.clone())
                    .await?
            }

            StarMessagePayload::None => {}
            StarMessagePayload::MessagePayload(message_payload) => match &message_payload {
                MessagePayload::Request(request) => {
                    let delivery = Delivery::new(request.clone(), star_message, self.skel.clone());
                    self.process_resource_message_request_delivery(delivery)
                        .await?;
                }
                MessagePayload::Response(_) => {}
                MessagePayload::Actor(_) => {}
            },
            StarMessagePayload::Space(_) => {}
            StarMessagePayload::Reply(_) => {}
            StarMessagePayload::UniqueId(_) => {}
        };
        Ok(())
    }

    async fn process_resource_message_request_delivery(
        &mut self,
        delivery: Delivery<Message<ResourceRequestMessage>>,
    ) -> Result<(), Error> {

        match &delivery.message.payload {
            ResourceRequestMessage::Create(create) => {
                let parent_key = match create.parent.clone().key_or("expected parent to be a ResourceKey"){
                    Ok(key) => {key}
                    Err(error) => {
                        return Err(error);
                    }
                };
                let child_manager = self
                    .get_child_resource_manager(parent_key)
                    .await?;
                let delivery = delivery.clone();
                let create = create.clone();
                tokio::spawn(async move {
                    let record = child_manager.create(create.clone()).await.await;
                    match record {
                        Ok(record) => match record {
                            Ok(record) => {
                                delivery
                                    .reply(ResourceResponseMessage::Resource(Option::Some(record)))
                                    .await;
                            }
                            Err(fail) => {
                                eprintln!("Fail: {}", fail.to_string());
                            }
                        },
                        Err(err) => {
                            eprintln!("Error: {}", err);
                        }
                    }
                });
            }
            ResourceRequestMessage::Select(selector) => {
                let resources = self.skel.registry.as_ref().unwrap().select(selector.clone()).await?;
                delivery
                    .reply(ResourceResponseMessage::Resources( resources ))
                    .await?;
            }
            ResourceRequestMessage::Unique(resource_type) => {
                let unique_src = self
                    .skel
                    .registry
                    .as_ref()
                    .unwrap()
                    .unique_src(delivery.message.to.clone().into())
                    .await;
                delivery
                    .reply(ResourceResponseMessage::Unique(
                        unique_src.next(resource_type).await?,
                    ))
                    .await?;
            }
            ResourceRequestMessage::State => {
                let (action, mut rx) =
                    StarCoreAction::new(StarCoreCommand::State(delivery.message.to.clone()));
                self.skel.core_tx.send(action).await?;
                tokio::spawn(async move {
                    let result = rx.await;
                    if let Ok(Ok(StarCoreResult::State(state))) = result {
                        delivery
                            .reply(ResourceResponseMessage::State(state))
                            .await
                            .unwrap_or_default();
                    } else {
                        delivery
                            .reply(ResourceResponseMessage::Fail(Fail::expected("Ok(Ok(StarCoreResult::State(state)))")))
                            .await
                            .unwrap_or_default();
                    }
                });
            }
        }
        Ok(())
    }


    async fn process_child_manager_resource_action(
        &mut self,
        message: StarMessage,
        action: ChildManagerResourceAction,
    ) -> Result<(), Error> {

        if let Option::Some(manager) = self.skel.registry.clone() {

            match action {
                ChildManagerResourceAction::Register(registration) => {
                    let result = manager.register(registration.clone()).await;
                    self.skel
                        .comm()
                        .reply_result_empty(message.clone(), result)
                        .await;
                }
                ChildManagerResourceAction::Location(location) => {
                    let result = manager.set_location(location.clone()).await;
                    self.skel
                        .comm()
                        .reply_result_empty(message.clone(), result)
                        .await;
                }
                ChildManagerResourceAction::Find(find) => {

                    let result = manager.get(find.to_owned()).await;

                    match result {
                        Ok(result) => match result {
                            Some(record) => {
                                self.skel
                                    .comm()
                                    .reply_result(message.clone(), Ok(Reply::Resource(record)))
                                    .await;
                            }
                            None => {
                                self.skel
                                    .comm()
                                    .reply_result(
                                        message.clone(),
                                        Err(Fail::ResourceNotFound(find)),
                                    )
                                    .await;
                            }
                        },
                        Err(fail) => {
                            self.skel
                                .comm()
                                .reply_result(message.clone(), Err(fail))
                                .await;
                        }
                    }
                }
                ChildManagerResourceAction::Status(report) => {
                    unimplemented!()
                }

                ChildManagerResourceAction::Create(create) => {
                    unimplemented!();
/*                    let child_manager = self
                        .get_child_resource_manager(parent )
                        .await?;
                    let skel = self.skel.clone();
                    tokio::spawn(async move {
                        let record = child_manager.create(create.clone()).await.await;
                        match record {
                            Ok(record) => match record {
                                Ok(record) => {
                                    skel.comm()
                                        .reply_result(message, Ok(Reply::Resource(record)))
                                        .await;
                                }
                                Err(fail) => {
                                    eprintln!("Fail: {}", fail.to_string());
                                }
                            },
                            Err(err) => {
                                eprintln!("Error: {}", err);
                            }
                        }
                    });

 */

                    //        println!("child {} location host: {}", record.key, record.location.host );

                    /*                    tokio::spawn( async move {
                                            let parent = create.parent.clone();
                    println!("parent_key: {}", parent.clone());
                                            if let Result::Ok(manager) = self.get_child_resource_manager(parent.clone() ).await
                                            {
                                                let manager = manager.await??;
                                                match manager.create(create).await.await {
                                                    Ok(result) => {
                                                        match result {
                                                            Ok(record) => {
                                                                skel.comm().simple_reply(message, SimpleReply::Ok(Reply::Key(record.key))).await;
                                                            }
                                                            Err(fail) => {
                                                                skel.comm().simple_reply(message, SimpleReply::Fail(fail)).await;
                                                            }
                                                        }
                                                    }
                                                    Err(err) => {
                                                        eprintln!("{}",err);
                                                        skel.comm().simple_reply(message, SimpleReply::Fail(Fail::Unexpected)).await;
                                                    }
                                                }
                                            } else {
                                                eprintln!("could not find resource manager for {} within star {} and star kind {}",parent, &skel.info.star, &skel.info.kind );
                                                skel.comm().simple_reply(message, SimpleReply::Fail(Fail::Unexpected)).await;
                                            }
                                        });*/
                }
                ChildManagerResourceAction::UniqueResourceId { parent, child_type } => {
                    let unique_src = self
                        .skel
                        .registry
                        .as_ref()
                        .unwrap()
                        .unique_src(parent)
                        .await;
                    let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(
                        Reply::Id(unique_src.next(&child_type).await?),
                    )));
                    self.skel
                        .star_tx
                        .send(StarCommand::SendProtoMessage(proto))
                        .await;
                }
            }
        }

        Ok(())
    }

    async fn process_resource_host_action(
        &self,
        message: StarMessage,
        action: ResourceHostAction,
    ) -> Result<(), Error> {
        match action {
            ResourceHostAction::IsHosting(resource) => {
                if let Option::Some(resource) = self.get_resource(&resource).await? {
                    let record = resource.into();
                    let record = ResourceRecord::new(record, self.skel.info.key.clone());
                    self.skel
                        .comm()
                        .simple_reply(message, SimpleReply::Ok(Reply::Resource(record)))
                        .await;
                } else {
                    self.skel
                        .comm()
                        .simple_reply(
                            message,
                            SimpleReply::Fail(Fail::ResourceNotFound(resource.into())),
                        )
                        .await;
                }
            }
            ResourceHostAction::Assign(assign) => {
                let (action, rx) = StarCoreAction::new(StarCoreCommand::Assign(assign.clone()));
                self.skel.core_tx.send(action).await;
                let result = rx.await??;
                if let StarCoreResult::Resource(Option::Some(resource)) = result {
                    let record = ResourceRecord::new(resource.into(), self.skel.info.key.clone());
                    self.skel
                        .comm()
                        .simple_reply(message, SimpleReply::Ok(Reply::Resource(record)))
                        .await;
                } else {
                    self.skel
                        .comm()
                        .simple_reply(message, SimpleReply::Fail(Fail::expected("Option::Some(resource)")))
                        .await;
                }
            }
        }
        Ok(())
    }

    async fn get_child_resource_manager(&mut self, key: ResourceKey) -> Result<Parent, Fail> {
        let resource = match key.resource_type() {
            ResourceType::Root => {
                if self.skel.info.kind != StarKind::Central {
                    return Err(Fail::ResourceNotFound(ResourceKey::Root.into()));
                }
                Option::Some(Resource::new(
                    key.clone(),
                    ResourceAddress::root(),
                    ResourceArchetype {
                        kind: ResourceKind::Root,
                        specific: None,
                        config: None,
                    },
                    Arc::new(MemoryDataTransfer::none()),
                ))
            }
            _ => self.get_resource(&key).await?,
        };

        if let Option::Some(resource) = resource {
            Ok(Parent {
                core: ParentCore {
                    stub: resource.into(),
                    selector: ResourceHostSelector::new(self.skel.clone()),
                    child_registry: self.skel.registry.as_ref().unwrap().clone(),
                    skel: self.skel.clone(),
                },
            })
        } else {
            Err(Fail::ResourceNotFound(key.clone().into()))
        }
    }

    /*    async fn get_child_resource_manager(&mut self, key: ResourceKey ) -> Result<oneshot::Receiver<Result<ChildResourceManager,Fail>>,Fail>{
    println!(" ::::>  GET RESOURCE MANAGER for {} <:::: [star kind {}]",key,&self.skel.info.kind );

            let resource = match key.resource_type(){
                ResourceType::Nothing => {
                    if self.skel.info.kind != StarKind::Central {
                        return Err(Fail::ResourceNotFound(ResourceKey::Nothing))
                    }

                    Option::Some(Arc::new(LocalHostedResource {
                        unique_src: self.skel.registry.clone().ok_or(format!("this star {} does not host resources",self.skel.info.kind))?.unique_src(key.clone()).await,
                        resource: ResourceStub {
                            key:key.clone(),
                            address: ResourceAddress::nothing(),
                            archetype: ResourceArchetype {
                                kind: ResourceKind::Nothing,
                                specific: None,
                                config: None
                            },
                            owner: None
                        }
                    }))
                }
                _ => {
                    self.skel.resources.get(key.clone()).await?
                }
            };

            if let Option::Some(resource) = resource {
    println!("::::> FOUND RESOURCE MANAGER :::::");
                let (tx, rx) = oneshot::channel();

                let mut star_api = StarlaneApi::new(self.skel.star_tx.clone());
                let skel = self.skel.clone();

                if self.skel.registry.is_none() {
                    tx.send(Err(Fail::Error(format!("this star: {} does not have a registry and therefore cannot manage resources", self.skel.info.kind))));
                    return Err(Fail::Unexpected);
                }

                let id_seq = Arc::new(IdSeq::new(0));

                tokio::spawn(async move {
                    match star_api.fetch_resource_address(key.clone()).await {
                        Ok(address) => {
                            let manager = ChildResourceManager {
                                core: ChildResourceManagerCore {
                                    key: key.clone(),
                                    address: address,
                                    selector: ResourceHostSelector::new(skel.clone()),
                                    registry: skel.registry.as_ref().cloned().unwrap(),
                                    id_seq: id_seq.clone()
                                }
                            };
                            tx.send(Ok(manager));
                        }
                        Err(fail) => {
                            tx.send(Err(fail));
                        }
                    }
                });

                Ok(rx)
            } else {
    println!("::::> ??? RESOURCE MANAGER NOT FOUND :::::");
                    Err(Fail::ResourceNotFound(key.clone()))
            }
        }

     */

    async fn diagnose(&self, diagnose: Diagnose) {
        match diagnose {
            Diagnose::HandlersSatisfied(satisfied) => {
                if let Option::Some(star_handler) = &self.skel.star_handler {
                    if let Result::Ok(satisfaction) =
                        star_handler.satisfied(self.skel.info.kind.handles()).await
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
    SendProtoMessage(ProtoStarMessage),
    SetFlags(SetFlags),
    ReleaseHold(StarKey),
    GetStarInfo(oneshot::Sender<Option<StarInfo>>),
    WindInit(Wind),
    WindCommit(WindCommit),
    WindDown(WindDown),

    Test(StarTest),

    Frame(Frame),
    ForwardFrame(ForwardFrame),
    FrameTimeout(FrameTimeoutInner),
    FrameError(FrameErrorInner),

    Diagnose(Diagnose),
    CheckStatus,
    SetStatus(StarStatus),
    RefreshHandles,

    ResourceRecordRequest(Request<ResourceIdentifier, ResourceRecord>),
    ResourceRecordRequestFromStar(Request<(ResourceIdentifier, StarKey), ResourceRecord>),
    ResourceRecordSet(Set<ResourceRecord>),

    GetCaches(oneshot::Sender<Arc<ProtoArtifactCachesFactory>>),
    Shutdown
}

#[derive(Clone)]
pub enum ConstellationBroadcast{
    Status(ConstellationStatus)
}

pub enum Diagnose {
    HandlersSatisfied(YesNo<Satisfaction>),
}

pub struct SetFlags {
    pub flags: Flags,
    pub tx: oneshot::Sender<()>,
}

pub struct ActorCreate {
    pub app: AppKey,
    pub kind: ActorKind,
    pub data: Arc<Vec<u8>>,
}

impl ActorCreate {
    pub fn new(app: AppKey, kind: ActorKind, data: Vec<u8>) -> Self {
        ActorCreate {
            app: app,
            kind: kind,
            data: Arc::new(data),
        }
    }
}

pub struct ForwardFrame {
    pub to: StarKey,
    pub frame: Frame,
}

pub struct AddResourceLocation {
    pub tx: mpsc::Sender<()>,
    pub resource_location: ResourceRecord,
}

pub struct Wind {
    pub pattern: StarPattern,
    pub tx: oneshot::Sender<WindHits>,
    pub max_hops: usize,
    pub action: WindAction,
}

impl Wind {
    pub fn new(pattern: StarPattern, action: WindAction) -> (Self, oneshot::Receiver<WindHits>) {
        let (tx, rx) = oneshot::channel();
        (
            Wind {
                pattern: pattern,
                tx: tx,
                max_hops: 16,
                action: action,
            },
            rx,
        )
    }
}

pub enum CoreRequest {
    AppSequenceRequest(CoreAppSequenceRequest),
}

pub struct CoreAppSequenceRequest {
    pub app: AppKey,
    pub user: UserKey,
    pub tx: Sender<u64>,
}

pub struct SetSupervisorForApp {
    pub supervisor: StarKey,
    pub app: AppKey,
}

impl SetSupervisorForApp {
    pub fn new(supervisor: StarKey, app: AppKey) -> Self {
        SetSupervisorForApp {
            supervisor: supervisor,
            app: app,
        }
    }
}

pub enum ServerCommand {
    PledgeToSupervisor,
}

pub struct LocalResourceLocation {
    pub resource: ResourceKey,
    pub gathering: Option<GatheringKey>,
}

impl LocalResourceLocation {
    pub fn new(resource: ResourceKey, gathering: Option<GatheringKey>) -> Self {
        LocalResourceLocation {
            resource: resource,
            gathering: gathering,
        }
    }
}

pub struct Request<P:Debug, R> {
    pub payload: P,
    pub tx: oneshot::Sender<Result<R, Fail>>,
    pub log: bool,
}

impl<P:Debug, R> Debug for Request<P,R>
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.payload.fmt(f)
    }
}


impl<P:Debug, R> Request<P, R> {
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

/*
impl fmt::Display for StarCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarCommand::Init => "Init".to_string(),
            StarCommand::AddLaneEndpoint(_) => format!("AddLane").to_string(),
            StarCommand::AddConnectorController(_) => format!("AddConnectorController").to_string(),
            StarCommand::AddLogger(_) => format!("AddLogger").to_string(),
            StarCommand::Test(_) => format!("Test").to_string(),
            StarCommand::Frame(frame) => format!("Frame({})", frame).to_string(),
            StarCommand::FrameTimeout(_) => format!("FrameTimeout").to_string(),
            StarCommand::FrameError(_) => format!("FrameError").to_string(),
            StarCommand::WindInit(_) => format!("Search").to_string(),
            StarCommand::WindCommit(_) => format!("SearchResult").to_string(),
            StarCommand::ReleaseHold(_) => format!("ReleaseHold").to_string(),
            StarCommand::ForwardFrame(_) => format!("ForwardFrame").to_string(),
            StarCommand::WindDown(_) => format!("SearchReturnResult").to_string(),
            StarCommand::SendProtoMessage(_) => format!("SendProtoMessage(_)").to_string(),
            StarCommand::SetFlags(_) => format!("SetFlags(_)").to_string(),
            StarCommand::ConstellationBroadcast(_)=> {
                "ConstellationBroadcast".to_string()
            }
            StarCommand::ResourceRecordRequest(_) => "ResourceRecordRequest".to_string(),
            StarCommand::ResourceRecordSet(_) => "SetResourceLocation".to_string(),
            StarCommand::Diagnose(_) => "Diagnose".to_string(),
            StarCommand::CheckStatus => "CheckStatus".to_string(),
            StarCommand::ResourceRecordRequestFromStar(_) => {
                "ResourceRecordRequestFromStar".to_string()
            }
            StarCommand::SetStatus(_) => "StarStatus".to_string(),
            StarCommand::GetCaches(_) => "GetCaches".to_string(),
            StarCommand::GetStarInfo(_) => "GetStarInfo".to_string(),
            StarCommand::AddProtoLaneEndpoint(_) => "ProtoLaneEndpoint".to_string()
        };
        write!(f, "{}", r)
    }
}

 */

#[derive(Clone)]
pub struct StarController {
    pub star_tx: mpsc::Sender<StarCommand>,
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

    pub async fn get_star_info( &self )->Result<Option<StarInfo>,Error> {
        let (tx,rx) = oneshot::channel();
        self.star_tx.send(StarCommand::GetStarInfo(tx)).await;
        Ok(rx.await?)
    }
}

#[derive(Clone)]
pub struct StarWatchInfo {
    pub id: Id,
    pub timestamp: Instant,
    pub lane: StarKey,
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

#[async_trait]
impl Transaction for ResourceLocationRequestTransaction {
    async fn on_frame(
        &mut self,
        frame: &Frame,
        lane: Option<&mut LaneWrapper>,
        command_tx: &mut mpsc::Sender<StarCommand>,
    ) -> TransactionResult {
        /*

        if let Frame::StarMessage( message ) = frame
        {
            if let StarMessagePayload::ActorLocationReport(location ) = &message.payload
            {
                command_tx.send( StarCommand::AddActorLocation(AddEntityLocation { tx: self.tx.clone(), entity_location: location.clone() })).await;
            }
        }



         */
        unimplemented!();
        TransactionResult::Done
    }
}

pub struct StarSearchTransaction {
    pub pattern: StarPattern,
    pub reported_lanes: HashSet<StarKey>,
    pub lanes: HashSet<StarKey>,
    pub hits: HashMap<StarKey, HashMap<StarKey, usize>>,
    command_tx: mpsc::Sender<StarCommand>,
    tx: Vec<oneshot::Sender<WindHits>>,
    local_hit: Option<StarKey>,
}

impl StarSearchTransaction {
    pub fn new(
        pattern: StarPattern,
        command_tx: mpsc::Sender<StarCommand>,
        tx: oneshot::Sender<WindHits>,
        lanes: HashSet<StarKey>,
        local_hit: Option<StarKey>,
    ) -> Self {
        StarSearchTransaction {
            pattern: pattern,
            reported_lanes: HashSet::new(),
            hits: HashMap::new(),
            command_tx: command_tx,
            tx: vec![tx],
            lanes: lanes,
            local_hit: local_hit,
        }
    }

    fn collapse(&self) -> HashMap<StarKey, usize> {
        let mut rtn = HashMap::new();
        for (lane, map) in &self.hits {
            for (star, hops) in map {
                if rtn.contains_key(star) {
                    if let Some(old) = rtn.get(star) {
                        if hops < old {
                            rtn.insert(star.clone(), hops.clone());
                        }
                    }
                } else {
                    rtn.insert(star.clone(), hops.clone());
                }
            }
        }

        if let Option::Some(local) = &self.local_hit {
            rtn.insert(local.clone(), 0);
        }

        rtn
    }

    pub async fn commit(&mut self) {
        if self.tx.len() != 0 {
            let tx = self.tx.remove(0);
            let commit = WindCommit {
                tx: tx,
                result: WindHits {
                    pattern: self.pattern.clone(),
                    hits: self.collapse(),
                    lane_hits: self.hits.clone(),
                },
            };

            self.command_tx.send(StarCommand::WindCommit(commit)).await;
        }
    }
}

#[async_trait]
impl Transaction for StarSearchTransaction {

    async fn on_lane_closed( &mut self, key: &StarKey ) -> TransactionResult  {
        self.lanes.remove(key );
        self.reported_lanes.remove(key );

        if self.reported_lanes == self.lanes {
            self.commit().await;
            TransactionResult::Done
        } else {
            TransactionResult::Continue
        }
    }

    async fn on_frame(
        &mut self,
        frame: &Frame,
        lane: Option<&mut LaneWrapper>,
        command_tx: &mut mpsc::Sender<StarCommand>,
    ) -> TransactionResult {
        if let Option::None = lane {
            eprintln!("lane is not set for StarSearchTransaction");
            return TransactionResult::Done;
        }

        let lane = lane.unwrap();

        if let Frame::StarWind(StarWind::Down(wind_down)) = frame {
            if let WindResults::Hits(hits) = &wind_down.result {
                let mut lane_hits = HashMap::new();
                for hit in hits.clone() {
                    if !lane_hits.contains_key(&hit.star) {
                        lane_hits.insert(hit.star.clone(), hit.hops);
                    } else {
                        if let Option::Some(old) = lane_hits.get(&hit.star) {
                            if hit.hops < *old {
                                lane_hits.insert(hit.star.clone(), hit.hops);
                            }
                        }
                    }
                }

                self.hits
                    .insert(lane.get_remote_star().unwrap(), lane_hits);
            }
        }

        self.reported_lanes.insert( lane.get_remote_star().expect("expected the lane to have a remote star key") );

        if self.reported_lanes == self.lanes {
            self.commit().await;
            TransactionResult::Done
        } else {
            TransactionResult::Continue
        }
    }
}

pub struct LaneHit {
    lane: StarKey,
    star: StarKey,
    hops: usize,
}

pub struct WindCommit {
    pub result: WindHits,
    pub tx: oneshot::Sender<WindHits>,
}

#[derive(Clone)]
pub struct WindHits {
    pub pattern: StarPattern,
    pub hits: HashMap<StarKey, usize>,
    pub lane_hits: HashMap<StarKey, HashMap<StarKey, usize>>,
}

impl WindHits {
    pub fn nearest(&self) -> Option<WindHit> {
        let mut min: Option<WindHit> = Option::None;

        for (star, hops) in &self.hits {
            if min.as_ref().is_none() || hops < &min.as_ref().unwrap().hops {
                min = Option::Some(WindHit {
                    star: star.clone(),
                    hops: hops.clone(),
                });
            }
        }

        min
    }
}

pub enum TransactionResult {
    Continue,
    Done,
}

#[async_trait]
pub trait Transaction: Send + Sync {
    async fn on_lane_closed( &mut self, key: &StarKey ) -> TransactionResult  {
            TransactionResult::Continue
    }

    async fn on_frame(
        &mut self,
        frame: &Frame,
        lane: Option<&mut LaneWrapper>,
        command_tx: &mut mpsc::Sender<StarCommand>,
    ) -> TransactionResult;
}

pub struct ShortestPathStarKey {
    pub to: StarKey,
    pub next_lane: StarKey,
    pub hops: usize,
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
        let mut bin = bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn from_bin(mut bin: Vec<u8>) -> Result<StarKey, Error> {
        let mut key = bincode::deserialize::<StarKey>(bin.as_slice())?;
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

    pub fn child_subgraph( &self ) -> Vec<StarSubGraphKey>{
        let mut subgraph = self.subgraph.clone();
        subgraph.push( StarSubGraphKey::Small(self.index));
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
    pub core_tx: mpsc::Sender<StarCoreAction>,
    pub flags: Flags,
    pub logger: Logger,
    pub sequence: Arc<AtomicU64>,
    pub auth_token_source: AuthTokenSource,
    pub registry: Option<Arc<dyn ResourceRegistryBacking>>,
    pub star_handler: Option<StarHandleBacking>,
    pub persistence: Persistence,
    pub data_access: FileAccess,
    pub caches: Arc<ProtoArtifactCachesFactory>,
}

impl Debug for StarSkel{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.info.fmt(f)
    }
}

impl StarSkel {
    pub fn comm(&self) -> StarComm {
        StarComm {
            star_tx: self.star_tx.clone(),
            core_tx: self.core_tx.clone(),
        }
    }
}



#[derive(Debug,Clone)]
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

pub struct PublicKeySource {}

impl PublicKeySource {
    pub fn new() -> Self {
        PublicKeySource {}
    }

    pub async fn get_public_key_and_hash(&self, star: &StarKey) -> (PublicKey, UniqueHash) {
        (
            PublicKey {
                id: Default::default(),
                data: vec![],
            },
            UniqueHash {
                id: HashId::new_v4(),
                hash: vec![],
            },
        )
    }

    pub async fn create_encrypted_payloads(
        &self,
        creds: &Credentials,
        star: &StarKey,
        payload: SpaceMessage,
    ) -> Result<(HashEncrypted<AuthToken>, Encrypted<SpaceMessage>), Error> {
        unimplemented!();
        /*
        let auth_token = AuthTokenSource::new();
        let (public_key,hash) = self.get_public_key_and_hash(star).await;
        let auth_token = HashEncrypted::encrypt(&auth_token.auth(creds).unwrap(), &hash, &public_key );
        let payload = Encrypted::encrypt( &payload, &public_key );
        Ok((auth_token,payload))
         */
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

#[derive(Clone)]
pub struct StarComm {
    pub star_tx: mpsc::Sender<StarCommand>,
    pub core_tx: mpsc::Sender<StarCoreAction>,
}

impl StarComm {
    pub async fn send(&self, proto: ProtoStarMessage) {
        self.star_tx
            .send(StarCommand::SendProtoMessage(proto))
            .await;
    }
    pub async fn reply<R>(&self, message: StarMessage, result: Result<R, Fail>) {
        match result {
            Ok(_) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
                self.send(proto).await;
            }
            Err(fail) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                self.send(proto).await;
            }
        }
    }

    pub async fn reply_ok(&self, message: StarMessage) {
        let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
        self.star_tx
            .send(StarCommand::SendProtoMessage(proto))
            .await;
    }

    pub async fn handle_ok_response<R>(
        &self,
        rx: oneshot::Receiver<Result<R, Fail>>,
        message: StarMessage,
    ) where
        R: Send + Sync + 'static,
    {
        let star_tx = self.star_tx.clone();
        tokio::spawn(async move {
            let reply = match rx.await {
                Ok(result) => match result {
                    Ok(ok) => SimpleReply::Ok(Reply::Empty),
                    Err(fail) => SimpleReply::Fail(fail),
                },
                Err(err) => SimpleReply::Fail(Fail::ChannelRecvErr),
            };
            let proto = message.reply(StarMessagePayload::Reply(reply));
            star_tx.send(StarCommand::SendProtoMessage(proto)).await;
        });
    }
    pub async fn send_and_get_ok_result(
        &self,
        proto: ProtoStarMessage,
        tx: oneshot::Sender<Result<(), Fail>>,
    ) {
        let result = proto.get_ok_result().await;
        tokio::spawn(async move {
            match tokio::time::timeout(Duration::from_secs(30), result).await {
                Ok(result) => match result {
                    Ok(payload) => match payload {
                        StarMessagePayload::Reply(reply) => match reply {
                            SimpleReply::Ok(reply) => {
                                tx.send(Result::Ok(()));
                            }
                            SimpleReply::Fail(fail) => {
                                tx.send(Result::Err(fail));
                            }
                            _ => {
                                tx.send(Result::Err(Fail::expected("SimpleReply::Ok(reply)")));
                            }
                        },
                        _ => {
                            tx.send(Result::Err(Fail::expected("StarMessagePayload::Reply(_)")));
                        }
                    },
                    Err(error) => {
                        tx.send(Result::Err(Fail::expected("Result::Ok(_)")) );
                    }
                },
                Err(elapsed) => {
                    tx.send(Result::Err(Fail::Timeout));
                }
            };
        });
        self.star_tx
            .send(StarCommand::SendProtoMessage(proto))
            .await;
    }

    pub async fn send_and_get_result(
        &self,
        proto: ProtoStarMessage,
        tx: oneshot::Sender<Result<Reply, Fail>>,
    ) {
        let result = proto.get_ok_result().await;
        tokio::spawn(async move {
            match tokio::time::timeout(Duration::from_secs(30), result).await {
                Ok(result) => match result {
                    Ok(payload) => match payload {
                        StarMessagePayload::Reply(reply) => match reply {
                            SimpleReply::Ok(reply) => {
                                tx.send(Result::Ok(reply));
                            }
                            SimpleReply::Fail(fail) => {
                                tx.send(Result::Err(fail));
                            }
                            _ => {
                                tx.send(Result::Err(Fail::expected("SimpleReply::Ok(_)")));
                            }
                        },
                        _ => {
                            tx.send(Result::Err(Fail::expected("SimpleReply::Ok(_)")));
                        }
                    },
                    Err(error) => {
                        tx.send(Result::Err(Fail::Error(error.to_string())));
                    }
                },
                Err(elapsed) => {
                    tx.send(Result::Err(Fail::Timeout));
                }
            };
        });
        self.star_tx
            .send(StarCommand::SendProtoMessage(proto))
            .await;
    }
}

impl StarComm {
    pub async fn reply_rx(&self, message: StarMessage, rx: oneshot::Receiver<Result<Reply, Fail>>) {
        let star_tx = self.star_tx.clone();
        tokio::spawn(async move {
            match tokio::time::timeout(Duration::from_secs(5), rx).await {
                Ok(result) => match result {
                    Ok(result) => match result {
                        Ok(reply) => {
                            let proto =
                                message.reply(StarMessagePayload::Reply(SimpleReply::Ok(reply)));
                            star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                        }
                        Err(fail) => {
                            let proto =
                                message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                            star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                        }
                    },
                    Err(_) => {
                        let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(
                            Fail::Error("Internal Error".to_string()),
                        )));
                        star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                    }
                },
                Err(err) => {
                    let proto =
                        message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Timeout)));
                    star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                }
            }
        });
    }

    pub async fn simple_reply(&self, message: StarMessage, reply: SimpleReply) {
        let proto = message.reply(StarMessagePayload::Reply(reply));
        self.send(proto).await;
    }

    pub async fn reply_result_empty_rx(
        &self,
        message: StarMessage,
        rx: oneshot::Receiver<Result<(), Fail>>,
    ) {
        let star_tx = self.star_tx.clone();
        tokio::spawn(async move {
            match rx.await {
                Ok(result) => match result {
                    Ok(_) => {
                        let proto =
                            message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
                        star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                    }
                    Err(fail) => {
                        let proto =
                            message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                        star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                    }
                },
                Err(fail) => {
                    let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(
                        Fail::expected("Ok(result)"),
                    )));
                    star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                }
            }
        });
    }

    pub async fn reply_result_empty(&self, message: StarMessage, result: Result<(), Fail>) {
        match result {
            Ok(reply) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
                self.star_tx
                    .send(StarCommand::SendProtoMessage(proto))
                    .await;
            }
            Err(fail) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                self.star_tx
                    .send(StarCommand::SendProtoMessage(proto))
                    .await;
            }
        }
    }

    pub async fn reply_result(&self, message: StarMessage, result: Result<Reply, Fail>) {
        match result {
            Ok(reply) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(reply)));
                self.star_tx
                    .send(StarCommand::SendProtoMessage(proto))
                    .await;
            }
            Err(fail) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                self.star_tx
                    .send(StarCommand::SendProtoMessage(proto))
                    .await;
            }
        }
    }

    /*
    pub async fn relay( &self, message: StarMessage, rx: oneshot::Receiver<StarMessagePayload> )
    {
        self.relay_trigger(message,rx, Option::None, Option::None).await;
    }

    pub async fn relay_trigger(&self, message: StarMessage, rx: oneshot::Receiver<StarMessagePayload>, trigger: Option<StarVariantCommand>, trigger_reply: Option<Reply> )
    {
        let star_tx = self.star_tx.clone();
        tokio::spawn(async move {
            let proto = match rx.await
            {
                Ok(payload) => {
                    if let Option::Some(command) = trigger {
                        variant_tx.send(command).await;
                    }
                    Self::relay_payload(message,payload,trigger_reply)
                }
                Err(err) => {
                    message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error("rx recv error".to_string()))))
                }
            };
            star_tx.send( StarCommand::SendProtoMessage(proto)).await;
        });
    }

     */

    fn relay_payload(
        message: StarMessage,
        payload: StarMessagePayload,
        trigger_reply: Option<Reply>,
    ) -> ProtoStarMessage {
        match payload {
            StarMessagePayload::Reply(payload_reply) => match payload_reply {
                SimpleReply::Ok(_) => match trigger_reply {
                    None => message.reply(StarMessagePayload::Reply(payload_reply)),
                    Some(reply) => message.reply(StarMessagePayload::Reply(SimpleReply::Ok(reply))),
                },
                _ => message.reply(StarMessagePayload::Reply(payload_reply)),
            },
            _ => message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error(
                "unexpected response".to_string(),
            )))),
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

    async fn timeout<X>( rx: oneshot::Receiver<X>) -> Result<X,Fail> {
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

        match Self::timeout( rx ).await? {
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
        match Self::timeout( rx).await? {
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
        match Self::timeout( rx).await? {
            ResourceRegistryResult::Resource(resource) => Ok(resource),
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

pub struct LogId<T>(T);

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
impl ToString for LogId<String> {
    fn to_string(&self) -> String {
        self.0.clone()
    }
}

impl ToString for LogId<&Star> {
    fn to_string(&self) -> String {
        format!("{}", self.0.skel.info.to_string())
    }
}

impl ToString for LogId<&mut Star> {
    fn to_string(&self) -> String {
        format!("{}", self.0.skel.info.to_string())
    }
}

impl ToString for LogId<&StarMessage> {
    fn to_string(&self) -> String {
        format!("<Message>[{}]", self.0.id.to_string())
    }
}

impl ToString for LogId<&ProtoStarMessage> {
    fn to_string(&self) -> String {
        "<proto>".to_string()
    }
}
