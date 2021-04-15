use std::sync::{Mutex, Weak, Arc};
use crate::lane::{Lane, TunnelConnector, OutgoingLane, ConnectorController, LaneMeta, LaneCommand, TunnelConnectorFactory, ConnectionInfo};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI32, AtomicI64 };
use futures::future::join_all;
use futures::future::select_all;
use crate::frame::{ProtoFrame, Frame, StarSearchHit, StarSearchPattern, StarMessageInner, StarMessagePayload, StarSearchInner, StarSearchResultInner, RejectionInner, ApplicationAssignInner, ApplicationReportSupervisorInner};
use crate::error::Error;
use crate::id::{Id, IdSeq};
use futures::FutureExt;
use serde::{Serialize,Deserialize};
use crate::proto::{ProtoTunnel, ProtoStar, PlaceholderKernel};
use std::{fmt, cmp};
use tokio::sync::mpsc::{Sender, Receiver};
use std::cmp::Ordering;
use tokio::sync::mpsc;
use tokio::sync::broadcast;
use crate::frame::Frame::{StarSearch, StarMessage};
use url::Url;
use tokio::sync::broadcast::error::SendError;
use crate::frame::StarMessagePayload::ApplicationCreateRequest;
use crate::frame::ProtoFrame::CentralSearch;

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub enum StarKind
{
    Central,
    Mesh,
    Supervisor,
    Server,
    Gateway,
    Link,
    Client,
    Ext(ExtStarKind)
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
            StarKind::Ext(ext) => ext.relay_messages,
        }
    }
}


#[derive(Clone)]
pub struct StarInfo
{
   pub star_key: StarKey,
   pub kind: StarKind,
   pub sequence: Arc<IdSeq>
}

pub enum StarCore
{
    Central(Box<dyn CentralCore>),
    Mesh,
    Supervisor(Box<dyn Core>),
    Server,
    Gateway,
    Link,
    Client,
    Ext
}

impl StarCore
{
   pub fn kind(&self)->StarKind
   {
       match self{
           StarCore::Central(_) => StarKind::Central,
           StarCore::Mesh => StarKind::Mesh,
           StarCore::Supervisor(_) => StarKind::Supervisor,
           StarCore::Server => StarKind::Server,
           StarCore::Gateway => StarKind::Gateway,
           StarCore::Link => StarKind::Link,
           StarCore::Client => StarKind::Client,
           StarCore::Ext => StarKind::Ext(ExtStarKind{ relay_messages: true })
       }
   }
}

impl Core for StarCore
{
    fn handle(&mut self, message: StarMessageInner) -> Result<Option<Vec<StarMessageInner>>, Error> {
        match self
        {
            StarCore::Central(core) => {
                core.handle(message)
            }
            StarCore::Mesh => {
                Err("this core does not know how to handle this message".into())
            }
            StarCore::Supervisor(core) => {
                core.handle(message)
            }
            StarCore::Server => {
                Err("this core does not know how to handle this message".into())
            }
            StarCore::Gateway => {
                Err("this core does not know how to handle this message".into())
            }
            StarCore::Link => {
                Err("this core does not know how to handle this message".into())
            }
            StarCore::Client => {
                Err("this core does not know how to handle this message".into())
            }
            StarCore::Ext => {
                Err("this core does not know how to handle this message".into())
            }
        }

        // terrible error message...
    }

}

pub trait Core: Send+Sync
{
    fn handle( &mut self, message: StarMessageInner ) -> Result<Option<Vec<StarMessageInner>>,Error>;
}

pub trait CentralCore: Core
{
}

pub struct CentralCoreDefault
{
    info: StarInfo,
    supervisors: Vec<StarKey>,
    sequence: IdSeq,
    application_to_supervisor: HashMap<Id,StarKey>,
    application_name_to_app_id : HashMap<String,Id>,
    supervisor_index: usize
}

impl CentralCoreDefault
{
    pub fn new( info: StarInfo )->Self
    {
        CentralCoreDefault {
            info: info,
            supervisors: vec!(),
            sequence: IdSeq::new(0),
            application_to_supervisor: HashMap::new(),
            application_name_to_app_id: HashMap::new(),
            supervisor_index: 0
        }
    }


    fn select_supervisor( &mut self ) -> Option<StarKey>
    {
        if self.supervisors.len() == 0
        {
            return Option::None;
        }

        self.supervisor_index = self.supervisor_index + 1;

        Option::Some(self.supervisors.get( self.supervisor_index % self.supervisors.len() ).unwrap().clone())
    }

}

impl CentralCore for CentralCoreDefault
{}

impl Core for CentralCoreDefault
{
    fn handle(&mut self, message: StarMessageInner) -> Result<Option<Vec<StarMessageInner>>, Error> {
        let mut message = message;
        match &message.payload
        {
            StarMessagePayload::RequestSequence => {
                message.reply(StarMessagePayload::AssignSequence(self.sequence.next().index));
                Ok(Option::Some(vec![message]))
            }
            StarMessagePayload::SupervisorPledgeToCentral => {
                self.supervisors.push(message.from.clone());
                Ok(Option::None)
            }
            StarMessagePayload::ApplicationCreateRequest(request) => {
                let app_id = self.sequence.next();
                let supervisor = self.select_supervisor();
                if let Option::None = supervisor
                {
                    message.reply(StarMessagePayload::Reject(RejectionInner{ message: "no supervisors available to host application.".to_string()}));
                    Ok(Option::Some(vec![message]))
                }
                else {
                    if let Some(name)=&request.name
                    {
                        self.application_name_to_app_id.insert( name.clone(), app_id.clone() );
                    }
                    let message = StarMessageInner {
                        from: self.info.star_key.clone(),
                        to: supervisor.unwrap(),
                        transaction: message.transaction.clone(),
                        payload: StarMessagePayload::ApplicationAssign( ApplicationAssignInner{
                            app_id: app_id,
                            data: request.data.clone(),
                            notify: vec![message.from,self.info.star_key.clone()]
                        } )
                    };
                    Ok(Option::Some(vec![message]))
                }
            }
            StarMessagePayload::ApplicationNotifyReady(_) => {
                Ok(Option::Some(vec![]))
                // do nothing
            }
            StarMessagePayload::ApplicationRequestSupervisor(request) => {
                if let Option::Some(supervisor) = self.application_to_supervisor.get(&request.app_id ) {
                    message.reply(StarMessagePayload::ApplicationReportSupervisor(ApplicationReportSupervisorInner { app_id: request.app_id, supervisor: supervisor.clone() }));
                    Ok(Option::Some(vec![message]))
                }
                else {
                    message.reply(StarMessagePayload::Reject(RejectionInner{ message: format!("cannot find app_id: {}",request.app_id).to_string() }));
                    Ok(Option::Some(vec![message]))
                }
            }
            StarMessagePayload::ApplicationLookupId(request) => {
                let app_id = self.application_name_to_app_id.get(&request.name );
                if let Some(app_id) = app_id
                {
                    if let Option::Some(supervisor) = self.application_to_supervisor.get(&app_id ) {
                      message.reply(StarMessagePayload::ApplicationReportSupervisor(ApplicationReportSupervisorInner { app_id: app_id.clone(), supervisor: supervisor.clone() }));
                      Ok(Option::Some(vec![message]))
                    }
                    else {
                        Ok(Option::Some(vec![message]))
                    }
                }
                else {
                    message.reply(StarMessagePayload::Reject(RejectionInner{ message: format!("could not find app_id for lookup name: {}",request.name).to_string() }));
                    Ok(Option::Some(vec![message]))
                }
                // return this if both conditions fail

            }
            _ => {
                Err("central does not handle message of this type: _ ".into())
            }
        }
    }

}


pub trait SupervisorCoreBacking: Send+Sync
{
    fn add_server( &mut self, server: StarKey );
    fn remove_server( &mut self, server: &StarKey );
    fn select_server(&mut self) -> Option<StarKey>;

    fn add_application( &mut self, app_id: Id , data: Vec<u8> );
    fn remove_application( &mut self, app_id: Id );
}

pub struct DefaultSupervisorCoreBacking
{
    info: StarInfo,
    servers: Vec<StarKey>,
    server_select_index: usize,
    applications: HashSet<Id>
}

impl DefaultSupervisorCoreBacking
{
    pub fn new(info: StarInfo)->Self
    {
        DefaultSupervisorCoreBacking{
            info: info,
            servers: vec![],
            server_select_index: 0,
            applications: HashSet::new()
        }
    }
}

impl SupervisorCoreBacking for DefaultSupervisorCoreBacking
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
}

pub struct SupervisorCore
{
    info: StarInfo,
    backing: Box<dyn SupervisorCoreBacking>
}

impl SupervisorCore
{
    pub fn new(info: StarInfo)->Self
    {
       SupervisorCore{
           info: info.clone(),
           backing: Box::new(DefaultSupervisorCoreBacking::new(info))
       }
    }
}

impl Core for SupervisorCore
{
    fn handle(&mut self, message: StarMessageInner) -> Result<Option<Vec<StarMessageInner>>, Error> {

        match &message.payload
        {
            StarMessagePayload::ApplicationAssign(assign) => {
                self.backing.add_application(assign.app_id.clone(), assign.data.clone());

                // Now we need to Launch this application in the selected server

                Ok(Option::None)
            }
            StarMessagePayload::ServerPledgeToSupervisor => {

                self.backing.add_server(message.from.clone());
                Ok(Option::None)
            }
            _ => {
                Err("SupervisorCore does not handle message of this type: _".into())
            }
        }

    }
}

pub trait StarCoreProvider: Send+Sync
{
    fn provide( &self, info: StarInfo ) -> StarCore;
}



pub struct DefaultStarCoreProvider
{

}

impl DefaultStarCoreProvider
{
    pub fn new()->Self
    {
        DefaultStarCoreProvider{}
    }
}

impl StarCoreProvider for DefaultStarCoreProvider
{
    fn provide(&self, info: StarInfo ) -> StarCore {

        match &info.kind{
            StarKind::Central => StarCore::Central(Box::new(CentralCoreDefault::new(info.clone()))),
            StarKind::Mesh => StarCore::Mesh,
            StarKind::Supervisor => StarCore::Supervisor(Box::new(SupervisorCore::new(info.clone()))),
            StarKind::Server => StarCore::Server,
            StarKind::Gateway => StarCore::Gateway,
            StarKind::Link => StarCore::Link,
            StarKind::Client => StarCore::Client,
            StarKind::Ext(_) => StarCore::Ext
        }
    }
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ServiceData
{
    pub port: u16
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct GatewayKind
{

}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone, Serialize, Deserialize)]
pub struct ExtStarKind
{
    relay_messages: bool
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
    pub kind: StarKind,
    pub key: StarKey,
    core: StarCore,
    command_rx: Receiver<StarCommand>,
    lanes: HashMap<StarKey, LaneMeta>,
    connector_ctrls: Vec<ConnectorController>,
    transactions: HashMap<i64,Box<dyn Transaction>>,
    transaction_seq: AtomicI64,
    star_search_transactions: HashMap<i64,StarSearchTransaction>,
    frame_hold: FrameHold,
    logger: StarLogger
}

impl Star
{
    /*
    pub fn new(key: StarKey, core: StarCore) ->(Self, StarController)
    {
        let (command_tx, command_rx) = mpsc::channel(32);
        (Star{
            kind: core.kind(),
            core: core,
            key,
            command_rx: command_rx,
            lanes: HashMap::new(),
            connector_ctrls: vec![],
            sequence: Option::None,
            transactions: HashMap::new(),
            transaction_seq: AtomicI64::new(0),
            star_search_transactions: HashMap::new(),
            frame_hold: HashMap::new(),
            logger: StarLogger::new()
        }, StarController{
            command_tx: command_tx
        })
    }

     */

    pub fn from_proto(key: StarKey, core: StarCore, command_rx: Receiver<StarCommand>, lanes: HashMap<StarKey,LaneMeta>, connector_ctrls: Vec<ConnectorController>, logger: StarLogger, frame_hold: FrameHold ) ->Self
    {
        Star{
            kind: core.kind(),
            core: core,
            key,
            command_rx: command_rx,
            lanes: lanes,
            connector_ctrls: connector_ctrls,
            transactions: HashMap::new(),
            transaction_seq: AtomicI64::new(0),
            star_search_transactions: HashMap::new(),
            frame_hold: frame_hold,
            logger: logger
        }
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

            futures.push(self.command_rx.recv().boxed() );

            let (command,index,_) = select_all(futures).await;

            if let Some(command) = command
            {
                match command{
                    StarCommand::AddLane(lane) => {
                        if let Some(remote_star)=lane.remote_star.as_ref()
                        {
                            self.lanes.insert(remote_star.clone(), LaneMeta::new(lane));

                            if self.kind.is_central()
                            {
                                self.broadcast( Frame::Proto(ProtoFrame::CentralFound(1)), &Option::None ).await;
                            }

                        }
                        else {
                            eprintln!("for star remote star must be set");
                         }
                    }
                    StarCommand::AddConnectorController(connector_ctrl) => {
                        self.connector_ctrls.push(connector_ctrl);
                    }
                    StarCommand::AddLogger(tx) => {
                        self.logger.tx.push(tx);
                    }
                    StarCommand::Test(test) => {
                        match test
                        {
                            StarTest::StarSearchForStarKey(star) => {
                                self.search_for_star(star).await;
                            }
                        }
                    }
                    StarCommand::Frame(frame) => {
                        let lane_key = lanes.get(index).unwrap().clone();
                        self.process_frame(frame, lane_key ).await;
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

    async fn on_init( &mut self )
    {

        // anybody but central
        if !self.kind.is_central()
        {
            self.send(StarMessageInner::to_central(self.key.clone(), StarMessagePayload::RequestSequence ));
        }

        match self.kind
        {
            StarKind::Central => {}
            StarKind::Mesh => {}
            StarKind::Supervisor => {
                self.send( StarMessageInner::to_central( self.key.clone(), StarMessagePayload::SupervisorPledgeToCentral));
            }
            StarKind::Server => {
            }
            StarKind::Gateway => {}
            StarKind::Link => {}
            StarKind::Client => {}
            StarKind::Ext(_) => {}
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
            self.send_frame(star, frame.clone()).await;
        }
    }

    async fn send(&mut self, message: StarMessageInner )
    {
        self.send_frame(message.to.clone(), Frame::StarMessage(message) );
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
            self.search_for_star(star.clone());
        }
    }

    fn lane_with_shortest_path_to_star( &self, star: &StarKey ) -> Option<&LaneMeta>
    {
        let mut min_hops= usize::MAX;
        let mut rtn = Option::None;

        for (_,lane) in &self.lanes
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

    fn get_hops_to_star( &self, star: &StarKey ) -> Option<usize>
    {
        let mut rtn= Option::None;

        for (_,lane) in &self.lanes
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

    async fn search_for_star( &mut self, star: StarKey )
    {

        let search_id = self.transaction_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed );
        let search_transaction = StarSearchTransaction::new(StarSearchPattern::StarKey(self.key.clone()));
        self.star_search_transactions.insert(search_id, search_transaction );

        let search = StarSearchInner{
            from: self.key.clone(),
            pattern: StarSearchPattern::StarKey(star),
            hops: vec![self.key.clone()],
            transactions: vec![search_id],
            max_hops: MAX_HOPS,
            multi: false
        };

        self.logger.log(StarLog::StarSearchInitialized(search.clone()));
        for (star,lane) in &self.lanes
        {
            lane.lane.outgoing.tx.send( LaneCommand::Frame( Frame::StarSearch(search.clone()))).await;
        }
    }

    async fn process_frame( &mut self, frame: Frame, lane_key: StarKey )
    {
        match frame
        {
            Frame::Proto(proto) => {
              if let ProtoFrame::CentralSearch = proto
              {
                  if self.kind.is_central()
                  {

                      self.broadcast(Frame::Proto(ProtoFrame::CentralFound(1)) , &Option::None).await;
                  } else if let Option::Some(hops) = self.get_hops_to_star(&StarKey::central() )
                  {
                      self.broadcast(Frame::Proto(ProtoFrame::CentralFound(hops+1)) , &Option::None).await;
                  }
                  else
                  {
                      self.search_for_star(StarKey::central()).await;
                  }
              }
            }
            Frame::StarSearch(search) => {
                self.on_star_search(search, lane_key).await;
            }
            Frame::StarSearchResult(result) => {
                self.on_star_search_result(result, lane_key ).await;
            }
            Frame::StarMessage(message) => {
                match self.on_message(message).await
                {
                    Ok(_) => {}
                    Err(error) => {

                    }
                }
            }
            _ => {
                eprintln!("star does not handle frame: {}", frame)
            }
        }
    }

    async fn on_star_search( &mut self, mut search: StarSearchInner, lane_key: StarKey )
    {
        let hit = match &search.pattern
        {
            StarSearchPattern::StarKey(star) => {
                self.key == *star
            }
            StarSearchPattern::StarKind(kind) => {
                self.kind == *kind
            }
        };

        if hit
        {
            if search.pattern.is_single_match()
            {
                let hops = search.hops.len() + 1;
                let frame = Frame::StarSearchResult( StarSearchResultInner {
                    missed: None,
                    hops: search.hops.clone(),
                    hits: vec![ StarSearchHit { star: self.key.clone(), hops: hops as _ } ],
                    search: search.clone(),
                    transactions: search.transactions.clone()
                });

                let lane = self.lanes.get_mut(&lane_key).unwrap();
                lane.lane.outgoing.tx.send(LaneCommand::Frame(frame)).await;
            }
            else {
                // create a SearchTransaction here.
                // gather ALL results into this transaction
            }

            if !search.multi
            {
                return;
            }
        }

        let search_id = self.transaction_seq.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let search_transaction = StarSearchTransaction::new(search.pattern.clone() );
        self.star_search_transactions.insert(search_id,search_transaction);

        search.inc( self.key.clone(), search_id );

        if search.max_hops > MAX_HOPS
        {
            eprintln!("rejecting a search with more than 255 hops");
        }

        if (search.hops.len() as i32) > search.max_hops || self.lanes.len() <= 1
        {
            eprintln!("search has reached maximum hops... need to send not found");
        }

        for (star,lane) in &self.lanes
        {
            if !search.hops.contains(star)
            {
                lane.lane.outgoing.tx.send(LaneCommand::Frame(Frame::StarSearch(search.clone()))).await;
            }
        }
    }

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
                    }
                }
            }
        }
    }

    async fn on_message( &mut self, mut message: StarMessageInner ) -> Result<(),Error>
    {
        if message.to != self.key
        {
            if self.kind.relay()
            {
                self.send(message).await;
                return Ok(());
            }
            else {
                return Err("this star does not relay messages".into())
            }
        }
        else {

            match message.payload
            {
                _ => {
                    if let Ok(Some(messages)) = self.core.handle(message)
                    {
                        for message in messages
                        {
                            self.send( message ).await;
                        }
                    }
                }

            }

        }
        return Ok(());
    }


}

pub trait StarKernel : Send
{
}





pub enum StarCommand
{
    AddLane(Lane),
    AddConnectorController(ConnectorController),
    AddLogger(broadcast::Sender<StarLog>),
    Test(StarTest),
    Frame(Frame),
    FrameTimeout(FrameTimeoutInner),
    FrameError(FrameErrorInner)
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
        };
        write!(f, "{}",r)
    }
}

#[derive(Clone)]
pub struct StarController
{
    pub command_tx: Sender<StarCommand>
}


#[derive(Clone)]
pub struct StarSearchTransaction
{
    pub pattern: StarSearchPattern,
    pub reported_lane_count: i32,
    pub hits: HashMap<StarKey,StarSearchHit>
}

impl StarSearchTransaction
{
    pub fn new(pattern: StarSearchPattern) ->Self
    {
        StarSearchTransaction{
            pattern: pattern,
            reported_lane_count: 0,
            hits: HashMap::new()
        }
    }
}


pub enum TransactionState
{
    Continue,
    Done
}

pub trait Transaction : Send+Sync
{
    fn on_frame( &mut self, frame: Frame, lane: & mut LaneMeta )->TransactionState;
}

pub struct StarKeySearchTransaction
{
}

impl Transaction for StarKeySearchTransaction
{
    fn on_frame(&mut self, frame: Frame, lane: &mut LaneMeta)->TransactionState {

        if let Frame::StarSearchResult(result) = frame
        {
            for hit in result.hits
            {
                lane.star_paths.insert(hit.star.clone(), hit.hops.clone() as _ );
            }
        }

        TransactionState::Done
    }
}


#[derive(Clone)]
pub enum StarLog
{
   StarSearchInitialized(StarSearchInner),
   StarSearchResult(StarSearchResultInner),
   StarSearchComplete(StarSearchTransaction)
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