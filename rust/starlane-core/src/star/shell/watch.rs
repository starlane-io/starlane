use std::collections::{HashMap, HashSet};
use std::collections::hash_map::RandomState;
use std::future::Future;

use mysql::uuid::Uuid;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::Sender;
use tokio::time::Duration;

use starlane_resources::message::{Fail, MessageId, ProtoMessage};

use crate::error::Error;
use crate::frame::{Frame, ProtoFrame, Reply, ReplyKind, StarMessage, WatchFrame};
use crate::lane::{LaneKey, LaneSession, UltimaLaneKey};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo};
use crate::resource::ResourceRecord;
use crate::star::{StarKey, StarSkel};
use crate::star::core::message::CoreMessageCall;
use crate::star::variant::FrameVerdict;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::watch::{Notification, Topic, Watch, WatchKey, Watcher, WatchSelector, WatchStub};

#[derive(Clone)]
pub struct WatchApi {
    pub tx: mpsc::Sender<WatchCall>,
}

impl WatchApi {
    pub fn new(tx: mpsc::Sender<WatchCall>) -> Self {
        Self { tx }
    }

    pub fn fire(&self, notification: Notification ){
println!("WatchApi: FIRE NOTIFICATION!");
        self.tx.try_send(WatchCall::Fire(notification)).unwrap_or_default();
    }

    pub fn watch(&self, watch: Watch, session: LaneSession) {
println!("WatchApi: Watching: {}", session.lane.to_string() );
        self.tx.try_send(WatchCall::WatchForLane {watch,session} ).unwrap_or_default();
    }

    pub fn un_watch(&self, key: WatchKey ) {
        self.tx.try_send(WatchCall::UnWatchForLane(key) ).unwrap_or_default();
    }

    pub fn notify(&self, notification: Notification ) {
        self.tx.try_send(WatchCall::Notify(notification) ).unwrap_or_default();
    }

    pub async fn listen(&self, selector: WatchSelector) -> Result<Watcher,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.try_send(WatchCall::Watch { selection: selector,tx} ).unwrap_or_default();
        Ok(tokio::time::timeout( Duration::from_secs(15), rx).await??)
    }

    pub fn un_listen( &self, stub: WatchStub ) {
        self.tx.try_send(WatchCall::UnWatch(stub) ).unwrap_or_default();
    }
}

pub enum WatchCall {
    Fire(Notification),
    WatchForLane {watch: Watch, session: LaneSession},
    UnWatchForLane(WatchKey),
    Watch {selection: WatchSelector, tx: oneshot::Sender<Watcher>},
    UnWatch(WatchStub),
    Next{ selector: WatchSelector, next: NextKind },
    Notify(Notification)
}

impl Call for WatchCall {}

pub struct WatchComponent {
    skel: StarSkel,
    key_to_lane: HashMap<WatchKey,WatchLane>,
    selection_to_lane: HashMap<WatchSelector,Vec<WatchLane>>,
    selection_to_next: HashMap<WatchSelector, NextWatch>,
    listeners: HashMap<WatchSelector,HashMap<WatchKey,mpsc::Sender<Notification>>>,
}

impl WatchComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<WatchCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), key_to_lane: Default::default(), selection_to_lane: Default::default(), listeners: Default::default(), selection_to_next: Default::default() }),
            skel.watch_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<WatchCall> for WatchComponent {
    async fn process(&mut self, call: WatchCall) {
        match call {
            WatchCall::Fire(notification) => {
                self.notify(notification);
            }
            WatchCall::WatchForLane { watch, session } => {
                self.watch_for_lane(watch, session);
            }
            WatchCall::UnWatchForLane(key) => {
                self.un_watch_for_lane(key);
            }
            WatchCall::Watch { selection, tx } => {
                self.watch(selection, tx);
            }
            WatchCall::UnWatch(stub) => {
                self.un_watch(stub);
            }
            WatchCall::Next { selector: selection, next } => {
                self.next(selection,next);
            }
            WatchCall::Notify(notification) => {
                self.notify(notification);
            }
        }
    }
}

impl WatchComponent {

    fn watch_for_lane(&mut self, watch: Watch, session: LaneSession) {
println!("WatchApi: WATCHING on star {} for lane: {}",  self.skel.info.key.to_string(), session.lane.to_string() );
        if let LaneKey::Ultima(lane) = session.lane

        {
            let watch = WatchLane {
                key: watch.key,
                lane,
                selector: watch.selector
            };

            self.key_to_lane.insert(watch.key.clone(), watch.clone() );
            let mut watches = if let Option::Some( mut watches) = self.selection_to_lane.remove(&watch.selector)
            {
                watches
            } else {
                vec![]
            };

            watches.push(watch.clone());
println!("WatchApi::watch_for_lane inserting selector: {:?}", watch.selector );
            self.selection_to_lane.insert(watch.selector.clone(), watches );

            let skel = self.skel.clone();
            tokio::spawn(async move {
                Self::watch_next(skel, watch.selector).await;
            });

        } else {
            error!("proto lanes cannot Watch");
        }
    }

    async fn watch_next(skel: StarSkel, selector: WatchSelector) {
println!("WatchApi::watch_next");
            match Self::find_next(&skel,&selector).await {
                Ok(next) => {

println!("WatchApi ... found next: {}", next.to_string() );
                    skel.watch_api.tx.try_send( WatchCall::Next { selector: selector, next }).unwrap_or_default();
                }
                Err(error) => {
                    error!("Watch Error: {}", error.to_string() );
                }
           }
    }

    async fn find_next(skel: &StarSkel, selector: &WatchSelector ) -> Result<NextKind,Error> {
        match &selector.topic {
            Topic::Resource(resource_key) => {
                let record = skel.resource_locator_api.locate(resource_key.clone().into()).await?;
                if skel.info.key == record.location.star {
                    Ok(NextKind::Core)
                } else {
                    let lane = skel.golden_path_api.golden_lane_leading_to_star(record.location.star).await?;
                    Ok(NextKind::Lane(lane))
                }
            }
            Topic::Star(star) => {
                if *star == skel.info.key {
                    Ok(NextKind::Shell)
                } else {
                    let lane = skel.golden_path_api.golden_lane_leading_to_star(star.clone()).await?;
                    Ok(NextKind::Lane(lane))
                }
            }
        }
    }

    fn next(&mut self, selection: WatchSelector, next: NextKind ) {

println!("WatchApi::next( {} )", self.skel.info.key.to_string() );
        if !self.selection_to_next.contains_key(&selection ) {
            let next = NextWatch::new(next, selection.clone() );
            self.selection_to_next.insert(selection.clone(), next.clone() );

            if let NextKind::Lane(lane) = &next.kind {
                let watch = next.clone().into();
                self.skel.lane_muxer_api.forward_frame(LaneKey::Ultima(lane.clone()), Frame::Watch(WatchFrame::Watch(watch)) ).unwrap_or_default();
            }
        }
    }

    fn un_watch_for_lane(&mut self, key: WatchKey)  {
        if let Option::Some(watch) = self.key_to_lane.remove(&key) {
            if let Option::Some( mut watches) = self.selection_to_lane.remove(&watch.selector)
            {
                watches.retain( |w| w.key != watch.key );
                if watches.is_empty() {
                    if let Option::Some(next ) = self.selection_to_next.remove(&watch.selector) {
                        if let NextKind::Lane(lane) = next.kind {
                            self.skel.lane_muxer_api.forward_frame(LaneKey::Ultima(lane.clone()), Frame::Watch(WatchFrame::UnWatch(next.key)) ).unwrap_or_default();
                        }
                    }

                } else {
                    self.selection_to_lane.insert(watch.selector.clone(), watches );
                }
            }
        }
    }


    fn watch(&mut self, selector: WatchSelector, result_tx: oneshot::Sender<Watcher> )  {
println!("WatchApi: adding listener on star {}",  self.skel.info.key.to_string() );


        {
            let skel = self.skel.clone();
            let selector_cp = selector.clone();
            tokio::spawn(async move {
                Self::watch_next(skel, selector_cp).await;
            });
        }

        let stub = WatchStub{
            key: WatchKey::new_v4(),
            selection: selector
        };

        let (tx,rx) = mpsc::channel(256);

        let listener = Watcher::new(stub.clone(), self.skel.watch_api.clone(), rx );

        let mut map = match self.listeners.remove(&stub.selection ) {
            None => HashMap::new(),
            Some(map) => map
        };

        map.insert(stub.key.clone(), tx );
        self.listeners.insert( stub.selection, map );

        result_tx.send(listener).unwrap_or_default();
    }




    fn un_watch(&mut self, stub: WatchStub )  {

        match self.listeners.remove(&stub.selection ) {
            None => {}
            Some(mut map) => {
                map.remove( &stub.key );
                if !map.is_empty() {
                    self.listeners.insert( stub.selection, map );
                }
            }
        };

        self.un_watch_for_lane(stub.key);
    }

    fn notify(&self, notification: Notification ) {
println!("WatchApi: notify({}) selectors: {}", self.skel.info.key.to_string(), self.selection_to_lane.len() );
println!("WatchApi: notify() notification.selector: {:?}", notification.selector );

for (k,v) in &self.selection_to_lane {
    println!("WatchApi: k {:?}", k );
    println!("WatchApi: notify() selector == k {}", notification.selector == *k );
}
        let mut lanes = HashSet::new();
        if let Option::Some(watch_lanes) = self.selection_to_lane.get(&notification.selector) {

println!("WatchApi: found a selector watchlane list : {}", watch_lanes.len() );
            for watch_lane in watch_lanes {
println!("WatchApi adding watch lane: {}", watch_lane.key);
                lanes.insert( watch_lane.lane.clone() );
            }
        }

        for lane in lanes {
println!("WatchApi:: forwarding frame: {}", lane.to_string() );
            self.skel.lane_muxer_api.forward_frame(LaneKey::Ultima(lane), Frame::Watch(WatchFrame::Notify(notification.clone())));
        }

        if let Option::Some(listeners) = self.listeners.get(&notification.selector) {
            for (k,tx) in listeners {

                if !tx.is_closed() {
                    tx.try_send(notification.clone()).unwrap_or_default();
                } else {
                    self.skel.watch_api.un_listen( WatchStub{key:k.clone(),selection: notification.selector.clone() });
                }
            }
        }
    }


}

#[derive(Clone,Hash,Eq,PartialEq)]
pub struct NextWatch {
    pub key: WatchKey,
    pub kind: NextKind,
    pub selection: WatchSelector
}

impl NextWatch {
    pub fn new(kind: NextKind, selection: WatchSelector) -> Self {
        Self{
            key: WatchKey::new_v4(),
            kind,
            selection
        }
    }
}

impl Into<Watch> for NextWatch {
    fn into(self) -> Watch {
        Watch {
            key: self.key,
            selector: self.selection
        }
    }
}

#[derive(Clone,Hash,Eq,PartialEq,strum_macros::Display)]
pub enum NextKind {
    Core,
    Shell,
    Lane(UltimaLaneKey)
}

#[derive(Clone)]
pub struct WatchLane{
    pub key: WatchKey,
    pub lane: UltimaLaneKey,
    pub selector: WatchSelector
}