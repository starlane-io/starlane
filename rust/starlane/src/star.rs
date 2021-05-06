use std::{cmp, fmt};
use std::borrow::Borrow;
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;
use std::future::Future;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicI32, AtomicI64};

use futures::future::{BoxFuture, join_all, Map};
use futures::future::select_all;
use futures::FutureExt;
use futures::prelude::future::FusedFuture;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, oneshot};
use tokio::sync::broadcast::error::{RecvError, SendError};
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, timeout};
use tokio::time::error::Elapsed;
use url::Url;

use crate::actor::{Actor, ActorKey, ActorKind, ActorLocation, ActorWatcher};
use crate::app::{AppCommandWrapper, AppController, AppCreate, AppKind, Application, AppLocation};
use crate::core::StarCoreCommand;
use crate::error::Error;
use crate::frame::{ActorBind, ActorEvent, ActorLocationReport, ActorLocationRequest, ActorLookup, ActorMessage, AppAssign, AppCreateRequest, ApplicationSupervisorReport, AppNotifyCreated, AppSupervisorRequest, Event, Frame, ProtoFrame, Rejection, WindHit, StarMessage, StarMessageAck, StarMessagePayload, WindUp, StarPattern, WindDown, Watch, WatchInfo, SequenceMessage, ProtoEvolution, ProtoSequence, WindAction, StarWind, WindResults};
use crate::id::{Id, IdSeq};
use crate::keys::AppKey;
use crate::label::Labels;
use crate::lane::{ConnectionInfo, ConnectorController, Lane, LaneCommand, LaneMeta, OutgoingLane, TunnelConnector, TunnelConnectorFactory};
use crate::message::{MessageExpect, MessageExpectWait, MessageReplyTracker, MessageResult, MessageUpdate, ProtoMessage, StarMessageDeliveryInsurance, TrackerJob};
use crate::proto::{PlaceholderKernel, ProtoStar, ProtoTunnel};
use crate::star::central::CentralManager;
use crate::star::supervisor::{SupervisorCommand, SupervisorManager};
use crate::logger::{Logger, Flags, Flag, StarFlag, ProtoStarLog, Log, ProtoStarLogPayload};
use crate::space::SpaceCommand;
use crate::frame::WindAction::SearchHits;

pub mod central;
pub mod supervisor;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum StarKind
{
    Central,
    Mesh,
    Supervisor,
    Server(ServerKindExt),
    Store(StoreKindExt),
    Gateway,
    Link,
    Client
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
        if let StarKind::Supervisor = self
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
        if let StarKind::Supervisor = self
        {
            Ok(())
        }
        else {
            Err("not supervisor".into())
        }
    }

    pub fn server_result(&self)->Result<(),Error>
    {
        if let StarKind::Server(_)= self
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
            StarKind::Supervisor => false,
            StarKind::Server(_) => true,
            StarKind::Gateway => true,
            StarKind::Client => true,
            StarKind::Link => true,
            StarKind::Store(_) => false
        }
    }
}

impl fmt::Display for StarKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!( f,"{}",
        match self{
            StarKind::Central => "Central".to_string(),
            StarKind::Mesh => "Mesh".to_string(),
            StarKind::Supervisor => "Supervisor".to_string(),
            StarKind::Server(ext) => format!("Server({})",ext.name).to_string(),
            StarKind::Store(ext) => format!("Store({})",ext.name).to_string(),
            StarKind::Gateway => "Gateway".to_string(),
            StarKind::Link => "Link".to_string(),
            StarKind::Client => "Client".to_string(),
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

pub static MAX_HOPS: usize = 16;

pub struct Star
{
    data: StarData,
    star_rx: mpsc::Receiver<StarCommand>,
    core_tx: mpsc::Sender<StarCoreCommand>,
    lanes: HashMap<StarKey, LaneMeta>,
    connector_ctrls: Vec<ConnectorController>,
    transactions: HashMap<Id,Box<dyn Transaction>>,
    frame_hold: FrameHold,
    watches: HashMap<ActorKey,HashMap<Id,StarWatchInfo>>,
    actor_locations: LruCache<ActorKey, ActorLocation>,
    app_locations: LruCache<AppKey,StarKey>,
    messages_received: HashMap<Id,Instant>,
    message_reply_trackers: HashMap<Id, MessageReplyTracker>,
    actors: HashSet<ActorKey>,
}

impl Star
{

    pub fn from_proto(data: StarData,
                      star_rx: mpsc::Receiver<StarCommand>,
                      core_tx: mpsc::Sender<StarCoreCommand>,
                      lanes: HashMap<StarKey,LaneMeta>,
                      connector_ctrls: Vec<ConnectorController>,
                      frame_hold: FrameHold ) ->Self

    {
        Star{
            data: data,
            star_rx: star_rx,
            core_tx: core_tx,
            lanes: lanes,
            connector_ctrls: connector_ctrls,
            transactions: HashMap::new(),
            frame_hold: frame_hold,
            watches: HashMap::new(),
            actor_locations: LruCache::new(64*1024 ),
            app_locations: LruCache::new(4*1024 ),
            messages_received: HashMap::new(),
            message_reply_trackers: HashMap::new(),
            actors: HashSet::new()
        }
    }

    pub fn has_actor(&self, key: &ActorKey) -> bool
    {
        self.actors.contains(&key)
    }


    pub async fn run(mut self)
    {
        self.broadcast(Frame::Proto(ProtoFrame::Evolution(ProtoEvolution::Report))).await;


        self.data.manager_tx.send(StarManagerCommand::Init ).await;
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
                    StarCommand::SetFlags(set_flags ) => {
                       self.data.flags= set_flags.flags;
                       set_flags.tx.send(());
                    }
                    StarCommand::AddLane(lane) => {
                        if let Some(remote_star)=lane.remote_star.as_ref()
                        {
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));

                            if self.data.info.kind.is_central()
                            {
                                self.broadcast( Frame::Proto(ProtoFrame::CentralFound(1)) ).await;
                            }

                        }
                        else {
                            eprintln!("for star remote star must be set");
                         }
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::AddActorLocation(add_entity_location) => {
                        self.actor_locations.put(add_entity_location.entity_location.actor.clone(), add_entity_location.entity_location.clone() );
                        add_entity_location.tx.send( ()).await;
                    }
                    StarCommand::AddAppLocation(add_app_location) => {
                        self.app_locations.put(add_app_location.app_location.app.clone(), add_app_location.app_location.supervisor.clone() );
                        add_app_location.tx.send( add_app_location.app_location ).await;
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
                    StarCommand::AppLifecycleCommand(command)=>{
                        self.on_space_command(command).await;
                    }
                    StarCommand::AppCommand(command)=>{
                        self.on_app_command(command).await;
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
                    StarCommand::ActorCommand(command) => {
                        self.core_tx.send( StarCoreCommand::Actor(command)).await;
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

    async fn send_proto_message( &mut self, proto: ProtoMessage )
    {

        if let Err(errors) = proto.validate() {
                eprintln!("protomessage is not valid cannot send: {}", errors );
            return;
        }

        let id = self.data.sequence.next();

        let message = StarMessage{
            id: id,
            from: self.data.info.star.clone(),
            to: proto.to.unwrap(),
            transaction: proto.transaction,
            payload: proto.payload,
            reply_to: proto.reply_to
        };

        if message.to == self.data.info.star
        {
            eprintln!("star {} kind {} cannot send a proto message to itself, payload: {} ", self.data.info.star, self.data.info.kind, message.payload );
        }
        else {
            let delivery = StarMessageDeliveryInsurance::with_txrx(message, proto.expect, proto.tx.clone(), proto.tx.subscribe() );
            self.message(delivery).await;
        }
    }

    async fn on_space_command(&mut self, command: SpaceCommand)
    {
println!("on_app_lifecycle_command: {}",command );
    }

    async fn on_app_command( &mut self, command: AppCommandWrapper)
    {

    }


    async fn search_for_star( &mut self, star: StarKey, tx: oneshot::Sender<WindHits> )
   {
        let wind = Wind {
            pattern: StarPattern::StarKey(star),
            tx: tx,
            max_hops: 16,
            action: WindAction::SearchHits
        };
        self.data.star_tx.send( StarCommand::WindInit(wind) ).await;
    }

    async fn do_wind(&mut self, wind: Wind)
    {
        let tx = wind.tx;
        let wind_up = WindUp::new(self.data.info.star.clone(), wind.pattern, wind.action );
        self.do_wind_up(wind_up, tx, Option::None).await;
    }

    async fn do_wind_up(&mut self, mut wind: WindUp, tx: oneshot::Sender<WindHits>, exclude: Option<HashSet<StarKey>> )
    {
        let tid = self.data.sequence.next();

        let num_excludes:usize = match &exclude
        {
            None => 0,
            Some(exclude) => exclude.len()
        };

        let local_hit = match wind.pattern.is_match(&self.data.info){
            true => Option::Some(self.data.info.star.clone()),
            false => Option::None
        };

        let transaction = Box::new(StarSearchTransaction::new(wind.pattern.clone(), self.data.star_tx.clone(), tx, self.lanes.len()-num_excludes, local_hit ));
        self.transactions.insert(tid.clone(), transaction );

        wind.transactions.push(tid.clone());
        wind.hops.push( self.data.info.star.clone() );

        self.broadcast_excluding(Frame::StarWind(StarWind::Up(wind)), &exclude ).await;
    }




    async fn on_wind_up_hop(&mut self, mut wind_up: WindUp, lane_key: StarKey )
    {

        if wind_up.pattern.is_match(&self.data.info)
        {

            if wind_up.pattern.is_single_match()
            {

                let hit = WindHit {
                    star: self.data.info.star.clone(),
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

        if wind_up.max_hops > MAX_HOPS
        {
            eprintln!("rejecting a search with more than maximum {} hops", MAX_HOPS);
            return;
        }

        let hit = wind_up.pattern.is_match(&self.data.info);

        if wind_up.hops.len()+1 > wind_up.max_hops || self.lanes.len() <= 1 || !self.data.info.kind.relay()
        {

            let hits = match hit
            {
                true => {
                    vec![WindHit {star: self.data.info.star.clone(), hops: wind_up.hops.len().clone()+1 }]
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

        let command_tx = self.data.star_tx.clone();
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
        &self.data.info.star
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

            let star_tx = self.data.star_tx.clone();
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
            let command_tx = self.data.star_tx.clone();
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
            Frame::StarMessage(message) => {
                message.transaction
            },
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


                match transaction.on_frame(frame,lane, &mut self.data.star_tx).await
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
                  ProtoFrame::CentralSearch => {
                      if self.data.info.kind.is_central()
                      {
                          self.broadcast(Frame::Proto(ProtoFrame::CentralFound(1))).await;
                      } else if let Option::Some(hops) = self.get_hops_to_star(&StarKey::central() )
                      {
                          self.broadcast(Frame::Proto(ProtoFrame::CentralFound(hops+1))).await;
                      }
                      else
                      {
                          let (tx,rx) = oneshot::channel();
                          self.search_for_star(StarKey::central() ,tx ).await;
                          let command_tx = self.data.star_tx.clone();
                          tokio::spawn( async move {
                              if let Ok(result) = rx.await
                              {
                                  if let Some(hit)=result.nearest()
                                  {
                                      // we found Central, now broadcast it
                                      command_tx.send( StarCommand::Frame(Frame::Proto(ProtoFrame::CentralSearch))).await;
                                  }
                              }
                          });
                      }
                  },
                  ProtoFrame::RequestSubgraphExpansion => {
                      if let Option::Some(lane_key) = lane_key
                      {
                          let mut subgraph = self.data.info.star.subgraph.clone();
                          subgraph.push(self.data.info.star.index.clone());
                          self.send_frame(lane_key.clone(), Frame::Proto(ProtoFrame::GrantSubgraphExpansion(subgraph))).await;
                      }
                      else
                      {
                          eprintln!("missing lane key in RequestSubgraphExpansion")
                      }

                  }
                  ProtoFrame::Evolution(ProtoEvolution::Request)=> {
                      self.broadcast(Frame::Proto(ProtoFrame::Evolution(ProtoEvolution::Report) )).await;
                  }
                  ProtoFrame::Sequence(ProtoSequence::Request)=> {



                      if let Option::Some(star) = lane_key.cloned()
                      {
                          if let Some(lane) = self.lanes.get(&star)
                          {
                              if self.data.info.kind.is_central()
                              {
                                      lane.lane.outgoing.tx.send(LaneCommand::Frame(Frame::Proto(ProtoFrame::Sequence(ProtoSequence::Reply(self.data.sequence.next().index))))).await;
                              }
                              else {
                                  let mut proto = ProtoMessage::new();
                                  proto.to = Option::Some(StarKey::central());
                                  proto.expect = MessageExpect::RetryUntilOk;
                                  proto.payload = StarMessagePayload::Sequence(SequenceMessage::Request);
                                  let mut rx = proto.get_ok_result().await;

                                  let lane_tx = lane.lane.outgoing.tx.clone();
                                  tokio::spawn(async move {
                                      if let Ok(StarMessagePayload::Sequence(SequenceMessage::Response(sequence))) = rx.await
                                      {
                                          lane_tx.send(LaneCommand::Frame(Frame::Proto(ProtoFrame::Sequence(ProtoSequence::Reply(sequence))))).await;
                                      }
                                  });

                                  self.data.star_tx.send(StarCommand::SendProtoMessage(proto)).await;
                              }
                          }
                      }
                  }
                  ProtoFrame::Sequence(ProtoSequence::Reply(_))=> {
                      if self.data.flags.check(Flag::Star(StarFlag::DiagnoseSequence)){
                          self.data.logger.log( Log::ProtoStar(ProtoStarLog::new(self.data.info.kind.clone(), ProtoStarLogPayload::SequenceReplyRecv )));
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
                if let Option::Some(watches) = self.watches.get_mut(&info.entity)
                {
                    watches.remove(&info.id);
                    if watches.is_empty()
                    {
                        self.watches.remove( &info.entity);
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
        match self.watches.get_mut(&watch_info.entity)
        {
            None => {
                let mut watches = HashMap::new();
                watches.insert(watch_info.id.clone(), star_watch);
                self.watches.insert(watch_info.entity.clone(), watches);
            }
            Some(mut watches) => {
                watches.insert(watch_info.id.clone(), star_watch);
            }
        }
    }

    async fn forward_watch( &mut self, watch: Watch )
    {
        let has_entity = match &watch
        {
            Watch::Add(info) => {
                self.has_actor(&info.entity)
            }
            Watch::Remove(info) => {
                self.has_actor(&info.entity)
            }
        };

        let entity = match &watch
        {
            Watch::Add(info) => {
                &info.entity
            }
            Watch::Remove(info) => {
                &info.entity
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
                self.send_frame(location.star.clone(), Frame::Watch(watch)).await;
            }
            else
            {
                let mut rx = self.find_actor_location(lookup).await;
                let command_tx = self.data.star_tx.clone();
                tokio::spawn( async move {
                    if let Option::Some(_) = rx.recv().await
                    {
                        command_tx.send(StarCommand::Frame(Frame::Watch(watch))).await;
                    }
                });
            }
        }
    }

    fn get_app_location(&mut self, app_key: &AppKey) -> Option<&StarKey>
    {
        self.app_locations.get(app_key )
    }

    async fn find_app_location(&mut self, app: AppKey ) -> mpsc::Receiver<AppLocation>
    {
        unimplemented!()
        /*
        let payload = StarMessagePayload::ApplicationSupervisorRequest(AppSupervisorRequest { app: app.clone() } );
        let mut message = StarMessage::new(self.info.sequence.next(), self.info.star_key.clone(), StarKey::central(), payload );
        message.transaction = Option::Some(self.info.sequence.next());

        let (transaction,rx) = ApplicationSupervisorSearchTransaction::new(app.clone());
        let transaction = Box::new(transaction);
        self.transactions.insert( message.transaction.unwrap().clone(), transaction );

        let delivery = StarMessageDeliveryInsurance::new(message, MessageExpect::ReplyErrOrTimeout(MessageExpectWait::Short));

        self.message( delivery ).await;

        rx

         */
    }

    fn get_entity_location(&mut self, kind: ActorLookup) -> Option<&ActorLocation>
    {
        if let ActorLookup::Key(entity) = &kind
        {
            self.actor_locations.get(entity)
        }
        else {
            Option::None
        }
    }

    async fn find_actor_location(&mut self, lookup: ActorLookup) -> mpsc::Receiver<()>
    {

        let supervisor_star = self.get_app_location(&lookup.app() ).cloned();

        match supervisor_star{
            None => {
                let mut rx = self.find_app_location(lookup.app()).await;
                let (xt,xr) = mpsc::channel(1);

                tokio::spawn( async move {
                    if let Option::Some(_) = rx.recv().await
                    {
                        xt.send(()).await;
                    }
                });

                xr
            }
            Some(supervisor_star) => {
                unimplemented!()
/*                let payload = StarMessagePayload::ActorLocationRequest(ActorLocationRequest { lookup } );
                let mut message = StarMessage::new(self.info.sequence.next(), self.info.star_key.clone(), supervisor_star, payload );
                message.transaction = Option::Some(self.info.sequence.next());
                let (transaction,rx) =  ResourceLocationRequestTransaction::new();
                self.transactions.insert( message.transaction.unwrap().clone(), Box::new(transaction) );
                let delivery = StarMessageDeliveryInsurance::new(message, MessageExpect::ReplyErrOrTimeout(MessageExpectWait::Short));
                self.message( delivery ).await;
                rx

 */
            }
        }

    }


    async fn on_message(&mut self, mut message: StarMessage) -> Result<(),Error>
    {
        if message.to != self.data.info.star
        {
            if self.data.info.kind.relay() || message.from == self.data.info.star
            {
                //forward the message
                self.send_frame(message.to.clone(), Frame::StarMessage(message) ).await;
                return Ok(());
            }
            else {
                return Err(format!("this star {} does not relay Messages", self.data.info.kind ).into())
            }
        }
        else {
            self.process_message_reply(&message).await;
            Ok(self.data.manager_tx.send( StarManagerCommand::Frame( Frame::StarMessage(message))).await?)
        }
    }




}

pub trait StarKernel : Send
{
}





pub enum StarCommand
{
    AddLane(Lane),
    AddConnectorController(ConnectorController),
    AddActorLocation(AddEntityLocation),
    AddAppLocation(AddAppLocation),
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
    AppLifecycleCommand(SpaceCommand),
    AppCommand(AppCommandWrapper),
    ActorCommand(ActorCommand)
}


pub struct SetFlags
{
    pub flags: Flags,
    pub tx: oneshot::Sender<()>
}


pub enum ActorCommand
{
   Create(ActorCreate)
}

pub struct ActorCreate
{
    pub app: AppKey,
    pub kind: ActorKind,
    pub data: Vec<u8>
}

impl ActorCreate
{
    pub fn new(app:AppKey, kind: ActorKind, data:Vec<u8>) -> Self
    {
        ActorCreate {
            app: app,
            kind: kind,
            data: data
        }
    }
}

pub struct ForwardFrame
{
    pub to: StarKey,
    pub frame: Frame
}

pub struct AddEntityLocation
{
    pub tx: mpsc::Sender<()>,
    pub entity_location: ActorLocation
}


pub struct AddAppLocation
{
    pub tx: mpsc::Sender<AppLocation>,
    pub app_location: AppLocation
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

pub enum StarManagerCommand
{
    StarData(StarData),
    Init,
    Frame(Frame),
    CentralCommand(CentralCommand),
    SupervisorCommand(SupervisorCommand),
    ServerCommand(ServerCommand),
}

pub enum CentralCommand
{
  SequenceRequest,
  SupervisorSelect(SupervisorSelect)
}

#[derive(Clone,Serialize,Deserialize)]
pub struct SupervisorSelect
{
 //right now supervisors are selected at random
}

pub enum ServerCommand
{
    PledgeToSupervisor
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


impl fmt::Display for StarManagerCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            StarManagerCommand::Frame(frame) => format!("Frame({})", frame).to_string(),
            StarManagerCommand::CentralCommand(_) => "CentralCommand".to_string(),
            StarManagerCommand::SupervisorCommand(_) => "SupervisorCommand".to_string(),
            StarManagerCommand::ServerCommand(_) => "ServerCommand".to_string(),
            StarManagerCommand::Init => "Init".to_string(),
            StarManagerCommand::StarData(_) => "StarData".to_string()
        };
        write!(f, "{}",r)
    }
}

impl fmt::Display for ActorCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            ActorCommand::Create(_) => format!("Create(_)").to_string()
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
            StarCommand::AddActorLocation(_) => format!("AddResourceLocation").to_string(),
            StarCommand::AddAppLocation(_) => format!("AddAppLocation").to_string(),
            StarCommand::ForwardFrame(_) => format!("ForwardFrame").to_string(),
            StarCommand::AppLifecycleCommand(_) => format!("AppLifecycleCommand").to_string(),
            StarCommand::AppCommand(_) => format!("AppCommand").to_string(),
            StarCommand::WindDown(_) => format!("SearchReturnResult").to_string(),
            StarCommand::ActorCommand(command) => format!("EntityCommand({})", command).to_string(),
            StarCommand::SendProtoMessage(_) => format!("SendProtoMessage(_)").to_string(),
            StarCommand::SetFlags(_) => format!("SetFlags(_)").to_string(),
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone)]
pub struct StarController
{
    pub command_tx: mpsc::Sender<StarCommand>
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

       self.command_tx.send( StarCommand::SetFlags(set_flags) ).await;
       rx
   }

   pub async fn create_app(&self, name: Option<String>, kind: AppKind, data: Vec<u8> )->Result<AppController,Error>
   {

       let (tx,mut rx) = mpsc::channel(1);


       let app_create = SpaceCommand::AppCreate( AppCreate{
           kind: kind,
           data: Arc::new(vec!()),
           labels: Labels::new()
       } );

       self.command_tx.send( StarCommand::AppLifecycleCommand(app_create) ).await;

       match timeout(Duration::from_secs(10), rx.recv() ).await
       {
           Ok(opt) => {
               match opt{
                   None => {
                       Err("connection closed before AppController returned".into())
                   }
                   Some(ctrl) => {
                       Ok(ctrl)
                   }
               }
           }
           Err(error) => {
               Err("timeout when trying to acquire AppController".into())
           }
       }
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

#[async_trait]
impl Transaction for ApplicationSupervisorSearchTransaction
{
    async fn on_frame(&mut self, frame: &Frame, lane: Option<&mut LaneMeta>, command_tx: &mut mpsc::Sender<StarCommand>) -> TransactionResult {
        unimplemented!()

        /*
        if let Frame::StarMessage( message ) = frame
        {
            if let StarMessagePayload::ApplicationSupervisorReport(report) = &message.payload
            {
                command_tx.send( StarCommand::AddAppLocation(AddAppLocation{
                    tx: self.tx.clone(),
                    app_location: AppLocation{
                        app: report.app.clone(),
                        supervisor: report.supervisor.clone()
                    }
                })).await;
            }
        }

        TransactionResult::Done

         */
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
trait StarManager: Send+Sync
{
    async fn handle(&mut self, command: StarManagerCommand);
}

pub trait SupervisorManagerBacking: Send+Sync
{
    fn add_server( &mut self, server: StarKey );
    fn remove_server( &mut self, server: &StarKey );
    fn select_server(&mut self) -> Option<StarKey>;

    fn add_application(&mut self, app: AppKey, application: Box<dyn Application> );
    fn get_application(&mut self, app: AppKey ) -> Option<&Box<dyn Application>>;

    fn remove_application(&mut self, app: AppKey );

    fn set_entity_name(&mut self, name: String, key: ActorKey);
    fn set_entity_location(&mut self, entity: ActorKey, location: ActorLocation);
    fn get_entity_location(&self, lookup: &ActorLookup) -> Option<&ActorLocation>;
}


#[derive(PartialEq, Eq, PartialOrd, Hash, Debug, Clone, Serialize, Deserialize)]
pub struct StarKey
{
    pub subgraph: Vec<u16>,
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

   pub fn new_with_subgraph(subgraph: Vec<u16>, index: u16) ->Self
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

pub struct ServerManagerBackingDefault
{
    pub supervisor: Option<StarKey>
}

impl ServerManagerBackingDefault
{
   pub fn new()-> Self
   {
       ServerManagerBackingDefault{
           supervisor: Option::None
       }
   }
}

impl ServerManagerBacking for ServerManagerBackingDefault
{
    fn set_supervisor(&mut self, supervisor_star: StarKey) {
        self.supervisor = Option::Some(supervisor_star);
    }

    fn get_supervisor(&self) -> Option<&StarKey> {
        self.supervisor.as_ref()
    }
}

trait ServerManagerBacking: Send+Sync
{
    fn set_supervisor( &mut self, supervisor_star: StarKey );
    fn get_supervisor( &self )->Option<&StarKey>;
}


pub struct ServerManager
{
    data: StarData,
    backing: Box<dyn ServerManagerBacking>,
}

impl ServerManager
{
    pub fn new(data: StarData) -> Self
    {
        ServerManager
        {
            data: data,
            backing: Box::new(ServerManagerBackingDefault::new())
        }
    }

    pub fn set_supervisor( &mut self, supervisor_star: StarKey )
    {
        self.backing.set_supervisor(supervisor_star);
    }

    pub fn get_supervisor( &self )->Option<&StarKey>
    {
        self.backing.get_supervisor()
    }

    async fn pledge(&mut self)->Result<(),Error>
    {
        let (search,rx) = Wind::new(StarPattern::StarKind(StarKind::Supervisor), WindAction::SearchHits );
        self.data.star_tx.send( StarCommand::WindInit( search ) ).await;
        let result = rx.await?;


        if let Option::Some(hit) = result.nearest()
        {
           self.set_supervisor(hit.star.clone());
           let payload = StarMessagePayload::Pledge;
           let message = StarMessage::new(self.data.sequence.next(), self.data.info.star.clone(), hit.star, payload );
           self.data.star_tx.send( StarCommand::Frame(Frame::StarMessage(message))).await;
        }
        else {
            eprintln!("could not find a supervisor for Server results:{} ", result.hits.len() );
        }

        Ok(())
    }
}

#[async_trait]
impl StarManager for ServerManager
{
    async fn handle(&mut self, command: StarManagerCommand) {

    }
}


pub struct PlaceholderStarManager
{
    pub data: StarData
}

impl PlaceholderStarManager
{

    pub fn new(info: StarData) ->Self
    {
        PlaceholderStarManager{
            data: info
        }
    }
}

#[async_trait]
impl StarManager for PlaceholderStarManager
{
    async fn handle(&mut self, command: StarManagerCommand)  {
        match command
        {
            StarManagerCommand::Init => {}
            _ => {
                println!("command {} Placeholder unimplemented for kind: {}",command,self.data.info.kind);
            }
        }
    }
}

#[async_trait]
pub trait StarManagerFactory: Sync+Send
{
    async fn create(&self) -> mpsc::Sender<StarManagerCommand>;
}


pub struct StarManagerFactoryDefault
{
}

#[async_trait]
impl StarManagerFactory for StarManagerFactoryDefault
{
    async fn create(&self) -> mpsc::Sender<StarManagerCommand>
    {
        let (mut tx,mut rx) = mpsc::channel(32);

        tokio::spawn( async move {
            let mut manager:Box<dyn StarManager> = loop {
                if let Option::Some(StarManagerCommand::StarData(data)) = rx.recv().await
                {
                    if let StarKind::Central = data.info.kind
                    {
                        break Box::new(CentralManager::new(data.clone() ) );
                    }
                    else if let StarKind::Supervisor= data.info.kind
                    {
                        break Box::new(SupervisorManager::new(data.clone()));
                    }
                    else if let StarKind::Server(_)= data.info.kind
                    {
                        break Box::new(ServerManager::new(data.clone()));
                    }
                    else {
                        break Box::new(PlaceholderStarManager::new(data.clone()))
                    }
                }
                else {
                    eprintln!("must send StarData, before manager commands can be processed")
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
pub struct StarData
{
    pub info: StarInfo,
    pub sequence: Arc<IdSeq>,
    pub star_tx: mpsc::Sender<StarCommand>,
    pub manager_tx: mpsc::Sender<StarManagerCommand>,
    pub flags: Flags,
    pub logger: Logger
}

#[derive(Clone)]
pub struct StarInfo
{
   pub star: StarKey,
   pub kind: StarKind,
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