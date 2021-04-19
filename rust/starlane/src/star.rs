use std::{cmp, fmt};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicI32, AtomicI64};

use futures::channel::oneshot;
use futures::channel::oneshot::Canceled;
use futures::future::{join_all, Map, BoxFuture};
use futures::future::select_all;
use futures::FutureExt;
use lru::LruCache;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::{RecvError, SendError};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::mpsc;
use tokio::time::Instant;
use url::Url;

use crate::application::{ApplicationState, AppLocation};
use crate::error::Error;
use crate::frame::{ApplicationAssignInner, ApplicationNotifyReadyInner, ApplicationReportSupervisorInner, ApplicationRequestSupervisorInner, Frame, ProtoFrame, RejectionInner, ResourceBind, ResourceEvent, ResourceEventKind, ResourceLookupKind, ResourceMessage, ResourceReportLocation, ResourceRequestLocation, StarMessageInner, StarMessagePayload, SearchHit, StarSearchInner, StarSearchPattern, StarSearchResultInner, StarUnwindInner, StarUnwindPayload, StarWindInner, StarWindPayload, Watch, WatchInfo};
use crate::frame::Frame::{StarMessage, StarSearch};
use crate::frame::ProtoFrame::CentralSearch;
use crate::frame::StarMessagePayload::{ApplicationCreateRequest, Reject};
use crate::id::{Id, IdSeq};
use crate::lane::{ConnectionInfo, ConnectorController, Lane, LaneCommand, LaneMeta, OutgoingLane, TunnelConnector, TunnelConnectorFactory};
use crate::proto::{PlaceholderKernel, ProtoStar, ProtoTunnel};
use crate::resource::{ResourceKey, ResourceLocation, ResourceWatcher};
use futures::prelude::future::FusedFuture;
use crate::core::CoreCommand;
use std::collections::hash_map::RandomState;
use std::cell::Cell;
use std::borrow::Borrow;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum StarKind
{
    Central,
    Mesh,
    Supervisor,
    Server,
    Gateway,
    Link,
    Client
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
        if let StarKind::Server= self
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
            StarKind::Server => true,
            StarKind::Gateway => true,
            StarKind::Client => true,
            StarKind::Link => true,
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
            StarKind::Server => "Server".to_string(),
            StarKind::Gateway => "Gateway".to_string(),
            StarKind::Link => "Link".to_string(),
            StarKind::Client => "Client".to_string(),
        })
    }
}

impl fmt::Display for ResourceLookupKind{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self{
            ResourceLookupKind::Key(resource) => format!( "Key({})", resource ).to_string(),
            ResourceLookupKind::Name(lookup) => {format!( "Name({})", lookup.name).to_string()}
        };
        write!(f, "{}",r)
    }
}

pub struct StarLogger
{
   pub tx: Vec<broadcast::Sender<StarLog>>
}

impl StarLogger
{
    pub fn new() -> Self
    {
        StarLogger{
            tx: vec!()
        }
    }

    pub fn log( &mut self, log: StarLog )
    {
        self.tx.retain( |sender| {
            if let Err(SendError(_)) = sender.send(log.clone())
            {
                true
            }
            else {
                false
            }
        });
    }
}

pub static MAX_HOPS: i32 = 32;

pub struct Star
{
    info: StarInfo,
    command_rx: mpsc::Receiver<StarCommand>,
    manager_tx: mpsc::Sender<SubCommand>,
    lanes: HashMap<StarKey, LaneMeta>,
    connector_ctrls: Vec<ConnectorController>,
    transactions: HashMap<Id,Box<dyn Transaction>>,
    frame_hold: FrameHold,
    logger: StarLogger,
    watches: HashMap<ResourceKey,HashMap<Id,StarWatchInfo>>,
    resource_locations: LruCache<ResourceKey,ResourceLocation>,
    app_locations: LruCache<Id,StarKey>,
    resources: HashSet<ResourceKey>,
    core_tx: mpsc::Sender<CoreCommand>,
}

impl Star
{

    pub fn from_proto(info: StarInfo,
                      command_rx: mpsc::Receiver<StarCommand>,
                      manager_tx: mpsc::Sender<SubCommand>,
                      core_tx: mpsc::Sender<CoreCommand>,
                      lanes: HashMap<StarKey,LaneMeta>,
                      connector_ctrls: Vec<ConnectorController>,
                      logger: StarLogger,
                      frame_hold: FrameHold ) ->Self

    {
        Star{
            info: info,
            command_rx: command_rx,
            manager_tx: manager_tx,
            lanes: lanes,
            connector_ctrls: connector_ctrls,
            transactions: HashMap::new(),
            frame_hold: frame_hold,
            logger: logger,
            watches: HashMap::new(),
            resource_locations: LruCache::new(64*1024 ),
            app_locations: LruCache::new(4*1024 ),
            resources: HashSet::new(),
            core_tx: core_tx
        }
    }

    pub fn has_resource( &self, key: &ResourceKey ) -> bool
    {
        self.resources.contains(&key)
    }


    pub async fn run(mut self)
    {
        self.on_init();
        loop {
            let mut futures = vec!();
            let mut lanes = vec!();

            for (key,mut lane) in &mut self.lanes
            {
                futures.push( lane.lane.incoming.recv().boxed() );
                lanes.push( key.clone() )
            }


            futures.push( self.command_rx.recv().boxed());

            let (command,index,_) = select_all(futures).await;

            if let Some(command) = command
            {
                match command{
                    StarCommand::AddLane(lane) => {
                        if let Some(remote_star)=lane.remote_star.as_ref()
                        {
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));

                            if self.info.kind.is_central()
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
                    StarCommand::AddResourceLocation(add_resource_location) => {
                        self.resource_locations.put( add_resource_location.resource_location.resource.clone(), add_resource_location.resource_location.clone() );
                        add_resource_location.tx.send( ()).await;
                    }
                    StarCommand::AddAppLocation(add_app_location) => {
                        self.app_locations.put( add_app_location.app_location.app_id.clone(), add_app_location.app_location.supervisor.clone() );
                        add_app_location.tx.send( () ).await;
                    }
                    StarCommand::ReleaseHold(star) => {
                        if let Option::Some(frames) = self.frame_hold.release(&star)
                        {
println!("RELEASING HOLD!");
                            for frame in frames
                            {
                                self.send_frame(star.clone(),frame).await;
                            }
                        }
                    }
                    StarCommand::AddLogger(tx) => {
                        self.logger.tx.push(tx);
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
                    StarCommand::Search(search) =>
                    {
                        self.do_search(search).await;
                    }
                    StarCommand::SearchCommit(commit) =>
                    {
                        for lane in commit.result.hits.keys()
                        {
                            let hits = commit.result.hits.get(lane).unwrap();
                            for (star,size) in hits
                            {
                                self.lanes.get_mut(lane).unwrap().star_paths.put(star.clone(),size.clone() );

                            }
                        }
                        commit.tx.send( commit.result );
                    }
                    StarCommand::Frame(frame) => {
                        let lane_key = lanes.get(index).unwrap().clone();
                        self.process_frame(frame, lane_key ).await;
                    }
                    StarCommand::ForwardFrame(forward) => {
                        self.send_frame( forward.to.clone(), forward.frame ).await;
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
    async fn search_for_star( &mut self, star: StarKey, tx: oneshot::Sender<SearchResult> )
    {
        let search = Search{
            pattern: StarSearchPattern::StarKey(star),
            tx: tx,
            max_hops: 16
        };
        self.do_search(search);
    }

    async fn do_search( &mut self, search: Search )
    {
        let tx = search.tx;
        let search = StarSearchInner{
            from: self.info.star_key.clone(),
            pattern: search.pattern,
            hops: vec!(),
            transactions: vec!(),
            max_hops: MAX_HOPS
        };

        self.do_search_with_hops(search, tx, Option::None).await;
    }

    async fn do_search_with_hops( &mut self, mut search: StarSearchInner, tx: oneshot::Sender<SearchResult>, exclude: Option<HashSet<StarKey>> )
    {
        let tid = self.info.sequence.next();

        let transaction = Box::new(StarSearchTransaction::new(search.pattern.clone(), self.info.command_tx.clone(), tx, self.lanes.len() ));
        self.transactions.insert(tid.clone(), transaction );

        search.transactions.push(tid.clone());
        search.hops.push( self.info.star_key.clone() );

        self.broadcast_excluding(Frame::StarSearch(search), &exclude ).await;
    }




    async fn on_star_search_hop(&mut self, mut search: StarSearchInner, lane_key: StarKey )
    {
        let hit = match &search.pattern
        {
            StarSearchPattern::StarKey(star) => {
                self.info.star_key == *star
            }
            StarSearchPattern::StarKind(kind) => {
                self.info.kind == *kind
            }
        };

        if hit
        {
            if search.pattern.is_single_match()
            {
                let hops = search.hops.len() + 1;
                let results = Frame::StarSearchResult( StarSearchResultInner {
                    missed: None,
                    hops: search.hops.clone(),
                    hits: vec![ SearchHit { star: self.info.star_key.clone(), hops: hops as _ } ],
                    search: search.clone(),
                    transactions: search.transactions.clone()
                });

                let lane = self.lanes.get_mut(&lane_key).unwrap();
                lane.lane.outgoing.tx.send(LaneCommand::Frame(results)).await;
                return;
            }
            else {
                // create a SearchTransaction here.
                // gather ALL results into this transaction
            }
        }

        if search.max_hops > MAX_HOPS
        {
            eprintln!("rejecting a search with more than maximum {} hops", MAX_HOPS);
        }

        if (search.hops.len() as i32)+1 > search.max_hops || self.lanes.len() <= 1
        {
            eprintln!("search has reached maximum hops... need to send not found");
        }

        let mut exclude = HashSet::new();
        exclude.insert( lane_key );

        let (tx,rx) = oneshot::channel();

        self.do_search_with_hops(search, tx, Option::Some(exclude) );
    }

    pub fn star_key(&self)->&StarKey
    {
        &self.info.star_key
    }

    async fn on_init( &mut self )
    {
        match self.info.kind
           {
            StarKind::Central => {}
            StarKind::Mesh => {}
            StarKind::Supervisor => {}
            StarKind::Server => {
/*                if let Ok(search) = self.search( StarSearchPattern::StarKind(StarKind::Supervisor)).await
                {
                    search.nearest();
                }

 */
            }
            StarKind::Gateway => {}
            StarKind::Link => {}
            StarKind::Client => {}
        }
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


    async fn send(&mut self, message: StarMessageInner )
    {
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
            let command_tx = self.info.command_tx.clone();
            tokio::spawn(async move {
                if let Ok(_)=rx.await
                {
                    command_tx.send( StarCommand::ReleaseHold(star) ).await;
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

    async fn on_star_search_result( &mut self, mut search_result: StarSearchResultInner, lane_key: StarKey )
    {

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

    async fn process_transactions( &mut self, frame: &Frame, lane_key: &StarKey )
    {
        let tid = match frame
        {
            Frame::StarMessage(message) => {
                message.transaction
            },
            Frame::StarSearchResult(result) => {
                result.transactions.last().cloned()
            }
            _ => Option::None
        };

        if let Option::Some(tid) = tid
        {
            let transaction = self.transactions.get_mut(&tid);
            if let Option::Some(transaction) = transaction
            {
                match transaction.on_frame(frame,self.lanes.get_mut(lane_key).unwrap(), &mut self.info.command_tx ).await
                {
                    TransactionResult::Continue => {}
                    TransactionResult::Done => {
                        self.transactions.remove(&tid);
                    }
                }
            }
        }
    }

    async fn process_frame( &mut self, frame: Frame, lane_key: StarKey )
    {
        self.process_transactions(&frame,&lane_key).await;
        match frame
        {
            Frame::Proto(proto) => {
              match &proto
              {
                  ProtoFrame::CentralSearch => {
                      if self.info.kind.is_central()
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
                          let command_tx = self.info.command_tx.clone();
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
                      let mut subgraph = self.info.star_key.subgraph.clone();
                      subgraph.push( self.info.star_key.index.clone() );
                      self.send_frame(lane_key.clone(), Frame::Proto(ProtoFrame::GrantSubgraphExpansion(subgraph))).await;
                  }
                  _ => {}

              }

            }
            Frame::StarSearch(search) => {
                self.on_star_search_hop(search, lane_key).await;
            }
            Frame::StarSearchResult(result) => {
                self.on_star_search_result(result, lane_key ).await;
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
            Frame::StarWind(wind) => {
                self.on_wind(wind).await;
            }
            Frame::StarUnwind(unwind) => {
                self.on_unwind(unwind).await;
            }
            _ => {
                eprintln!("star does not handle frame: {}", frame)
            }
        }
    }

    async fn on_event( &mut self, event: ResourceEvent, lane_key: StarKey  )
    {
        let watches = self.watches.get(&event.resource );

        if watches.is_some()
        {
            let watches = watches.unwrap();
            let mut stars: HashSet<StarKey> = watches.values().map( |info| info.lane.clone() ).collect();
            // just in case! we want to avoid a loop condition
            stars.remove( &lane_key );

            for lane in stars
            {
                self.send_frame( lane.clone(), Frame::ResourceEvent(event.clone()));
            }
        }
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
                if let Option::Some(watches) = self.watches.get_mut(&info.resource )
                {
                    watches.remove(&info.id);
                    if watches.is_empty()
                    {
                        self.watches.remove( &info.resource );
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
        match self.watches.get_mut(&watch_info.resource )
        {
            None => {
                let mut watches = HashMap::new();
                watches.insert(watch_info.id.clone(), star_watch);
                self.watches.insert(watch_info.resource.clone(), watches);
            }
            Some(mut watches) => {
                watches.insert(watch_info.id.clone(), star_watch);
            }
        }
    }

    async fn forward_watch( &mut self, watch: Watch )
    {
        let has_resource = match &watch
        {
            Watch::Add(info) => {
                self.has_resource(&info.resource)
            }
            Watch::Remove(info) => {
                self.has_resource(&info.resource)
            }
        };

        let resource = match &watch
        {
            Watch::Add(info) => {
                &info.resource
            }
            Watch::Remove(info) => {
                &info.resource
            }
        };

        if has_resource
        {
            self.core_tx.send(CoreCommand::Watch(watch)).await;
        }
        else
        {
            let lookup = ResourceLookupKind::Key(resource.clone());
            let location = self.get_resource_location(lookup.clone() );


            if let Some(location) = location.cloned()
            {
                self.send_frame(location.star.clone(), Frame::Watch(watch)).await;
            }
            else
            {
                let mut rx = self.find_resource_location(lookup).await;
                let command_tx = self.info.command_tx.clone();
                tokio::spawn( async move {
                    if let Option::Some(_) = rx.recv().await
                    {
                        command_tx.send(StarCommand::Frame(Frame::Watch(watch))).await;
                    }
                });
            }
        }
    }
    fn get_app_location(&mut self, app_id: &Id ) -> Option<&StarKey>
    {
        self.app_locations.get(app_id)
    }

    async fn find_app_location(&mut self, app_id: &Id ) -> mpsc::Receiver<()>
    {
        let payload = StarMessagePayload::ApplicationRequestSupervisor(ApplicationRequestSupervisorInner{ app_id: app_id.clone() } );
        let mut message = StarMessageInner::new(self.info.sequence.next(), self.info.star_key.clone(), StarKey::central(), payload );
        message.transaction = Option::Some(self.info.sequence.next());

        let (transaction,rx) = ApplicationSupervisorSearchTransaction::new(app_id.clone());
        let transaction = Box::new(transaction);
        self.transactions.insert( message.transaction.unwrap().clone(), transaction );

        self.send( message ).await;

        rx
    }

    fn get_resource_location( &mut self, kind: ResourceLookupKind ) -> Option<&ResourceLocation>
    {
        if let ResourceLookupKind::Key(resource) = &kind
        {
            self.resource_locations.get(resource)
        }
        else {
            Option::None
        }
    }
    async fn find_resource_location(&mut self, kind: ResourceLookupKind ) -> mpsc::Receiver<()>
    {

        let supervisor_star = self.get_app_location(&kind.app_id() ).cloned();

        match supervisor_star{
            None => {
                let rx = self.find_app_location(&kind.app_id()).await;
                rx
            }
            Some(supervisor_star) => {
                let payload = StarMessagePayload::ResourceRequestLocation(ResourceRequestLocation{ lookup: kind } );
                let mut message = StarMessageInner::new( self.info.sequence.next(), self.info.star_key.clone(), supervisor_star, payload );
                message.transaction = Option::Some(self.info.sequence.next());
                let (transaction,rx) =  ResourceLocationRequestTransaction::new();
                self.transactions.insert( message.transaction.unwrap().clone(), Box::new(transaction) );
                self.send( message ).await;
                rx
            }
        }

    }



    async fn on_wind( &mut self, mut wind: StarWindInner)
    {
        if wind.to != self.info.star_key
        {
            if self.info.kind.relay()
            {
                wind.stars.push( self.info.star_key.clone() );
                self.send_frame(wind.to.clone(), Frame::StarWind(wind)).await;
            }
            else {
                eprintln!("this star does not relay messages");
            }
        }
        else {
            let star_stack = wind.stars.clone();
            self.manager_tx.send(SubCommand::Frame(Frame::StarWind(wind)) ).await;
            /*{
                Ok(payload) => {
                    let unwind = StarUnwindInner{
                        stars: star_stack.clone(),
                        payload: payload
                    };
                    self.send_frame(star_stack.last().unwrap().clone(), Frame::StarUnwind(unwind) ).await;
                }
                Err(error) => {
                    eprintln!("encountered handle_wind error: {}", error );
                }
            };

             */
        }
    }

    async fn on_unwind( &mut self, mut unwind: StarUnwindInner)
    {
        if unwind.stars.len() > 1
        {
            unwind.stars.pop();
            if self.info.kind.relay()
            {
                let star = unwind.stars.last().unwrap().clone();
                self.send_frame(star, Frame::StarUnwind(unwind)).await;
            }
            else {
                return eprintln!("this star does not relay messages");
            }
        }
    }

    async fn on_message( &mut self, mut message: StarMessageInner ) -> Result<(),Error>
    {
        if message.to != self.info.star_key
        {
            if self.info.kind.relay()
            {
                self.send(message).await;
                return Ok(());
            }
            else {
                return Err("this star does not relay messages".into())
            }
        }
        else {
            Ok(self.manager_tx.send( SubCommand::Frame( Frame::StarMessage(message))).await?)
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
    AddResourceLocation(AddResourceLocation),
    AddAppLocation(AddAppLocation),
    AddLogger(broadcast::Sender<StarLog>),
    ReleaseHold(StarKey),
    Search(Search),
    SearchCommit(SearchCommit),
    Test(StarTest),
    Frame(Frame),
    ForwardFrame(ForwardFrame),
    FrameTimeout(FrameTimeoutInner),
    FrameError(FrameErrorInner)
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


pub struct AddAppLocation
{
    pub tx: mpsc::Sender<()>,
    pub app_location: AppLocation
}


pub struct Search
{
    pub pattern: StarSearchPattern,
    pub tx: oneshot::Sender<SearchResult>,
    pub max_hops: usize
}

impl Search
{
    pub fn new( pattern: StarSearchPattern, on_result: fn(result: Option<SearchResult>)->Option<StarCommand> ) -> (Self,oneshot::Receiver<SearchResult>)
    {
        let (tx,rx) = oneshot::channel();
        (Search{
           pattern: pattern,
           tx: tx,
           max_hops: 16,
          } ,rx )
    }
}

pub enum SubCommand
{
    Frame(Frame),
    SupervisorCommand(SupervisorCommand),
    ServerCommand(ServerCommand),
}

pub enum CentralCommand
{

}

pub enum SupervisorCommand
{
    PledgeToCentral
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


impl fmt::Display for SubCommand{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = match self {
            SubCommand::Frame(_) => "Frame".to_string(),
            SubCommand::SupervisorCommand(_) => "SupervisorCommand".to_string(),
            SubCommand::ServerCommand(_) => "ServerCommand".to_string()
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
            StarCommand::Search(_) => format!("Search").to_string(),
            StarCommand::SearchCommit(_) => format!("SearchResult").to_string(),
            StarCommand::ReleaseHold(_) => format!("ReleaseHold").to_string(),
            StarCommand::AddResourceLocation(_) => format!("AddResourceLocation").to_string(),
            StarCommand::AddAppLocation(_) => format!("AddAppLocation").to_string(),
            StarCommand::ForwardFrame(_) => format!("ForwardFrame").to_string(),
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone)]
pub struct StarController
{
    pub command_tx: mpsc::Sender<StarCommand>
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
    pub app_id: Id,
    pub tx: mpsc::Sender<()>
}

impl ApplicationSupervisorSearchTransaction
{
    pub fn new(app_id: Id) ->(Self,mpsc::Receiver<()>)
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
    async fn on_frame(&mut self, frame: &Frame, lane: &mut LaneMeta, command_tx: &mut Sender<StarCommand>) -> TransactionResult {

        if let Frame::StarMessage( message ) = frame
        {
            if let StarMessagePayload::ApplicationReportSupervisor(report) = &message.payload
            {
                command_tx.send( StarCommand::AddAppLocation(AddAppLocation{
                    tx: self.tx.clone(),
                    app_location: AppLocation{
                        app_id: report.app_id.clone(),
                        supervisor: report.supervisor.clone()
                    }
                })).await;
            }
        }

        TransactionResult::Done
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
    async fn on_frame(&mut self, frame: &Frame, lane: &mut LaneMeta, command_tx: &mut Sender<StarCommand>) -> TransactionResult {

        if let Frame::StarMessage( message ) = frame
        {
            if let StarMessagePayload::ResourceReportLocation(location ) = &message.payload
            {
                command_tx.send( StarCommand::AddResourceLocation(AddResourceLocation{ tx: self.tx.clone(), resource_location: location.clone() })).await;
            }
        }

        TransactionResult::Done
    }

}


pub struct StarSearchTransaction
{
    pub pattern: StarSearchPattern,
    pub reported_lane_count: usize,
    pub lanes: usize,
    pub hits: HashMap<StarKey, HashMap<StarKey,usize>>,
    command_tx: mpsc::Sender<StarCommand>,
    tx: Vec<oneshot::Sender<SearchResult>>
}

impl StarSearchTransaction
{
    pub fn new(pattern: StarSearchPattern, command_tx: mpsc::Sender<StarCommand>, tx: oneshot::Sender<SearchResult>, lanes: usize ) ->Self
    {
        StarSearchTransaction{
            pattern: pattern,
            reported_lane_count: 0,
            hits: HashMap::new(),
            command_tx: command_tx,
            tx: vec!(tx),
            lanes: lanes
        }
    }

    pub async fn commit(&mut self)
    {
        if self.tx.len() != 0
        {
            let tx = self.tx.remove(0);
            let commit = SearchCommit {
                tx: tx,
                result: SearchResult
                {
                    pattern: self.pattern.clone(),
                    hits: self.hits.clone()
                }
            };

            self.command_tx.send(StarCommand::SearchCommit(commit)).await;
        }
    }
}

#[async_trait]
impl Transaction for StarSearchTransaction
{
    async fn on_frame(&mut self, frame: &Frame, lane: &mut LaneMeta, command_tx: &mut Sender<StarCommand>) -> TransactionResult {

        if let Frame::StarSearchResult(result) = frame
        {
            let mut lane_hits = HashMap::new();

            for hit in &result.hits
            {
                if !lane_hits.contains_key(&hit.star )
                {
                    lane_hits.insert( hit.star.clone(), hit.hops );
                }
                else
                {
                    if let Option::Some(old) = lane_hits.get( &hit.star )
                    {
                        if hit.hops < *old
                        {
                            lane_hits.insert( hit.star.clone(), hit.hops );
                        }
                    }
                }
            }
            self.hits.insert( lane.lane.remote_star.clone().unwrap(), lane_hits );
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

pub struct LaneHit{
    lane: StarKey,
    star: StarKey,
    hops: usize
}

pub struct SearchCommit
{
    pub result: SearchResult,
    pub tx: oneshot::Sender<SearchResult>
}


#[derive(Clone)]
pub struct SearchResult
{
    pub pattern: StarSearchPattern,
    pub hits: HashMap<StarKey, HashMap<StarKey,usize>>,
}

impl SearchResult
{
   pub fn nearest(&self)->Option<SearchHit>
   {
       let mut min = Option::None;
       for lane in self.hits.values()
       {
           for (star,hops) in lane
           {
               let hit = SearchHit{
                   star: star.clone(),
                   hops: hops.clone()
               };
              if min.is_none()
              {
                  min = Option::Some(hit);
              }
              else if let Option::Some(prev) = &min
              {
                  if hit.hops < prev.hops
                  {
                      min = Option::Some(hit)
                  }
              }

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
    async fn on_frame( &mut self, frame: &Frame, lane: &mut LaneMeta, command_tx: &mut mpsc::Sender<StarCommand> )-> TransactionResult;
}

#[derive(Clone)]
pub enum StarLog
{
   StarSearchInitialized(StarSearchInner),
   StarSearchResult(StarSearchResultInner),
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
    async fn handle(&mut self, command: SubCommand) -> Result<(),Error>;
}

pub struct CentralManager
{
    info: StarInfo,
    backing: Box<dyn CentralManagerBacking>
}

impl CentralManager
{
    pub fn new(info: StarInfo )->CentralManager
    {
        CentralManager
        {
            info: info.clone(),
            backing: Box::new( CentralManagerBackingDefault::new(info) )
        }
    }
}

#[async_trait]
impl StarManager for CentralManager
{
    async fn handle(&mut self, command: SubCommand ) -> Result<(), Error> {

        if let SubCommand::Frame(Frame::StarMessage(message)) = command
        {
            let mut message = message;
            match &message.payload
            {
                StarMessagePayload::SupervisorPledgeToCentral => {
                    self.backing.add_supervisor(message.from.clone());
                    Ok(())
                }
                StarMessagePayload::ApplicationCreateRequest(request) => {
                    let app_id = self.info.sequence.next();
                    let supervisor = self.backing.select_supervisor();
                    if let Option::None = supervisor
                    {
                        message.reply(self.info.sequence.next(), StarMessagePayload::Reject(RejectionInner { message: "no supervisors available to host application.".to_string() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    } else {
                        if let Some(name) = &request.name
                        {
                            self.backing.set_application_name_to_app_id(name.clone(),app_id.clone());
                        }
                        let message = StarMessageInner {
                            id: self.info.sequence.next(),
                            from: self.info.star_key.clone(),
                            to: supervisor.unwrap(),
                            transaction: message.transaction.clone(),
                            payload: StarMessagePayload::ApplicationAssign(ApplicationAssignInner {
                                app_id: app_id,
                                data: request.data.clone(),
                                notify: vec![message.from, self.info.star_key.clone()]
                            }),
                            retry: 0,
                            max_retries: 16
                        };
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    }
                }
                StarMessagePayload::ApplicationNotifyReady(notify) => {
                    self.backing.set_application_state(notify.app_id.clone(), ApplicationState::Ready);
                    Ok(())
                    // do nothing
                }
                StarMessagePayload::ApplicationRequestSupervisor(request) => {
                    if let Option::Some(supervisor) = self.backing.select_supervisor()
                    {
                        message.reply(self.info.sequence.next(), StarMessagePayload::ApplicationReportSupervisor(ApplicationReportSupervisorInner { app_id: request.app_id, supervisor: supervisor.clone() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    } else {
                        message.reply(self.info.sequence.next(), StarMessagePayload::Reject(RejectionInner { message: format!("cannot find app_id: {}", request.app_id).to_string() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    }
                }
                StarMessagePayload::ApplicationLookupId(request) => {
                    let app_id = self.backing.get_application_for_name(&request.name);
                    if let Some(app_id) = app_id
                    {
                        if let Option::Some(supervisor) = self.backing.get_application_for_supervisor(&app_id) {
                            message.reply(self.info.sequence.next(), StarMessagePayload::ApplicationReportSupervisor(ApplicationReportSupervisorInner { app_id: app_id.clone(), supervisor: supervisor.clone() }));
                            self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                            Ok(())
                        } else {
                            self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                            Ok(())
                        }
                    } else {
                        message.reply(self.info.sequence.next(), StarMessagePayload::Reject(RejectionInner { message: format!("could not find app_id for lookup name: {}", request.name).to_string() }));
                        self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(message))).await?;
                        Ok(())
                    }
                    // return this if both conditions fail
                }
                _ => {
                    Err("unimplemented".into())
                }
            }
        }
        else if let SubCommand::Frame(Frame::StarWind(wind)) = &command {
            match wind.payload
            {
                StarWindPayload::RequestSequence => {
                    let payload = StarUnwindPayload::AssignSequence(self.backing.sequence_next().index);
                    let inner = StarUnwindInner{
                        stars: wind.stars.clone(),
                        payload: payload
                    };

                    self.info.command_tx.send( StarCommand::ForwardFrame(ForwardFrame{ to: inner.stars.last().cloned().unwrap(), frame: Frame::StarUnwind(inner)})).await;

                    Ok(())
                }
            }
        }
        else {
            Err(format!("{} cannot handle command {}",self.info.kind,command).into() )
        }
    }

}

trait CentralManagerBacking: Send+Sync
{
    fn sequence_next(&mut self)->Id;
    fn add_supervisor(&mut self, star: StarKey );
    fn remove_supervisor(&mut self, star: StarKey );
    fn set_application_for_supervisor(&mut self, app_id: Id, supervisor_star: StarKey );
    fn get_application_for_supervisor(&self, app_id: &Id) -> Option<&StarKey>;
    fn set_application_name_to_app_id(&mut self, name: String, app_id: Id );
    fn set_application_state(&mut self,  app_id: Id, state: ApplicationState );
    fn get_application_state(&self,  app_id: &Id ) -> Option<&ApplicationState>;
    fn get_application_for_name(&self,  name: &String ) -> Option<&Id>;
    fn select_supervisor(&mut self )->Option<StarKey>;
}


pub struct CentralManagerBackingDefault
{
    info: StarInfo,
    supervisors: Vec<StarKey>,
    application_to_supervisor: HashMap<Id,StarKey>,
    application_name_to_app_id : HashMap<String,Id>,
    application_state: HashMap<Id,ApplicationState>,
    supervisor_index: usize
}

impl CentralManagerBackingDefault
{
    pub fn new( info: StarInfo ) -> Self
    {
        CentralManagerBackingDefault {
            info: info,
            supervisors: vec![],
            application_to_supervisor: HashMap::new(),
            application_name_to_app_id: HashMap::new(),
            application_state: HashMap::new(),
            supervisor_index: 0
        }
    }
}

impl CentralManagerBacking for CentralManagerBackingDefault
{
    fn sequence_next(&mut self) -> Id {
        self.info.sequence.next()
    }

    fn add_supervisor(&mut self, star: StarKey) {
        if !self.supervisors.contains(&star)
        {
            self.supervisors.push(star);
        }
    }

    fn remove_supervisor(&mut self, star: StarKey) {
        self.supervisors.retain( |s| *s != star );
    }

    fn set_application_for_supervisor(&mut self, app_id: Id, supervisor_star: StarKey) {
        self.application_to_supervisor.insert( app_id, supervisor_star );
    }

    fn get_application_for_supervisor(&self, app_id: &Id) -> Option<&StarKey> {
        self.application_to_supervisor.get(app_id)
    }

    fn set_application_name_to_app_id(&mut self, name: String, app_id: Id) {
        self.application_name_to_app_id.insert( name, app_id );
    }

    fn set_application_state(&mut self, app_id: Id, state: ApplicationState) {
        self.application_state.insert( app_id, state );
    }

    fn get_application_state(&self, app_id: &Id)->Option<&ApplicationState> {
        self.application_state.get( app_id )
    }

    fn get_application_for_name(&self, name: &String) -> Option<&Id> {
        self.application_name_to_app_id.get(name)
    }


    fn select_supervisor(&mut self) -> Option<StarKey> {
        if self.supervisors.len() == 0
        {
            return Option::None;
        }
        else {
            self.supervisor_index = self.supervisor_index + 1;
            return self.supervisors.get(self.supervisor_index%self.supervisors.len()).cloned();
        }
    }
}

pub struct SupervisorManager
{
    info: StarInfo,
    backing: Box<dyn SupervisorManagerBacking>
}

impl SupervisorManager
{
    pub fn new(info: StarInfo)->Self
    {
        SupervisorManager{
            info: info.clone(),
            backing: Box::new(SupervisorManagerBackingDefault::new(info)),
        }
    }
}

#[async_trait]
impl StarManager for SupervisorManager
{
    async fn handle(&mut self, command: SubCommand) -> Result<(), Error> {
        match command
        {
            SubCommand::Frame(frame) => {
                match frame {
                    Frame::StarMessage(message) => {
                        self.handle_message(message).await
                    }
                    _ => Err(format!("{} manager does not know how to handle frame: {}", self.info.kind, frame).into())
                }
            }
            SubCommand::SupervisorCommand(command) => {
                if let SupervisorCommand::PledgeToCentral = command
                {
                    let message = StarMessageInner::new(self.info.sequence.next(), self.info.star_key.clone(), StarKey::central(), StarMessagePayload::SupervisorPledgeToCentral );
                    Ok(self.info.command_tx.send( StarCommand::Frame(Frame::StarMessage(message))).await?)
                }
                else {
                    Err(format!("{} manager does not know how to handle : ...", self.info.kind).into())
                }
            }
            SubCommand::ServerCommand(_) => {
                Err(format!("{} manager does not know how to handle : {}", self.info.kind, command).into())
            }
        }
    }
}


impl SupervisorManager
{
    async fn handle_message(&mut self, message: StarMessageInner) -> Result<(), Error> {

        let mut message = message;
        match &message.payload
        {
            StarMessagePayload::ApplicationAssign(assign) => {
                self.backing.add_application(assign.app_id.clone(), assign.data.clone());

                // TODO: Now we need to Launch this application in the ext
                // ext.launch_app()

                for notify in &assign.notify
                {
                    let notify_app_ready = StarMessageInner::new(self.info.sequence.next(), self.info.star_key.clone(), notify.clone(), StarMessagePayload::ApplicationNotifyReady(ApplicationNotifyReadyInner{app_id:assign.app_id.clone()}));
                    self.info.command_tx.send(StarCommand::Frame(Frame::StarMessage(notify_app_ready))).await?;
                }

                Ok(())
            }
            StarMessagePayload::ServerPledgeToSupervisor => {

                self.backing.add_server(message.from.clone());
                Ok(())
            }
            StarMessagePayload::ResourceReportLocation(report) =>
                {
                    self.backing.set_resource_location(report.resource.clone(),report.clone());
                    Ok(())
                }
            StarMessagePayload::ResourceRequestLocation(request) =>
                {

                    let location = self.backing.get_resource_location(&request.lookup);

                    match location
                    {
                        None => {
                            return Err(format!("cannot find resource: {}", request.lookup).into() );
                        }
                        Some(location) => {
                            let location = location.clone();
                            let payload = StarMessagePayload::ResourceReportLocation(location);
                            message.reply( self.info.sequence.next(), payload );
                            self.info.command_tx.send( StarCommand::Frame(Frame::StarMessage(message))).await?;
                        }
                    }
                    Ok(())
                }
            _ => {
                Err("SupervisorCore does not handle message of this type: _".into())
            }
        }
    }
}

pub trait SupervisorManagerBacking: Send+Sync
{
    fn add_server( &mut self, server: StarKey );
    fn remove_server( &mut self, server: &StarKey );
    fn select_server(&mut self) -> Option<StarKey>;

    fn add_application( &mut self, app_id: Id , data: Vec<u8> );
    fn remove_application( &mut self, app_id: Id );

    fn set_resource_name(&mut self, name: String, key: ResourceKey );
    fn set_resource_location(&mut self, resource: ResourceKey, location: ResourceLocation );
    fn get_resource_location(&self, lookup: &ResourceLookupKind) -> Option<&ResourceLocation>;
}

pub struct SupervisorManagerBackingDefault
{
    info: StarInfo,
    servers: Vec<StarKey>,
    server_select_index: usize,
    applications: HashSet<Id>,
    name_to_resource: HashMap<String,ResourceKey>,
    resource_location: HashMap<ResourceKey,ResourceLocation>
}

impl SupervisorManagerBackingDefault
{
    pub fn new(info: StarInfo)->Self
    {
        SupervisorManagerBackingDefault {
            info: info,
            servers: vec![],
            server_select_index: 0,
            applications: HashSet::new(),
            name_to_resource: HashMap::new(),
            resource_location: HashMap::new(),
        }
    }
}

impl SupervisorManagerBacking for SupervisorManagerBackingDefault
{
    fn add_server(&mut self, server: StarKey) {
        self.servers.push(server);
    }

    fn remove_server(&mut self, server: &StarKey) {
        self.servers.retain(|star| star != server );
    }

    fn select_server(&mut self) -> Option<StarKey> {
        if self.servers.len() == 0
        {
            return Option::None;
        }
        self.server_select_index = self.server_select_index +1;
        let server = self.servers.get( self.server_select_index % self.servers.len() ).unwrap();
        Option::Some(server.clone())
    }

    fn add_application(&mut self, app_id: Id, data: Vec<u8>) {
        self.applications.insert(app_id);
    }

    fn remove_application(&mut self, app_id: Id) {
        self.applications.remove(&app_id);
    }

    fn set_resource_name(&mut self, name: String, key: ResourceKey) {
        self.name_to_resource.insert(name,key );
    }

    fn set_resource_location(&mut self, resource: ResourceKey, location: ResourceLocation) {
        self.resource_location.insert( resource, location );
    }

    fn get_resource_location(&self, lookup: &ResourceLookupKind) -> Option<&ResourceLocation> {
        match lookup
        {
            ResourceLookupKind::Key(key) => {
                return self.resource_location.get(key)
            }
            ResourceLookupKind::Name(lookup) => {

                if let Some(key) = self.name_to_resource.get(&lookup.name)
                {
                    return self.resource_location.get(key)
                }
                else {
                    Option::None
                }
            }
        }
    }
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
    info: StarInfo,
    backing: Box<dyn ServerManagerBacking>,
}

impl ServerManager
{
    pub fn new( info: StarInfo ) -> Self
    {
        ServerManager
        {
            info: info,
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
}

#[async_trait]
impl StarManager for ServerManager
{
    async fn handle(&mut self, command: SubCommand) -> Result<(), Error> {
        if let SubCommand::ServerCommand(command) = command
        {
            match command{
            ServerCommand::PledgeToSupervisor => {

unimplemented!();

                let (search,rx) = Search::new(StarSearchPattern::StarKind(StarKind::Supervisor), |blah|{Option::None} );
                self.info.command_tx.send( StarCommand::Search( search ));

            }
            }
        }
        Ok(())
    }
}


pub struct PlaceholderStarManager
{

}

impl PlaceholderStarManager
{
    pub fn new()->Self
    {
        PlaceholderStarManager{}
    }
}

#[async_trait]
impl StarManager for PlaceholderStarManager
{
    async fn handle(&mut self, command: SubCommand ) -> Result<(), Error> {
        Err("unimplemented".into())
    }
}

#[async_trait]
pub trait StarManagerFactory: Sync+Send
{
    async fn create( &self, info: StarInfo ) -> mpsc::Sender<SubCommand>;
}


pub struct StarManagerFactoryDefault
{
}

impl StarManagerFactoryDefault
{
    fn create_inner( &self, info: &StarInfo) -> Box<dyn StarManager>
    {
        if let StarKind::Central = info.kind
        {
            return Box::new(CentralManager::new(info.clone()));
        }
        else if let StarKind::Supervisor= info.kind
        {
            return Box::new(SupervisorManager::new(info.clone()));
        }
        else if let StarKind::Server= info.kind
        {
            return Box::new(ServerManager::new(info.clone()));
        }
        else {
            Box::new(PlaceholderStarManager::new())
        }
    }
}

#[async_trait]
impl StarManagerFactory for StarManagerFactoryDefault
{
    async fn create( &self, info: StarInfo ) -> mpsc::Sender<SubCommand>
    {
        let (mut tx,mut rx) = mpsc::channel(32);
        let mut manager:Box<dyn StarManager> = self.create_inner(&info);

        tokio::spawn( async move {
            while let Option::Some(command) = rx.recv().await
            {
                match manager.handle(command).await
                {
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!("manager error: {}", error);
                    }
                }
            }
        }  );

        tx
    }
}


#[derive(Clone)]
pub struct StarInfo
{
   pub star_key: StarKey,
   pub kind: StarKind,
   pub sequence: Arc<IdSeq>,
   pub command_tx: mpsc::Sender<StarCommand>
}

