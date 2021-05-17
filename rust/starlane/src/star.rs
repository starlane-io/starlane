use std::{cmp, fmt};
use std::borrow::Borrow;
use std::cell::Cell;
use std::cmp::{min, Ordering};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;
use std::future::Future;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicI32, AtomicI64, AtomicU64};

use futures::future::{BoxFuture, join_all, Map};
use futures::future::select_all;
use futures::FutureExt;
use futures::prelude::future::FusedFuture;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot};
use tokio::sync::broadcast::error::{RecvError, SendError};
use tokio::sync::mpsc;
use tokio::sync::mpsc::Receiver;
use tokio::time::{Duration, Instant, timeout};
use tokio::time::error::Elapsed;
use url::Url;

use actor_host::ServerStarVariant;

use crate::actor::{ActorKey, ActorKind, ActorWatcher, ResourceMessage, ResourceMessageWrapper};
use crate::app::{AppCommandKind, AppController, AppCreateController, AppMeta, AppSpecific, AppLocation, AppCommand};
use crate::core::StarCoreCommand;
use crate::crypt::{Encrypted, HashEncrypted, HashId, PublicKey, UniqueHash};
use crate::error::Error;
use crate::frame::{ActorBind, ActorEvent, ActorLocationReport, ActorLocationRequest, ActorLookup, ApplicationSupervisorReport, ServerAppPayload, AppNotifyCreated, AppSupervisorLocationRequest, Event, Frame, ProtoFrame, Rejection, SpaceReply, SequenceMessage, SpaceMessage, SpacePayload, StarMessage, StarMessageAck, StarMessagePayload, StarPattern, StarWind, Watch, WatchInfo, WindAction, WindDown, WindHit, WindResults, WindUp, Reply, SimpleReply, AppPayload, ResourceManagerAction, ResourceHostAction, FromReply};
use crate::frame::WindAction::SearchHits;
use crate::id::{Id, IdSeq};
use crate::keys::{AppKey, MessageId, SpaceKey, UserKey, ResourceKey, GatheringKey};
use crate::resource::{Labels, ResourceRegistration, Selector, ResourceAssign, ResourceRegistryCommand, ResourceRegistryResult, Registry, ResourceType, ResourceLocation, ResourceManagerKey, ResourceBinding, ResourceAddress, ResourceRegistryAction, Resource, FieldSelection};
use crate::lane::{ConnectionInfo, ConnectorController, Lane, LaneCommand, LaneMeta, OutgoingLane, TunnelConnector, TunnelConnectorFactory};
use crate::logger::{Flag, Flags, Log, Logger, ProtoStarLog, ProtoStarLogPayload, StarFlag};
use crate::message::{MessageExpect, MessageExpectWait, MessageReplyTracker, MessageResult, MessageUpdate, ProtoMessage, StarMessageDeliveryInsurance, TrackerJob, Fail};
use crate::proto::{PlaceholderKernel, ProtoStar, ProtoTunnel};
use crate::space::{CreateAppControllerFail, RemoteSpaceCommand, RemoteSpaceCommandKind, SpaceController};
use crate::star::central::CentralStarVariant;
use crate::star::app_host::{SupervisorVariant};
use crate::permissions::{Authentication, AuthToken, AuthTokenSource, Credentials};
use tokio::sync::oneshot::Sender;
use std::str::FromStr;
use crate::star::space::SpaceVariant;
use crate::frame::ResourceHostAction::SliceAssign;
use std::iter::FromIterator;
use crate::star::pledge::{StarHandleBacking, StarHandle, Satisfaction};

pub mod central;
pub mod app_host;
pub mod actor_host;
pub mod filestore;
pub mod pledge;
pub mod space;
pub mod common;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize,Hash)]
pub enum StarKind
{
    Central,
    SpaceHost,
    Mesh,
    AppHost,
    ActorHost,
    FileStore,
    Gateway,
    Link,
    Client
}

impl StarKind
{
    pub fn is_resource_manager(&self)->bool{
        match self{
            StarKind::Central => true,
            StarKind::SpaceHost => true,
            StarKind::Mesh => false,
            StarKind::AppHost => true,
            StarKind::ActorHost => false,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => false
        }
    }

    pub fn is_resource_host(&self)->bool{
        match self{
            StarKind::Central => false,
            StarKind::SpaceHost => true,
            StarKind::Mesh => false,
            StarKind::AppHost => true,
            StarKind::ActorHost => true,
            StarKind::FileStore => true,
            StarKind::Gateway => false,
            StarKind::Link => false,
            StarKind::Client => true
        }
    }

    pub fn handles(&self)->HashSet<StarKind>{
        HashSet::from_iter(match self {
            StarKind::Central => vec![StarKind::AppHost, StarKind::SpaceHost],
            StarKind::SpaceHost => vec![StarKind::FileStore],
            StarKind::Mesh => vec![],
            StarKind::AppHost => vec![StarKind::ActorHost, StarKind::FileStore],
            StarKind::ActorHost => vec![],
            StarKind::FileStore => vec![],
            StarKind::Gateway => vec![],
            StarKind::Link => vec![],
            StarKind::Client => vec![]
        }.iter().cloned())
    }


    pub fn manages(&self)->HashSet<ResourceType>{
        HashSet::from_iter(match self {
            StarKind::Central => vec![ResourceType::Space],
            StarKind::SpaceHost => vec![ResourceType::SubSpace, ResourceType::App, ResourceType::FileSystem],
            StarKind::Mesh => vec![],
            StarKind::AppHost => vec![ResourceType::Actor, ResourceType::FileSystem],
            StarKind::ActorHost => vec![],
            StarKind::FileStore => vec![ResourceType::File],
            StarKind::Gateway => vec![],
            StarKind::Link => vec![],
            StarKind::Client => vec![]
        }.iter().cloned())
    }

    pub fn hosts(&self)->HashSet<ResourceType>{
        HashSet::from_iter(match self {
            StarKind::Central => vec![],
            StarKind::SpaceHost => vec![ResourceType::Space, ResourceType::SubSpace],
            StarKind::Mesh => vec![],
            StarKind::AppHost => vec![ResourceType::App],
            StarKind::ActorHost => vec![ResourceType::Actor],
            StarKind::FileStore => vec![ResourceType::FileSystem,ResourceType::File],
            StarKind::Gateway => vec![],
            StarKind::Link => vec![],
            StarKind::Client => vec![ResourceType::Actor]
        }.iter().cloned())
    }
}

impl FromStr for StarKind{

    type Err = ();

    fn from_str(input: &str) -> Result<StarKind, Self::Err> {
        match input {
            "Central"  => Ok(StarKind::Central),
            "Mesh"  => Ok(StarKind::Mesh),
            "AppHost"  => Ok(StarKind::AppHost),
            "ActorHost"  => Ok(StarKind::ActorHost),
            "FileStore"  => Ok(StarKind::FileStore),
            "Gateway"  => Ok(StarKind::Gateway),
            "Link"  => Ok(StarKind::Link),
            "Client"  => Ok(StarKind::Client),
            "SpaceHost"  => Ok(StarKind::SpaceHost),
            _      => Err(()),
        }
    }
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ServerKindExt
{
   pub name: String
}

impl ServerKindExt
{
    pub fn new( name: String ) -> Self
    {
        ServerKindExt{
            name: name
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct StoreKindExt
{
    pub name: String
}

impl StoreKindExt
{
    pub fn new( name: String ) -> Self
    {
        StoreKindExt{
            name: name
        }
    }
}


impl StarKind
{
    pub fn is_central(&self)->bool
    {
        if let StarKind::Central = self
        {
            return true;
        }
        else {
            return false;
        }
    }

    pub fn is_supervisor(&self)->bool
    {
        if let StarKind::AppHost = self
        {
            return true;
        }
        else {
            return false;
        }
    }


    pub fn is_client(&self)->bool
    {
        if let StarKind::Client = self
        {
            return true;
        }
        else {
            return false;
        }
    }

    pub fn central_result(&self)->Result<(),Error>
    {
        if let StarKind::Central = self
        {
            Ok(())
        }
        else {
            Err("not central".into())
        }
    }

    pub fn supervisor_result(&self)->Result<(),Error>
    {
        if let StarKind::AppHost = self
        {
            Ok(())
        }
        else {
            Err("not supervisor".into())
        }
    }

    pub fn server_result(&self)->Result<(),Error>
    {
        if let StarKind::ActorHost = self
        {
            Ok(())
        }
        else {
            Err("not server".into())
        }
    }

    pub fn client_result(&self)->Result<(),Error>
    {
        if let StarKind::Client = self
        {
            Ok(())
        }
        else {
            Err("not client".into())
        }
    }



    pub fn relay(&self) ->bool
    {
        match self
        {
            StarKind::Central => false,
            StarKind::Mesh => true,
            StarKind::AppHost => false,
            StarKind::ActorHost => true,
            StarKind::Gateway => true,
            StarKind::Client => true,
            StarKind::Link => true,
            StarKind::FileStore => false,
            StarKind::SpaceHost => false
        }
    }
}

impl fmt::Display for StarKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
        match self{
            StarKind::Central => "Central".to_string(),
            StarKind::Mesh => "Mesh".to_string(),
            StarKind::AppHost => "AppHost".to_string(),
            StarKind::ActorHost => "ActorHost".to_string(),
            StarKind::FileStore => "FileStore".to_string(),
            StarKind::Gateway => "Gateway".to_string(),
            StarKind::Link => "Link".to_string(),
            StarKind::Client => "Client".to_string(),
            StarKind::SpaceHost => "SpaceHost".to_string()
        })
    }
}



impl fmt::Display for ActorLookup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self{
            ActorLookup::Key(entity) => format!("Key({})", entity).to_string(),
        };
        write!(f, "{}",r)
    }
}

pub static MAX_HOPS: usize = 32;

pub struct Star
{
    skel: StarSkel,
    star_rx: mpsc::Receiver<StarCommand>,
    core_tx: mpsc::Sender<StarCoreCommand>,
    lanes: HashMap<StarKey, LaneMeta>,
    connector_ctrls: Vec<ConnectorController>,
    transactions: HashMap<u64,Box<dyn Transaction>>,
    frame_hold: FrameHold,
    watches: HashMap<ActorKey,HashMap<Id,StarWatchInfo>>,
    messages_received: HashMap<MessageId,Instant>,
    message_reply_trackers: HashMap<MessageId, MessageReplyTracker>,
    star_subgraph_expansion_seq: AtomicU64,
    resource_locations: LruCache<ResourceKey,ResourceLocation>,
    resource_address_to_key: LruCache<ResourceAddress,ResourceKey>
}

impl Star
{

    pub fn from_proto(data: StarSkel,
                      star_rx: mpsc::Receiver<StarCommand>,
                      core_tx: mpsc::Sender<StarCoreCommand>,
                      lanes: HashMap<StarKey,LaneMeta>,
                      connector_ctrls: Vec<ConnectorController>,
                      frame_hold: FrameHold ) ->Self

    {
        Star{
            skel: data,
            star_rx: star_rx,
            core_tx: core_tx,
            lanes: lanes,
            connector_ctrls: connector_ctrls,
            transactions: HashMap::new(),
            frame_hold: frame_hold,
            watches: HashMap::new(),
            messages_received: HashMap::new(),
            message_reply_trackers: HashMap::new(),
            star_subgraph_expansion_seq: AtomicU64::new(0),
            resource_locations: LruCache::new(64*1024 ),
            resource_address_to_key: LruCache::new(16*1024 ),
        }
    }

    pub fn has_resource(&self, key: &ResourceKey) -> bool
    {
        unimplemented!()
    }


    pub async fn run(mut self)
    {

        loop {
            let mut futures = vec!();
            let mut lanes = vec!();

            {
                for (key, mut lane) in &mut self.lanes
                {
                    futures.push(lane.lane.incoming.recv().boxed());
                    lanes.push(key.clone())
                }
            }


            futures.push( self.star_rx.recv().boxed());

            let (command,index,_) = select_all(futures).await;

            if let Some(command) = command
            {
                match command{

                    StarCommand::Init => {
                        self.skel.variant_tx.send(StarVariantCommand::Init).await;
                        self.init().await;
                    }
                    StarCommand::SetFlags(set_flags ) => {
                       self.skel.flags= set_flags.flags;
                       set_flags.tx.send(());
                    }
                    StarCommand::AddLane(lane) => {
                        if let Some(remote_star)=lane.remote_star.as_ref()
                        {
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));
                        }
                        else {
                            eprintln!("for star remote star must be set");
                         }
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::AddResourceLocation(add_resource_location) => {
                        self.resource_locations.put(add_resource_location.resource_location.key.clone(), add_resource_location.resource_location.clone() );
                        add_resource_location.tx.send( ()).await;
                    }
                    StarCommand::SendProtoMessage(message) => {
                        self.send_proto_message(message).await;
                    }
                    StarCommand::ReleaseHold(star) => {
                        if let Option::Some(frames) = self.frame_hold.release(&star)
                        {
                            let lane = self.lane_with_shortest_path_to_star(&star);
                            if let Option::Some(lane)=lane
                            {
                                for frame in frames
                                {
                                    lane.lane.outgoing.tx.send(LaneCommand::Frame(frame)).await;
                                }
                            }
                            else {
                                eprintln!("release hold called on star that is not ready!")
                            }
                       }
                    }
                    StarCommand::GetSpaceController(get)=>{
                        let (tx,rx) = mpsc::channel(16);
                        let star_tx = self.skel.star_tx.clone();
                        let user = get.auth.user.clone();
                        let ctrl = SpaceController::new( get.auth.user, tx );
                        tokio::spawn( async move {
                            let mut rx = rx;
                            while let Option::Some(command) = rx.recv().await {
                                if user == command.user
                                {
                                    star_tx.send( StarCommand::SpaceCommand(command)).await;
                                }
                                else {
                                    rx.close();
                                }
                            }
                        } );
                        get.tx.send(Ok(ctrl) );
                    }
                    StarCommand::SpaceCommand(command)=>{
                        self.on_remote_space_command(command).await;
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
                    StarCommand::WindInit(search) =>
                    {
                        self.do_wind(search).await;
                    }
                    StarCommand::WindCommit(commit) =>
                    {
                        for lane in commit.result.lane_hits.keys()
                        {
                            let hits = commit.result.lane_hits.get(lane).unwrap();
                            for (star,size) in hits
                            {
                                self.lanes.get_mut(lane).unwrap().star_paths.put(star.clone(),size.clone() );
                            }
                        }
                        commit.tx.send( commit.result );
                    }
                    StarCommand::WindDown(result) => {
                        let lane = result.hops.last().unwrap();
                        self.send_frame( lane.clone(), Frame::StarWind(StarWind::Down(result))).await;
                    }
                    StarCommand::Frame(frame) => {
                        let lane_key = lanes.get(index);
                        self.process_frame(frame, lane_key ).await;
                    }
                    StarCommand::ForwardFrame(forward) => {
                        self.send_frame( forward.to.clone(), forward.frame ).await;
                    }
                    StarCommand::ResourceMessage(request) => {
                        self.message_to_resource(request).await;
                    }
                    StarCommand::ResourceLocate(request) => {
                        self.locate_resource(request).await;
                    }
                    StarCommand::SetResourceLocation(set) => {
                        self.resource_locations.put( set.payload.key.clone(), set.payload.clone() );
                        set.commit();
                    }
                    StarCommand::SetResourceBind(set) => {
                        self.resource_address_to_key.put( set.payload.address.clone(), set.payload.key.clone() );
                        set.commit();
                    }
                    StarCommand::Diagnose(diagnose) => {
                        self.diagnose(diagnose).await;
                    }
                    _ => {
                        eprintln!("cannot process command: {}",command);
                    }
                }
            }
            else
            {
                println!("command_rx has been disconnected");
                return;
            }

        }
    }

    async fn init(&mut self){
        self.init_handles().await;
    }

    async fn init_handles(&mut self) {
        if let Option::Some(star_handler) = &self.skel.star_handler {
            for kind in self.skel.info.kind.handles() {
                let (search, rx) = Wind::new(StarPattern::StarKind(kind.clone()), WindAction::SearchHits);
                self.skel.star_tx.send(StarCommand::WindInit(search)).await;
                let star_handler = star_handler.clone();
                let kind = kind.clone();
                tokio::spawn( async move {
                    let result = tokio::time::timeout(Duration::from_secs(5), rx).await;
                    if let Ok(Ok(hits)) = result
                    {
                        for (star, hops) in hits.hits {
                            let handle = StarHandle {
                                key: star,
                                kind: StarKind::FileStore,
                                hops: Option::Some(hops)
                            };
                            star_handler.add_star_handle(handle).await;
                        }
                    } else {
                        eprintln!("error encountered when attempting to get a handle for: {}", kind );
                    }
                });
            }
        }
    }

    async fn message_to_resource(&mut self, request: Request<ResourceMessageWrapper,Fail> ) {
       if let Option::Some(location) = self.resource_locations.get(&request.payload.message.to.key )
       {
           let mut proto = ProtoMessage::new();
           proto.to = Option::Some(location.host.clone());
           proto.payload = StarMessagePayload::ResourceHost(ResourceHostAction::Message(request.payload.message));
           self.send_proto_message(proto).await;
       }
    }

    async fn locate_resource(&mut self, request: Request<ResourceKey,ResourceLocation> )
    {
        if let Option::Some(location) = self.resource_locations.get(&request.payload )
        {
            request.tx.send(Result::Ok(location.clone()) );
        }
        else
        {
            match request.payload.manager()
            {
                ResourceManagerKey::Central => {
                    self.request_resource_location_from_star(request, StarKey::central() ).await
                }
                ResourceManagerKey::Key(parent) => {
                    if let Option::Some(parent_resource_star ) = self.resource_locations.get(&parent).cloned() {
                        self.request_resource_location_from_star(request, parent_resource_star.host.clone() ).await
                    } else {
                        let (new_locate_request,rx) = Request::new(parent );
                        self.skel.star_tx.send(StarCommand::ResourceLocate(new_locate_request)).await;
                        tokio::spawn(async move{
                            let reply = rx.await;
                            if let Result::Ok(result) = reply
                            {
                                request.tx.send(result);
                            } else {
                                request.tx.send(Err(Fail::Unexpected));
                            }
                        });
                    }
                }
            }
        }
    }

    async fn request_resource_location_from_star(&mut self, locate: Request<ResourceKey,ResourceLocation>, star: StarKey )
    {
        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(star);
        proto.payload = StarMessagePayload::ResourceManager(ResourceManagerAction::Find(locate.payload.clone()));
        let reply = proto.get_ok_result().await;
        self.send_proto_message(proto).await;
        let star_tx = self.skel.star_tx.clone();
        tokio::spawn( async move {
            let result = reply.await;
            if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Location(location)))) = result {
                let (set,rx) = Set::new(location);
                star_tx.send( StarCommand::SetResourceLocation(set)).await;
                tokio::spawn( async move {
                    if let Result::Ok(location) = rx.await {
                        locate.tx.send( Ok(location) );
                    } else {
                        locate.tx.send( Err(Fail::Unexpected) );
                    }
                });
            }
            else if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Fail(fail))) = result {
                locate.tx.send( Err(fail) );
            }
            else
            {
                locate.tx.send( Err(Fail::Unexpected) );
            }
    } );
    }

    async fn request_resource_key_from_star(&mut self, request: Request<ResourceAddress,ResourceKey>, star: StarKey )
    {
        let mut proto = ProtoMessage::new();
        proto.to = Option::Some(star);
        proto.payload = StarMessagePayload::ResourceManager(ResourceManagerAction::GetKey(request.payload.clone()));
        let reply = proto.get_ok_result().await;
        self.send_proto_message(proto).await;
        let star_tx = self.skel.star_tx.clone();
        tokio::spawn( async move {
            let result = reply.await;
            if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Key(key)))) = result {
                let bind = ResourceBinding{
                    address: request.payload.clone(),
                    key: key
                };
                let (set,rx) = Set::new(bind);
                star_tx.send( StarCommand::SetResourceBind(set)).await;
                tokio::spawn( async move {
                    if let Result::Ok(bind) = rx.await {
                        request.tx.send( Ok(bind.key) );
                    } else {
                        request.tx.send( Err(Fail::Unexpected) );
                    }
                });
            }
            else if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Fail(fail))) = result {
                request.tx.send( Err(fail) );
            }
            else
            {
                request.tx.send( Err(Fail::Unexpected) );
            }
        } );
    }



    async fn send_proto_message( &mut self, proto: ProtoMessage )
    {

        if let Err(errors) = proto.validate() {
                eprintln!("protomessage is not valid cannot send: {}", errors );
            return;
        }

        let id = MessageId::new_v4();

        let message = StarMessage{
            id: id,
            from: self.skel.info.star.clone(),
            to: proto.to.unwrap(),
            //transaction: proto.transaction,
            payload: proto.payload,
            reply_to: proto.reply_to
        };

        if message.to == self.skel.info.star
        {
            eprintln!("star {} kind {} cannot send a proto message to itself, payload: {} ", self.skel.info.star, self.skel.info.kind, message.payload );
        }
        else {
            let delivery = StarMessageDeliveryInsurance::with_txrx(message, proto.expect, proto.tx.clone(), proto.tx.subscribe() );
            self.message(delivery).await;
        }
    }

    async fn on_remote_space_command(&mut self, command: RemoteSpaceCommand)
    {
        unimplemented!();
        match command.kind
        {
            RemoteSpaceCommandKind::AppCreateController(_) => {}
            RemoteSpaceCommandKind::AppSelect(_) => {}
        }
    }

    async fn on_app_command( &mut self, command: AppPayload )
    {
        println!("on_app_command!");
    }


   async fn search_for_star( &mut self, star: StarKey, tx: oneshot::Sender<WindHits> )
   {
        let wind = Wind {
            pattern: StarPattern::StarKey(star),
            tx: tx,
            max_hops: 16,
            action: WindAction::SearchHits
        };
        self.skel.star_tx.send( StarCommand::WindInit(wind) ).await;
    }

    async fn do_wind(&mut self, wind: Wind)
    {
        let tx = wind.tx;
        let wind_up = WindUp::new(self.skel.info.star.clone(), wind.pattern, wind.action );
        self.do_wind_up(wind_up, tx, Option::None).await;
    }

    async fn do_wind_up(&mut self, mut wind: WindUp, tx: oneshot::Sender<WindHits>, exclude: Option<HashSet<StarKey>> )
    {
        let tid = self.skel.sequence.fetch_add(1, std::sync::atomic::Ordering::Relaxed );

        let num_excludes:usize = match &exclude
        {
            None => 0,
            Some(exclude) => exclude.len()
        };

        let local_hit = match wind.pattern.is_match(&self.skel.info){
            true => Option::Some(self.skel.info.star.clone()),
            false => Option::None
        };

        let transaction = Box::new(StarSearchTransaction::new(wind.pattern.clone(), self.skel.star_tx.clone(), tx, self.lanes.len()-num_excludes, local_hit ));
        self.transactions.insert(tid.clone(), transaction );

        wind.transactions.push(tid.clone());
        wind.hops.push( self.skel.info.star.clone() );

        self.broadcast_excluding(Frame::StarWind(StarWind::Up(wind)), &exclude ).await;
    }




    async fn on_wind_up_hop(&mut self, mut wind_up: WindUp, lane_key: StarKey )
    {

        if wind_up.pattern.is_match(&self.skel.info)
        {

            if wind_up.pattern.is_single_match()
            {

                let hit = WindHit {
                    star: self.skel.info.star.clone(),
                    hops: wind_up.hops.len() + 1
                };

                match wind_up.action.update(vec![hit], WindResults::None )
                {
                    Ok(result) => {

                        let wind_down = WindDown{
                            missed: None,
                            hops: wind_up.hops.clone(),
                            transactions: wind_up.transactions.clone(),
                            wind_up: wind_up,
                            result: result
                        };

                        let wind = Frame::StarWind( StarWind::Down(wind_down) );

                        let lane = self.lanes.get_mut(&lane_key).unwrap();
                        lane.lane.outgoing.tx.send(LaneCommand::Frame(wind)).await;

                    }
                    Err(error) => {
                        eprintln!("error when attempting to update wind_down results {}",error);
                    }
                }

                return;
            }
            else {
                // need to create a new transaction here which gathers 'self' as a HIT
            }
        }

        let hit = wind_up.pattern.is_match(&self.skel.info);

        if wind_up.hops.len()+1 > min(wind_up.max_hops,MAX_HOPS) || self.lanes.len() <= 1 || !self.skel.info.kind.relay()
        {

            let hits = match hit
            {
                true => {
                    vec![WindHit {star: self.skel.info.star.clone(), hops: wind_up.hops.len().clone()+1 }]
                }
                false => {
                    vec!()
                }
            };

            match wind_up.action.update(hits, WindResults::None )
            {
                Ok(result) => {

                    let wind_down = WindDown{
                        missed: None,
                        hops: wind_up.hops.clone(),
                        transactions: wind_up.transactions.clone(),
                        wind_up: wind_up,
                        result: result
                    };

                    let wind = Frame::StarWind( StarWind::Down(wind_down) );

                    let lane = self.lanes.get_mut(&lane_key).unwrap();
                    lane.lane.outgoing.tx.send(LaneCommand::Frame(wind)).await;
                }
                Err(error) => {
                    eprintln!("error encountered when trying to update WindResult: {}",error );
                }
            }

            return;
        }

        let mut exclude = HashSet::new();
        exclude.insert( lane_key );

        let (tx,rx) = oneshot::channel();

        let relay_wind_up = wind_up.clone();

        let command_tx = self.skel.star_tx.clone();
        self.do_wind_up(relay_wind_up, tx, Option::Some(exclude) ).await;

        tokio::spawn( async move {
//            result.hits.iter().map(|(star,hops)| SearchHit{ star: star.clone(), hops: hops.clone()+1} ).collect()

            let wind_result = rx.await;

            match wind_result{
                Ok(wind_result) => {
                let hits = wind_result.hits.iter().map(|(star,hops)| WindHit { star: star.clone(), hops: hops.clone()+1} ).collect();
                    match wind_up.action.update(hits, WindResults::None)
                    {
                        Ok(result) => {
                            let mut wind_down = WindDown {
                                missed: None,
                                hops: wind_up.hops.clone(),
                                wind_up: wind_up.clone(),
                                transactions: wind_up.transactions.clone(),
                                result: result
                            };
                            command_tx.send( StarCommand::WindDown(wind_down) ).await;
                        }
                        Err(error) => {
                            eprintln!("{}",error);
                        }
                    }
                }
                Err(error) => {
                    eprintln!("{}",error);
                }
            }
        } );

    }

    pub fn star_key(&self)->&StarKey
    {
        &self.skel.info.star
    }

    pub fn star_tx(&self)->mpsc::Sender<StarCommand>
    {
        self.skel.star_tx.clone()
    }

    async fn broadcast(&mut self,  frame: Frame )
    {
        self.broadcast_excluding(frame, &Option::None ).await;
    }

    async fn broadcast_excluding(&mut self,  frame: Frame, exclude: &Option<HashSet<StarKey>> )
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
            self.send_frame(star, frame.clone()).await;
        }
    }

    async fn message(&mut self, delivery: StarMessageDeliveryInsurance)
    {

        let message = delivery.message.clone();
        if !delivery.message.payload.is_ack()
        {
            let tracker = MessageReplyTracker {
                reply_to: delivery.message.id.clone(),
                tx: delivery.tx.clone()
            };

            self.message_reply_trackers.insert(delivery.message.id.clone(), tracker);

            let star_tx = self.skel.star_tx.clone();
            tokio::spawn( async move {
                let mut delivery = delivery;
                delivery.retries = delivery.expect.retries();

                loop
                {
                    let wait = if delivery.retries == 0 && delivery.expect.retry_forever(){
                        // take a 2 minute break if retry_forever to be sure that all messages have expired
                        120 as u64
                    }
                    else {
                        delivery.expect.wait_seconds()
                    };
                    let result = timeout(Duration::from_secs(wait ) ,delivery.rx.recv() ).await;
                    match result{
                         Ok(result) => {
                             match result
                             {
                                 Ok(update) => {
                                     match update
                                     {
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
                                 if delivery.expect.retry_forever()
                                 {
                                     // we have to keep trying with a new message Id since the old one is now expired
                                     let proto = delivery.message.resubmit( delivery.expect, delivery.tx.clone(), delivery.tx.subscribe() );
                                     star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                                     break;
                                 }
                                 else {
                                     // out of retries, this
                                     delivery.tx.send(MessageUpdate::Result(MessageResult::Timeout));
                                     break;
                                 }
                             }
                             else {
                                 // we resend the message and hope it arrives this time
                                 star_tx.send( StarCommand::ForwardFrame( ForwardFrame{ to: delivery.message.to.clone(), frame: Frame::StarMessage(delivery.message.clone()) }  )).await;
                             }
                         }
                     }
                 }
            });
        }
        self.send_frame(message.to.clone(), Frame::StarMessage(message) ).await;

    }

    async fn send_frame(&mut self, star: StarKey, frame: Frame )
    {
        let lane = self.lane_with_shortest_path_to_star(&star);
        if let Option::Some(lane)=lane
        {
            lane.lane.outgoing.tx.send( LaneCommand::Frame(frame) ).await;
        }
        else {
            self.frame_hold.add( &star, frame );
            let (tx,rx) = oneshot::channel();

            self.search_for_star(star.clone(), tx ).await;
            let command_tx = self.skel.star_tx.clone();
            tokio::spawn(async move {

                match rx.await
                {
                    Ok(_) => {
                        command_tx.send( StarCommand::ReleaseHold(star) ).await;
                    }
                    Err(error) => {
                        eprintln!("RELEASE HOLD RX ERROR : {}",error);
                    }
                }
            });
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

        self.logger.log(StarLog::StarSearchInitialized(search.clone()));
        for (star,lane) in &self.lanes
        {
            lane.lane.outgoing.tx.send( LaneCommand::Frame( Frame::StarSearch(search.clone()))).await;
        }
    }*/

    async fn on_wind_down(&mut self, mut search_result: WindDown, lane_key: StarKey )
    {
//        println!("ON STAR SEARCH RESULTS");
    }
    /*
    async fn on_star_search_result( &mut self, mut search_result: StarSearchResultInner, lane_key: StarKey )
    {

        self.logger.log(StarLog::StarSearchResult(search_result.clone()));
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

    async fn process_transactions( &mut self, frame: &Frame, lane_key: Option<&StarKey> )
    {
        let tid = match frame
        {
/*            Frame::StarMessage(message) => {
                message.transaction
            },

 */
            Frame::StarWind(wind) => {
                match wind{
                    StarWind::Down(wind_down) => {
                        wind_down.transactions.last().cloned()
                    }
                    _ => Option::None
                }
            }
            _ => Option::None
        };

        if let Option::Some(tid) = tid
        {
            let transaction = self.transactions.get_mut(&tid);
            if let Option::Some(transaction) = transaction
            {
                let lane = match lane_key
                {
                    None => Option::None,
                    Some(lane_key) => {
                        self.lanes.get_mut(lane_key)
                    }
                };


                match transaction.on_frame(frame,lane, &mut self.skel.star_tx).await
                {
                    TransactionResult::Continue => {}
                    TransactionResult::Done => {
                        self.transactions.remove(&tid);
                    }
                }
            }
        }
    }

    async fn process_message_reply( &mut self, message: &StarMessage )
    {
        if message.reply_to.is_some() && self.message_reply_trackers.contains_key(message.reply_to.as_ref().unwrap()) {
            if let Some(tracker) = self.message_reply_trackers.get(message.reply_to.as_ref().unwrap()) {
                if let TrackerJob::Done = tracker.on_message(message)
                {
                    self.message_reply_trackers.remove(message.reply_to.as_ref().unwrap());
                }
            }
        }
    }

    async fn process_frame( &mut self, frame: Frame, lane_key: Option<&StarKey> )
    {
        self.process_transactions(&frame,lane_key).await;
        match frame
        {
            Frame::Proto(proto) => {
              match &proto
              {
                  ProtoFrame::RequestSubgraphExpansion => {
                      if let Option::Some(lane_key) = lane_key
                      {
                          let mut subgraph = self.skel.info.star.subgraph.clone();
                          subgraph.push(StarSubGraphKey::Big(self.star_subgraph_expansion_seq.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
                          self.send_frame(lane_key.clone(), Frame::Proto(ProtoFrame::GrantSubgraphExpansion(subgraph))).await;
                      }
                      else
                      {
                          eprintln!("missing lane key in RequestSubgraphExpansion")
                      }

                  }
                  _ => {}

              }

            }
            Frame::StarWind(wind) => {

                match wind{
                    StarWind::Up(wind_up) => {
                        if let Option::Some(lane_key) = lane_key
                        {
                            self.on_wind_up_hop(wind_up, lane_key.clone()).await;
                        }
                        else {
                            eprintln!("missing lanekey on WindUp");
                        }
                    }
                    StarWind::Down(wind_down) => {
                        if let Option::Some(lane_key) = lane_key
                        {
                            self.on_wind_down(wind_down, lane_key.clone()).await;
                        }
                        else {
                            eprintln!("missing lanekey on WindDown");
                        }

                    }
                }

            }
            Frame::StarMessage(message) => {
                match self.on_message(message).await
                {
                    Ok(messages) => {}
                    Err(error) => {
                        eprintln!("error: {}", error)
                    }
                }
            }
            _ => {
                eprintln!("star does not handle frame: {}", frame)
            }
        }
    }

    async fn on_event(&mut self, event: Event, lane_key: StarKey  )
    {
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

    async fn on_watch( &mut self, watch: Watch, lane_key: StarKey )
    {
        match &watch
        {
            Watch::Add(info) => {
                self.watch_add_renew(info, &lane_key);
                self.forward_watch(watch).await;
            }
            Watch::Remove(info) => {
                if let Option::Some(watches) = self.watches.get_mut(&info.actor)
                {
                    watches.remove(&info.id);
                    if watches.is_empty()
                    {
                        self.watches.remove( &info.actor);
                    }
                }
                self.forward_watch(watch).await;
            }
        }
    }

    fn watch_add_renew( &mut self, watch_info: &WatchInfo, lane_key: &StarKey )
    {
        let star_watch = StarWatchInfo{
            id: watch_info.id.clone(),
            lane: lane_key.clone(),
            timestamp: Instant::now()
        };
        match self.watches.get_mut(&watch_info.actor)
        {
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

    async fn forward_watch( &mut self, watch: Watch )
    {
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




    async fn on_message(&mut self, mut message: StarMessage) -> Result<(),Error>
    {
        if message.to != self.skel.info.star
        {
            if self.skel.info.kind.relay() || message.from == self.skel.info.star
            {
                //forward the message
                self.send_frame(message.to.clone(), Frame::StarMessage(message) ).await;
                return Ok(());
            }
            else {
                return Err(format!("this star {} does not relay Messages", self.skel.info.kind ).into())
            }
        }
        else {
            self.process_message_reply(&message).await;
            self.skel.variant_tx.send( StarVariantCommand::StarMessage( message.clone())).await?;
            self.process_message(message).await?;
            Ok(())
        }
    }


    async fn process_message(&mut self, mut message: StarMessage) -> Result<(),Error> {
        match &message.payload{
            StarMessagePayload::ResourceManager(action) => self.process_resource_manager_action(message.clone(),action.clone()).await?,
            StarMessagePayload::ResourceHost(action) => self.process_resource_host_action(message.clone(),action.clone()).await?,
            _ => {}
        };
        Ok(())
    }


    async fn process_resource_manager_action( &self, message: StarMessage, action: ResourceManagerAction ) -> Result<(),Error>
    {
        if let Option::Some(manager) = self.skel.resource_manager.clone()
        {
            match action {
                ResourceManagerAction::Register(registration) => {
                    let result = manager.register(registration.clone()).await;
                    self.skel.comm().reply_result_empty(message.clone(), result);
                }
                ResourceManagerAction::Location(location) => {
                    let result = manager.set_location(location.clone()).await;
                    self.skel.comm().reply_result_empty(message.clone(), result);
                }
                ResourceManagerAction::Find(find) => {
                    let result = manager.find(find.to_owned()).await;
                    self.skel.comm().reply_result(message.clone(), result);
                }
                ResourceManagerAction::GetKey(address) => {
                    let result = manager.get_key(address.clone()).await;
                    self.skel.comm().reply_result(message.clone(), result);
                }
                ResourceManagerAction::Bind(bind) => {
                    let result = manager.bind(bind.clone()).await;
                    self.skel.comm().reply_result_empty(message.clone(), result);
                }
                ResourceManagerAction::Status(report) => {
                    unimplemented!()
                }
                ResourceManagerAction::SliceStatus(report) => {
                    unimplemented!()
                }
                ResourceManagerAction::Create(_) => {
                    unimplemented!()
                }
                ResourceManagerAction::Select(selector) => {
                    let mut selector = selector.clone();
                    let result = manager.select(selector).await;
                    self.skel.comm().reply_result(message, Reply::from_result(result)).await;
                }
            }
        }

        Ok(())
    }

    async fn process_resource_host_action( &self, message: StarMessage, action: ResourceHostAction) -> Result<(),Error>
    {
        match action {
            ResourceHostAction::IsHosting(resource) => {
                let (request,rx) = Request::new(resource.clone());
                self.skel.core_tx.send(StarCoreCommand::HasResource(request)).await?;
                let skel = self.skel.clone();
                tokio::spawn( async move {
                    if let Result::Ok(Result::Ok(local)) = rx.await{
                        let location = ResourceLocation{
                            key: local.resource,
                            host: skel.info.star.clone(),
                            gathering: local.gathering
                        };
                        skel.comm().simple_reply(message,SimpleReply::Ok(Reply::Location(location))).await;
                    }
                    else {
                        skel.comm().simple_reply(message,SimpleReply::Fail(Fail::ResourceNotFound(resource))).await;
                    }
                });
            }
            ResourceHostAction::Assign(assign) => {
                unimplemented!()
            }
            ResourceHostAction::SliceAssign(assign) => {

                unimplemented!()
            }
            ResourceHostAction::Message(_) => {
                unimplemented!()
            }

        }
        Ok(())
    }


    async fn diagnose( &self, diagnose: Diagnose ) {
        match diagnose {
            Diagnose::HandlersSatisfied(satisfied) => {
                if let Option::Some(star_handler) = &self.skel.star_handler {
                    if let Result::Ok(satisfaction) = star_handler.satisfied(self.skel.info.kind.handles() ).await {
                        satisfied.tx.send( satisfaction );
                    } else {
                        // let satisfied.tx drop since we can't give it an answer
                    }
                } else {
                    satisfied.tx.send( Satisfaction::Ok );
                }
            }
        }
    }


}

pub trait StarKernel : Send
{
}





pub enum StarCommand
{
    AddLane(Lane),
    ConstellationConstructionComplete,
    Init,
    AddConnectorController(ConnectorController),
    AddResourceLocation(AddResourceLocation),
    AddLogger(broadcast::Sender<Logger>),
    SendProtoMessage(ProtoMessage),
    SetFlags(SetFlags),
    ReleaseHold(StarKey),
    WindInit(Wind),
    WindCommit(WindCommit),
    WindDown(WindDown),
    Test(StarTest),
    Frame(Frame),
    ForwardFrame(ForwardFrame),
    FrameTimeout(FrameTimeoutInner),
    FrameError(FrameErrorInner),
    SpaceCommand(RemoteSpaceCommand),
    GetSpaceController(GetSpaceController),
    AppCommand(AppCommand),
    ResourceMessage(Request<ResourceMessageWrapper,Fail>),
    ResourceLocate(Request<ResourceKey,ResourceLocation>),
    SetResourceLocation(Set<ResourceLocation>),
    SetResourceBind(Set<ResourceBinding>),
    Diagnose(Diagnose)
}

pub enum Diagnose{
    HandlersSatisfied(YesNo<Satisfaction>)
}

pub struct GetSpaceController
{
    pub space: SpaceKey,
    pub auth: Authentication,
    pub tx: oneshot::Sender<Result<SpaceController,GetSpaceControllerFail>>
}

pub enum GetSpaceControllerFail
{
    PermissionDenied,
    Error(Error)
}


pub struct SetFlags
{
    pub flags: Flags,
    pub tx: oneshot::Sender<()>
}



pub struct ActorCreate
{
    pub app: AppKey,
    pub kind: ActorKind,
    pub data: Arc<Vec<u8>>
}



impl ActorCreate
{
    pub fn new(app:AppKey, kind: ActorKind, data:Vec<u8>) -> Self
    {
        ActorCreate {
            app: app,
            kind: kind,
            data: Arc::new(data)
        }
    }
}

pub struct ForwardFrame
{
    pub to: StarKey,
    pub frame: Frame
}

pub struct AddResourceLocation
{
    pub tx: mpsc::Sender<()>,
    pub resource_location: ResourceLocation
}





pub struct Wind
{
    pub pattern: StarPattern,
    pub tx: oneshot::Sender<WindHits>,
    pub max_hops: usize,
    pub action: WindAction
}

impl Wind
{
    pub fn new(pattern: StarPattern, action: WindAction ) -> (Self, oneshot::Receiver<WindHits>)
    {
        let (tx,rx) = oneshot::channel();
        (Wind {
           pattern: pattern,
           tx: tx,
           max_hops: 16,
           action: action
        }, rx )
    }
}

pub enum StarVariantCommand
{
    StarSkel(StarSkel),
    Init,
    CoreRequest(CoreRequest),
    StarMessage(StarMessage),
    CentralCommand(CentralCommand),
}

pub enum CoreRequest
{
    AppSequenceRequest(CoreAppSequenceRequest)
}

pub struct CoreAppSequenceRequest
{
    pub app: AppKey,
    pub user: UserKey,
    pub tx: Sender<u64>
}

pub enum CentralCommand
{
  SequenceRequest
}

pub struct SetSupervisorForApp
{
    pub supervisor: StarKey,
    pub app: AppKey
}

impl SetSupervisorForApp
{
    pub fn new( supervisor: StarKey, app: AppKey ) -> Self
    {
        SetSupervisorForApp {
            supervisor: supervisor,
            app: app
        }
    }
}



pub enum ServerCommand
{
    PledgeToSupervisor
}

pub struct LocalResourceLocation
{
    pub resource: ResourceKey,
    pub gathering: Option<GatheringKey>
}

impl LocalResourceLocation
{
    pub fn new( resource: ResourceKey, gathering: Option<GatheringKey> )->Self
    {
        LocalResourceLocation {
            resource: resource,
            gathering: gathering
        }
    }
}

pub struct Request<P,R> {
   pub payload: P,
   pub tx: oneshot::Sender<Result<R,Fail>>
}

impl <P,R> Request<P,R>
{
    pub fn new( payload: P )->(Self,oneshot::Receiver<Result<R,Fail>>) {
        let (tx,rx) = oneshot::channel();
        (Request{
            payload: payload,
            tx: tx
        },rx)
    }
}

pub struct Query<P,R> {
    pub payload: P,
    pub tx: oneshot::Sender<R>
}

impl <P,R> Query<P,R>
{
    pub fn new( payload: P )->(Self,oneshot::Receiver<R>) {
        let (tx,rx) = oneshot::channel();
        (Query{
            payload: payload,
            tx: tx
        },rx)
    }
}

pub struct YesNo<R> {
    pub tx: oneshot::Sender<R>
}

impl <R> YesNo<R>
{
    pub fn new( )->(Self,oneshot::Receiver<R>) {
        let (tx,rx) = oneshot::channel();
        (YesNo{
            tx: tx
        },rx)
    }
}


pub struct Set<P> {
    pub payload: P,
    pub tx: oneshot::Sender<P>
}

impl <P> Set<P>
{
    pub fn new( payload: P )->(Self,oneshot::Receiver<P>) {
        let (tx,rx) = oneshot::channel();
        (Set{
            payload: payload,
            tx: tx
        },rx)
    }

    pub fn commit(self)
    {
        self.tx.send(self.payload);
    }
}

pub struct Empty {
}

impl Empty {
    pub fn new()->Self {
        Empty{}
    }
}



pub struct FrameTimeoutInner
{
    pub frame: Frame,
    pub retries: usize
}

pub struct FrameErrorInner
{
    pub frame: Frame,
    pub message: String
}


pub enum StarTest
{
   StarSearchForStarKey(StarKey)
}


impl fmt::Display for StarVariantCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarVariantCommand::StarMessage(message) => format!("StarMessage({})", message.payload ).to_string(),
            StarVariantCommand::CentralCommand(_) => "CentralCommand".to_string(),
            StarVariantCommand::Init => "Init".to_string(),
            StarVariantCommand::StarSkel(_) => "StarSkel".to_string(),
            StarVariantCommand::CoreRequest(_) => "CoreRequest".to_string(),
        };
        write!(f, "{}",r)
    }
}



impl fmt::Display for StarCommand{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarCommand::AddLane(_) => format!("AddLane").to_string(),
            StarCommand::AddConnectorController(_) => format!("AddConnectorController").to_string(),
            StarCommand::AddLogger(_) => format!("AddLogger").to_string(),
            StarCommand::Test(_) => format!("Test").to_string(),
            StarCommand::Frame(frame) => format!("Frame({})",frame).to_string(),
            StarCommand::FrameTimeout(_) => format!("FrameTimeout").to_string(),
            StarCommand::FrameError(_) => format!("FrameError").to_string(),
            StarCommand::WindInit(_) => format!("Search").to_string(),
            StarCommand::WindCommit(_) => format!("SearchResult").to_string(),
            StarCommand::ReleaseHold(_) => format!("ReleaseHold").to_string(),
            StarCommand::AddResourceLocation(_) => format!("AddResourceLocation").to_string(),
            StarCommand::ForwardFrame(_) => format!("ForwardFrame").to_string(),
            StarCommand::SpaceCommand(_) => format!("AppLifecycleCommand").to_string(),
            StarCommand::AppCommand(_) => format!("AppCommand").to_string(),
            StarCommand::WindDown(_) => format!("SearchReturnResult").to_string(),
            StarCommand::SendProtoMessage(_) => format!("SendProtoMessage(_)").to_string(),
            StarCommand::SetFlags(_) => format!("SetFlags(_)").to_string(),
            StarCommand::ConstellationConstructionComplete => "ConstellationConstructionComplete".to_string(),
            StarCommand::Init => "Init".to_string(),
            StarCommand::GetSpaceController(_) => "GetSpaceController".to_string(),
            StarCommand::ResourceMessage(_) => "ResourceMessage".to_string(),
            StarCommand::ResourceLocate(_) => "ResourceLocate".to_string(),
            StarCommand::SetResourceLocation(_) => "SetResourceLocation".to_string(),
            StarCommand::SetResourceBind(_) => "SetResourceBind".to_string(),
            StarCommand::Diagnose(_) => "Diagnose".to_string()
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone)]
pub struct StarController
{
    pub star_tx: mpsc::Sender<StarCommand>
}

impl StarController
{
   pub async fn set_flags(&self, flags: Flags ) -> oneshot::Receiver<()>
   {
       let (tx,rx) = oneshot::channel();

       let set_flags = SetFlags{
           flags: flags,
           tx: tx
       };

       self.star_tx.send( StarCommand::SetFlags(set_flags) ).await;
       rx
   }

   pub async fn get_space_controller( &self, space: &SpaceKey, authentication: &Authentication ) -> Result<SpaceController,GetSpaceControllerFail>
   {
       let (tx,rx) = oneshot::channel();

       let get = GetSpaceController{
           space: space.clone(),
           auth: authentication.clone(),
           tx: tx
       };

       self.star_tx.send( StarCommand::GetSpaceController(get) ).await;

       match rx.await
       {
           Ok(result) => result,
           Err(error) => {
               Err(GetSpaceControllerFail::Error(error.into()))
           }
       }
   }

   pub async fn diagnose_handlers_satisfaction(&self ) -> Result<Satisfaction,Error>{
       let( yesno, rx ) = YesNo::new();
       self.star_tx.send( StarCommand::Diagnose(Diagnose::HandlersSatisfied(yesno))).await;
       Ok(tokio::time::timeout( Duration::from_secs(5), rx).await??)
   }
}


#[derive(Clone)]
pub struct StarWatchInfo
{
    pub id: Id,
    pub timestamp: Instant,
    pub lane: StarKey
}


pub struct ApplicationSupervisorSearchTransaction
{
    pub app_id: AppKey,
    pub tx: mpsc::Sender<AppLocation>
}

impl ApplicationSupervisorSearchTransaction
{
    pub fn new(app_id: AppKey) ->(Self,mpsc::Receiver<AppLocation>)
    {
        let (tx,rx) = mpsc::channel(1);
        (ApplicationSupervisorSearchTransaction{
            app_id: app_id,
            tx: tx
        },rx)
    }
}


pub struct ResourceLocationRequestTransaction
{
    pub tx: mpsc::Sender<()>
}

impl ResourceLocationRequestTransaction
{
    pub fn new() ->(Self,mpsc::Receiver<()>)
    {
        let (tx,rx) = mpsc::channel(1);
        (ResourceLocationRequestTransaction{
            tx: tx
        },rx)
    }
}

#[async_trait]
impl Transaction for ResourceLocationRequestTransaction
{
    async fn on_frame(&mut self, frame: &Frame, lane: Option<&mut LaneMeta>, command_tx: &mut mpsc::Sender<StarCommand>) -> TransactionResult {
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


pub struct StarSearchTransaction
{
    pub pattern: StarPattern,
    pub reported_lane_count: usize,
    pub lanes: usize,
    pub hits: HashMap<StarKey, HashMap<StarKey,usize>>,
    command_tx: mpsc::Sender<StarCommand>,
    tx: Vec<oneshot::Sender<WindHits>>,
    local_hit: Option<StarKey>

}

impl StarSearchTransaction
{
    pub fn new(pattern: StarPattern, command_tx: mpsc::Sender<StarCommand>, tx: oneshot::Sender<WindHits>, lanes: usize, local_hit: Option<StarKey> ) ->Self
    {
        StarSearchTransaction{
            pattern: pattern,
            reported_lane_count: 0,
            hits: HashMap::new(),
            command_tx: command_tx,
            tx: vec!(tx),
            lanes: lanes,
            local_hit: local_hit
        }
    }

    fn collapse(&self) -> HashMap<StarKey,usize>
    {
        let mut rtn = HashMap::new();
        for (lane,map) in &self.hits
        {
            for (star,hops) in map
            {
                if rtn.contains_key(star)
                {
                    if let Some(old) = rtn.get(star)
                    {
                       if hops < old
                       {
                           rtn.insert( star.clone(), hops.clone() );
                       }
                    }
                }
                else
                {
                    rtn.insert( star.clone(), hops.clone() );
                }
            }
        }

        if let Option::Some(local) = &self.local_hit
        {
           rtn.insert( local.clone(), 0 );
        }

        rtn
    }

    pub async fn commit(&mut self)
    {
        if self.tx.len() != 0
        {
            let tx = self.tx.remove(0);
            let commit = WindCommit {
                tx: tx,
                result: WindHits
                {
                    pattern: self.pattern.clone(),
                    hits: self.collapse(),
                    lane_hits: self.hits.clone()
                }
            };

            self.command_tx.send(StarCommand::WindCommit(commit)).await;
        }
    }
}

#[async_trait]
impl Transaction for StarSearchTransaction
{
    async fn on_frame(&mut self, frame: &Frame, lane: Option<&mut LaneMeta>, command_tx: &mut mpsc::Sender<StarCommand>) -> TransactionResult {
        if let Option::None = lane
        {
            eprintln!("lane is not set for StarSearchTransaction");
            return TransactionResult::Done;
        }

        let lane = lane.unwrap();

        if let Frame::StarWind( StarWind::Down(wind_down)) = frame
        {

            if let WindResults::Hits(hits) = &wind_down.result
            {
                let mut lane_hits = HashMap::new();
                for hit in hits.clone()
                {
                    if !lane_hits.contains_key(&hit.star)
                    {
                        lane_hits.insert(hit.star.clone(), hit.hops);
                    } else {
                        if let Option::Some(old) = lane_hits.get(&hit.star)
                        {
                            if hit.hops < *old
                            {
                                lane_hits.insert(hit.star.clone(), hit.hops);
                            }
                        }
                    }
                }

            self.hits.insert( lane.lane.remote_star.clone().unwrap(), lane_hits );
            }
        }

        self.reported_lane_count = self.reported_lane_count+1;

        if self.reported_lane_count >= self.lanes
        {
            self.commit().await;
            TransactionResult::Done
        }
        else {
            TransactionResult::Continue
        }

    }
}

pub struct AppCreateTransaction
{
    pub command_tx: mpsc::Sender<StarCommand>,
    pub tx: mpsc::Sender<AppController>
}

#[async_trait]
impl Transaction for AppCreateTransaction
{
    async fn on_frame(&mut self, frame: &Frame, lane: Option<&mut LaneMeta>, command_tx: &mut mpsc::Sender<StarCommand>) -> TransactionResult
    {
        /*
        if let Frame::StarMessage(message) = &frame
        {
            if let StarMessagePayload::ApplicationNotifyReady(notify) = &message.payload
            {
                let (tx,mut rx) = mpsc::channel(1);
                let add = AddAppLocation{ tx: tx.clone(), app_location: notify.location.clone() };
                self.command_tx.send( StarCommand::AddAppLocation(add)).await;

                let ( app_tx, mut app_rx ) = mpsc::channel(1);
                let command_tx = self.command_tx.clone();
                tokio::spawn( async move {
                    while let Option::Some(command) = app_rx.recv().await {
                        command_tx.send( StarCommand::AppCommand(command)).await;
                    }
                });

                let app_ctrl_tx = self.tx.clone();
                tokio::spawn( async move {
                    if let Option::Some(location) = rx.recv().await
                    {
                        let ctrl = AppController{
                            app: location.app.clone(),
                            tx: app_tx
                        };
                        app_ctrl_tx.send(ctrl).await;
                    }
                });
                return TransactionResult::Done;
            }
        }

         */
        unimplemented!();
        TransactionResult::Continue
    }
}

pub struct LaneHit{
    lane: StarKey,
    star: StarKey,
    hops: usize
}

pub struct WindCommit
{
    pub result: WindHits,
    pub tx: oneshot::Sender<WindHits>
}


#[derive(Clone)]
pub struct WindHits
{
    pub pattern: StarPattern,
    pub hits: HashMap<StarKey,usize>,
    pub lane_hits: HashMap<StarKey,HashMap<StarKey,usize>>,
}

impl WindHits
{
   pub fn nearest(&self)->Option<WindHit>
   {
       let mut min: Option<WindHit> = Option::None;

       for (star,hops) in &self.hits
       {
           if min.as_ref().is_none() || hops < &min.as_ref().unwrap().hops
           {
               min = Option::Some( WindHit { star: star.clone(), hops: hops.clone() } );
           }
       }

       min
   }
}

pub enum TransactionResult
{
    Continue,
    Done
}

#[async_trait]
pub trait Transaction : Send+Sync
{
    async fn on_frame( &mut self, frame: &Frame, lane: Option<&mut LaneMeta>, command_tx: &mut mpsc::Sender<StarCommand> )-> TransactionResult;
}


pub struct ShortestPathStarKey
{
    pub to: StarKey,
    pub next_lane: StarKey,
    pub hops: usize
}


pub struct FrameHold
{
    hold: HashMap<StarKey,Vec<Frame>>
}

impl FrameHold {

    pub fn new()->Self
    {
        FrameHold{
            hold: HashMap::new()
        }
    }

    pub fn add(&mut self, star: &StarKey, frame: Frame)
    {
        if !self.hold.contains_key(star)
        {
            self.hold.insert( star.clone(), vec!() );
        }
        if let Option::Some(frames) = self.hold.get_mut(star)
        {
            frames.push(frame);
        }
    }

    pub fn release( &mut self, star: &StarKey ) -> Option<Vec<Frame>>
    {
        self.hold.remove(star)
    }

    pub fn has_hold( &self, star: &StarKey )->bool
    {
        return self.hold.contains_key(star);
    }
}


#[async_trait]
trait StarVariant: Send+Sync
{
    async fn handle(&mut self, command: StarVariantCommand);
}


#[derive(PartialEq, Eq, Ord, PartialOrd, Hash, Debug, Clone, Serialize, Deserialize)]
pub enum StarSubGraphKey
{
    Big(u64),
    Small(u16)
}


#[derive(PartialEq, Eq, PartialOrd, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKey
{
    pub subgraph: Vec<StarSubGraphKey>,
    pub index: u16
}

impl StarKey
{
    pub fn central()->Self
    {
        StarKey{
            subgraph: vec![],
            index: 0
        }
    }

}

impl StarKey
{
    pub fn bin(&self)->Result<Vec<u8>,Error>
    {
        let mut bin= bincode::serialize(self)?;
        Ok(bin)
    }

    pub fn from_bin(mut bin: Vec<u8> )->Result<StarKey,Error>
    {
        let mut key = bincode::deserialize::<StarKey>(bin.as_slice() )?;
        Ok(key)
    }
}

impl cmp::Ord for StarKey
{
    fn cmp(&self, other: &Self) -> Ordering {
        if self.subgraph.len() > other.subgraph.len()
        {
            Ordering::Greater
        }
        else if self.subgraph.len() < other.subgraph.len()
        {
            Ordering::Less
        }
        else if self.subgraph.cmp(&other.subgraph) != Ordering::Equal
        {
            return self.subgraph.cmp(&other.subgraph);
        }
        else
        {
            return self.index.cmp(&other.index );
        }
    }
}

impl fmt::Display for StarKey{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:?},{})", self.subgraph, self.index)
    }
}

#[derive(Eq,PartialEq,Hash,Clone)]
pub struct StarName
{
    pub constellation: String,
    pub star: String
}

impl StarKey
{
   pub fn new( index: u16)->Self
   {
       StarKey {
           subgraph: vec![],
           index: index
       }
   }

   pub fn new_with_subgraph(subgraph: Vec<StarSubGraphKey>, index: u16) ->Self
   {
      StarKey {
          subgraph,
          index: index
      }
   }

   pub fn with_index( &self, index: u16)->Self
   {
       StarKey {
           subgraph: self.subgraph.clone(),
           index: index
       }
   }

   // highest to lowest
   pub fn sort( a : StarKey, b: StarKey  ) -> Result<(Self,Self),Error>
   {
       if a == b
       {
           Err(format!("both StarKeys are equal. {}=={}",a,b).into())
       }
       else if a.cmp(&b) == Ordering::Greater
       {
           Ok((a,b))
       }
       else
       {
           Ok((b,a))
       }
   }
}

trait ServerVariantBacking: Send+Sync
{
    fn set_supervisor( &mut self, supervisor_star: StarKey );
    fn get_supervisor( &self )->Option<&StarKey>;
}


pub struct PlaceholderStarManager
{
    pub data: StarSkel
}

impl PlaceholderStarManager
{

    pub fn new(info: StarSkel) ->Self
    {
        PlaceholderStarManager{
            data: info
        }
    }
}

#[async_trait]
impl StarVariant for PlaceholderStarManager
{
    async fn handle(&mut self, command: StarVariantCommand)  {
        match &command
        {
            StarVariantCommand::Init => {}
            StarVariantCommand::StarMessage(message)=>{
                match &message.payload{
                    StarMessagePayload::Reply(_) => {}
                    _ => {
                        println!("command {} Placeholder unimplemented for kind: {}",command,self.data.info.kind);
                    }
                }
            }
            _ => {
                println!("command {} Placeholder unimplemented for kind: {}",command,self.data.info.kind);
            }
        }
    }
}

#[async_trait]
pub trait StarManagerFactory: Sync+Send
{
    async fn create(&self) -> mpsc::Sender<StarVariantCommand>;
}


pub struct StarManagerFactoryDefault
{
}

#[async_trait]
impl StarManagerFactory for StarManagerFactoryDefault
{
    async fn create(&self) -> mpsc::Sender<StarVariantCommand>
    {
        let (mut tx,mut rx) = mpsc::channel(32);

        tokio::spawn( async move {
            let mut manager:Box<dyn StarVariant> = loop {
                if let Option::Some(StarVariantCommand::StarSkel(data)) = rx.recv().await
                {
                    if let StarKind::Central = data.info.kind
                    {
                        break Box::new(CentralStarVariant::new(data.clone() ).await );
                    }
                    if let StarKind::SpaceHost = data.info.kind
                    {
                        break Box::new(SpaceVariant::new(data.clone() ).await );
                    }
                    else if let StarKind::AppHost = data.info.kind
                    {
                        break Box::new(SupervisorVariant::new(data.clone()).await );
                    }
                    else if let StarKind::ActorHost = data.info.kind
                    {
                        break Box::new(ServerStarVariant::new(data.clone()));
                    }
                    else {
                        break Box::new(PlaceholderStarManager::new(data.clone()))
                    }
                }
                else {
                    eprintln!("must send StarSkel, before manager commands can be processed")
                }
            };

            while let Option::Some(command) = rx.recv().await
            {
                manager.handle(command).await;
            }
        }  );

        tx
    }
}


#[derive(Clone)]
pub struct StarSkel
{
    pub info: StarInfo,
    pub star_tx: mpsc::Sender<StarCommand>,
    pub core_tx: mpsc::Sender<StarCoreCommand>,
    pub variant_tx: mpsc::Sender<StarVariantCommand>,
    pub flags: Flags,
    pub logger: Logger,
    pub sequence: Arc<AtomicU64>,
    pub auth_token_source: AuthTokenSource,
    pub resource_manager: Option<Arc<dyn ResourceRegistryBacking>>,
    pub star_handler: Option<StarHandleBacking>
}

impl StarSkel
{
    pub fn comm(&self) -> StarComm
    {
        StarComm{
            star_tx: self.star_tx.clone(),
            variant_tx: self.variant_tx.clone(),
            core_tx: self.core_tx.clone(),
        }
    }
}

#[derive(Clone)]
pub struct StarInfo
{
   pub star: StarKey,
   pub kind: StarKind,
}

impl StarInfo
{
    pub fn new( star: StarKey, kind: StarKind ) -> Self
    {
        StarInfo{
            star: star,
            kind: kind
        }
    }
}

pub struct PublicKeySource
{
}

impl PublicKeySource
{
    pub fn new()->Self
    {
        PublicKeySource{}
    }

    pub async fn get_public_key_and_hash(&self, star: &StarKey)->(PublicKey,UniqueHash)
    {
        (
            PublicKey{
                id: Default::default(),
                data: vec![]
            },
            UniqueHash{
                id: HashId::new_v4(),
                hash: vec![]
            }
        )
    }

    pub async fn create_encrypted_payloads( &self, creds: &Credentials, star: &StarKey, payload: SpaceMessage ) -> Result<(HashEncrypted<AuthToken>,Encrypted<SpaceMessage>),Error>
    {
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

#[derive(Clone,Serialize,Deserialize)]
pub struct StarNotify
{
    pub star: StarKey,
    pub transaction: Id
}

impl StarNotify
{
    pub fn new( star: StarKey, transaction: Id ) -> Self
    {
        StarNotify{
            star: star,
            transaction: transaction
        }
    }
}

#[derive(Clone)]
pub struct StarComm
{
    pub star_tx: mpsc::Sender<StarCommand>,
    pub variant_tx: mpsc::Sender<StarVariantCommand>,
    pub core_tx: mpsc::Sender<StarCoreCommand>
}

impl StarComm
{
     pub async fn send( &self, proto: ProtoMessage ) {
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
     }
    pub async fn reply<R>( &self, message: StarMessage, result: Result<R,Fail> ) {
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

    pub async fn reply_ok( &self, message: StarMessage ) {
        let proto = message.reply( StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
    }


    pub async fn handle_ok_response<R>( &self, rx: oneshot::Receiver<Result<R,Fail>>, message: StarMessage ) where R: Send+Sync+'static
    {
        let star_tx = self.star_tx.clone();
        tokio::spawn( async move {
            let reply = match rx.await
            {
                Ok(result) => {
                    match result
                    {
                        Ok(ok) => {
                            SimpleReply::Ok(Reply::Empty)
                        }
                        Err(fail) => {
                            SimpleReply::Fail(fail)
                        }
                    }
                }
                Err(err) => {
                    SimpleReply::Fail(Fail::RecvErr)
                }
            };
            let proto = message.reply(StarMessagePayload::Reply(reply));
            star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
        } );
    }
    pub async fn send_and_get_ok_result( &self, proto: ProtoMessage, tx: oneshot::Sender<Result<(),Fail>> ) {
        let result = proto.get_ok_result().await;
        tokio::spawn( async move {
            match tokio::time::timeout( Duration::from_secs(30), result).await
            {
                Ok(result) => {
                    match result{
                        Ok(payload) => {
                            match payload
                            {
                                StarMessagePayload::Reply(reply) => {
                                    match reply
                                    {
                                        SimpleReply::Ok(reply) => {
                                            tx.send(Result::Ok(()));
                                        }
                                        SimpleReply::Fail(fail) => {
                                            tx.send(Result::Err(fail));
                                        }
                                        _ => {
                                            tx.send(Result::Err(Fail::Unexpected));
                                        }
                                    }
                                }
                                _ => {
                                    tx.send(Result::Err(Fail::Unexpected));
                                }
                            }
                        }
                        Err(error) => {
                            tx.send(Result::Err(Fail::Unexpected));
                        }
                    }
                }
                Err(elapsed) => {
                    tx.send(Result::Err(Fail::Timeout));
                }
            };
        } );
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
    }

    pub async fn send_and_get_result( &self, proto: ProtoMessage, tx: oneshot::Sender<Result<Reply,Fail>> ) {
        let result = proto.get_ok_result().await;
        tokio::spawn( async move {
            match tokio::time::timeout( Duration::from_secs(30), result).await
            {
                Ok(result) => {
                    match result{
                        Ok(payload) => {
                            match payload
                            {
                                StarMessagePayload::Reply(reply) => {
                                    match reply
                                    {
                                        SimpleReply::Ok(reply) => {
                                            tx.send(Result::Ok(reply));
                                        }
                                        SimpleReply::Fail(fail) => {
                                            tx.send(Result::Err(fail));
                                        }
                                        _ => {
                                            tx.send(Result::Err(Fail::Unexpected));
                                        }
                                    }
                                }
                                _ => {
                                    tx.send(Result::Err(Fail::Unexpected));
                                }
                            }
                        }
                        Err(error) => {
                            tx.send(Result::Err(Fail::Unexpected));
                        }
                    }
                }
                Err(elapsed) => {
                    tx.send(Result::Err(Fail::Timeout));
                }
            };
        } );
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
    }
}

impl StarComm
{

    pub async fn reply_rx(&self, message: StarMessage, rx: oneshot::Receiver<Result<Reply,Fail>>)
    {
        let star_tx = self.star_tx.clone();
        tokio::spawn( async move {

            match tokio::time::timeout( Duration::from_secs(5), rx).await
            {
                Ok(result) => {
                    match result
                    {
                        Ok(result) => {
                            match result
                            {
                                Ok(reply) => {
                                    let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(reply)));
                                    star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
                                }
                                Err(fail) => {
                                    let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                                    star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
                                }
                            }
                        }
                        Err(_) => {
                            let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error("Internal Error".to_string()))));
                            star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
                        }
                    }
                }
                Err(err) => {
                    let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Timeout)));
                    star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
                }
            }

        });

    }

    pub async fn simple_reply(&self, message: StarMessage, reply: SimpleReply )
    {
      let proto = message.reply(StarMessagePayload::Reply(reply));
      self.send(proto).await;
    }

    pub async fn reply_result_empty_rx(&self, message: StarMessage, rx: oneshot::Receiver<Result<(),Fail>> )
    {
        let star_tx = self.star_tx.clone();
        tokio::spawn( async move {
            match rx.await
            {
                Ok(result) => {
                    match result {
                        Ok(_) => {
                            let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
                            star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                        }
                        Err(fail) => {
                            let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                            star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                        }
                    }
                }
                Err(fail) => {
                    let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Unexpected)));
                    star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                }
            }
        });
    }

    pub async fn reply_result_empty(&self, message: StarMessage, result: Result<(),Fail>)
    {
        match result
        {
            Ok(reply) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Empty)));
                self.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
            }
            Err(fail) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                self.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
            }
        }
    }

    pub async fn reply_result(&self, message: StarMessage, result: Result<Reply,Fail>)
    {
        match result
        {
            Ok(reply) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Ok(reply)));
                self.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
            }
            Err(fail) => {
                let proto = message.reply(StarMessagePayload::Reply(SimpleReply::Fail(fail)));
                self.star_tx.send( StarCommand::SendProtoMessage(proto) ).await;
            }
        }
    }

    pub async fn relay( &self, message: StarMessage, rx: oneshot::Receiver<StarMessagePayload> )
    {
        self.relay_trigger(message,rx, Option::None, Option::None).await;
    }

    pub async fn relay_trigger(&self, message: StarMessage, rx: oneshot::Receiver<StarMessagePayload>, trigger: Option<StarVariantCommand>, trigger_reply: Option<Reply> )
    {
        let variant_tx = self.variant_tx.clone();
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



    fn relay_payload(message: StarMessage, payload: StarMessagePayload, trigger_reply: Option<Reply> ) -> ProtoMessage
    {
        match payload
        {
            StarMessagePayload::Reply(payload_reply) => {
                match payload_reply
                {
                    SimpleReply::Ok(_) => {
                        match trigger_reply
                        {
                            None => {
                                message.reply(StarMessagePayload::Reply(payload_reply) )
                            }
                            Some(reply) => {
                                message.reply(StarMessagePayload::Reply(SimpleReply::Ok(reply)) )
                            }
                        }
                    }
                    _ => {
                        message.reply(StarMessagePayload::Reply(payload_reply) )
                    }
                }
            }
            _ => {
                message.reply(StarMessagePayload::Reply(SimpleReply::Fail(Fail::Error("unexpected response".to_string()))))
            }
        }
    }


}

#[async_trait]
pub trait ResourceRegistryBacking: Sync+Send {
    async fn accepts(&self, accept: HashSet<ResourceType> ) ->Result<(),Fail>;
    async fn register(&self, registration: ResourceRegistration)->Result<(),Fail>;
    async fn select(&self, select: Selector)->Result<Vec<Resource>,Fail>;
    async fn set_location(&self, location: ResourceLocation)->Result<(),Fail>;
    async fn find(&self, keys: ResourceKey)->Result<Reply,Fail>;
    async fn bind(&self, bind: ResourceBinding)->Result<(),Fail>;
    async fn get_address(&self, key: ResourceKey)->Result<Reply,Fail>;
    async fn get_key(&self, address: ResourceAddress)->Result<Reply,Fail>;
}

pub struct ResourceRegistryBackingSqLite
{
    registry: mpsc::Sender<ResourceRegistryAction>
}

impl ResourceRegistryBackingSqLite
{
    pub async fn new(accepts: HashSet<ResourceType>) ->Result<Self,Error>
    {
        let rtn = ResourceRegistryBackingSqLite
        {
            registry: Registry::new().await
        };

        rtn.accepts(accepts).await?;

        Ok(rtn)
    }
}

#[async_trait]
impl ResourceRegistryBacking for ResourceRegistryBackingSqLite
{
    async fn accepts(&self, accepts: HashSet<ResourceType>) -> Result<(), Fail> {
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Accepts(accepts));
        self.registry.send( request ).await?;
        tokio::time::timeout( Duration::from_secs(5),rx).await??;
        Ok(())
    }

    async fn register(&self,registration: ResourceRegistration) -> Result<(),Fail> {
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Register(registration));
        self.registry.send( request ).await?;
        tokio::time::timeout( Duration::from_secs(5),rx).await??;
        Ok(())
    }

    async fn select(&self, selector: Selector) ->Result<Vec<Resource>,Fail>{
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Select(selector));
        self.registry.send( request ).await?;
        match tokio::time::timeout( Duration::from_secs(5),rx).await??
        {
            ResourceRegistryResult::Resources(resources) => {
                Result::Ok(resources)
            }
            _ => {
                Result::Err(Fail::Timeout)
            }
        }
    }

    async fn set_location(&self, location: ResourceLocation) -> Result<(), Fail> {
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::SetLocation(location));
        self.registry.send( request ).await;
        tokio::time::timeout( Duration::from_secs(5),rx).await??;
        Ok(())
    }

    async fn find(&self, key: ResourceKey) -> Result<Reply, Fail> {
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Find(key));
        self.registry.send( request ).await;
        let result = tokio::time::timeout( Duration::from_secs(5),rx).await??;
        if let ResourceRegistryResult::Location(location) = result {
            Ok(Reply::Location(location))
        }
        else
        {
            Err(Fail::Unexpected)
        }
    }

    async fn bind(&self, bind: ResourceBinding) -> Result<(), Fail> {
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::Bind(bind));
        self.registry.send( request ).await;
        tokio::time::timeout( Duration::from_secs(5),rx).await??;
        Ok(())
    }

    async fn get_address(&self, key: ResourceKey) -> Result<Reply, Fail> {
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::GetAddress(key));
        self.registry.send( request ).await;
        let result = tokio::time::timeout( Duration::from_secs(5),rx).await??;
        if let ResourceRegistryResult::Address(address) = result {
            Ok(Reply::Address(address))
        }
        else
        {
            Err(Fail::Unexpected)
        }
    }

    async fn get_key(&self, address: ResourceAddress) -> Result<Reply, Fail> {
        let (request,rx) = ResourceRegistryAction::new(ResourceRegistryCommand::GetKey(address));
        self.registry.send( request ).await;
        let result = tokio::time::timeout( Duration::from_secs(5),rx).await??;
        if let ResourceRegistryResult::Key(key) = result {
            Ok(Reply::Key(key))
        }
        else
        {
            Err(Fail::Unexpected)
        }
    }
}

