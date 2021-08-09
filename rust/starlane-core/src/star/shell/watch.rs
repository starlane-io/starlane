use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage, ProtoFrame, WatchFrame};
use crate::message::resource::ProtoMessage;
use crate::message::{Fail, MessageId, ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::{StarSkel, StarKey};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use crate::lane::{UltimaLaneKey, LaneKey, LaneSession};
use crate::star::variant::FrameVerdict;
use crate::watch::{Notification, WatchSelection, WatchKey, Watch, WatchListener, WatchStub, Topic};
use std::collections::{HashMap, HashSet};
use mysql::uuid::Uuid;
use std::collections::hash_map::RandomState;
use tokio::sync::mpsc::Sender;
use crate::resource::ResourceRecord;
use std::future::Future;

#[derive(Clone)]
pub struct WatchApi {
    pub tx: mpsc::Sender<WatchCall>,
}

impl WatchApi {
    pub fn new(tx: mpsc::Sender<WatchCall>) -> Self {
        Self { tx }
    }

    pub fn fire(&self, notification: Notification ){
        self.tx.try_send(WatchCall::Fire(notification)).unwrap_or_default();
    }

    pub fn watch(&self, watch: Watch, session: LaneSession) {
        self.tx.try_send(WatchCall::Watch{watch,session} ).unwrap_or_default();
    }

    pub fn un_watch(&self, key: WatchKey ) {
        self.tx.try_send(WatchCall::UnWatch(key) ).unwrap_or_default();
    }

    pub fn notify(&self, notification: Notification ) {
        self.tx.try_send(WatchCall::Notify(notification) ).unwrap_or_default();
    }

    pub async fn listen( &self, selection: WatchSelection ) -> Result<WatchListener,Error> {
        let (tx,rx) = oneshot::channel();
        self.tx.try_send(WatchCall::Listen{selection,tx} ).unwrap_or_default();
        Ok(tokio::time::timeout( Duration::from_secs(15), rx).await??)
    }

    pub fn un_listen( &self, stub: WatchStub ) {
        self.tx.try_send(WatchCall::UnListen(stub) ).unwrap_or_default();
    }
}

pub enum WatchCall {
    Fire(Notification),
    Watch{watch: Watch, session: LaneSession},
    UnWatch(WatchKey),
    Listen{selection: WatchSelection, tx: oneshot::Sender<WatchListener>},
    UnListen(WatchStub),
    Next{selection:WatchSelection, next: NextKind },
    Notify(Notification)
}

impl Call for WatchCall {}

pub struct WatchComponent {
    skel: StarSkel,
    key_to_lane: HashMap<WatchKey,WatchLane>,
    selection_to_lane: HashMap<WatchSelection,Vec<WatchLane>>,

    selection_to_next: HashMap<WatchSelection, NextWatch>,

    listeners: HashMap<WatchSelection,HashMap<WatchKey,mpsc::Sender<Notification>>>,
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
            WatchCall::Watch { watch, session } => {
                self.watch(watch,session);
            }
            WatchCall::UnWatch(key) => {
                self.un_watch(key);
            }
            WatchCall::Listen { selection, tx } => {
                self.listen(selection,tx);
            }
            WatchCall::UnListen(stub) => {
                self.un_listen(stub);
            }
            WatchCall::Next { selection, next } => {
                self.next(selection,next);
            }
            WatchCall::Notify(notification) => {
                self.notify(notification);
            }
        }
    }
}

impl WatchComponent {

    fn watch(&mut self, watch: Watch, session: LaneSession) {
        if let LaneKey::Ultima(lane) = session.lane

        {
            let watch = WatchLane {
                key: watch.key,
                lane,
                selection: watch.selection
            };

            self.key_to_lane.insert(watch.key.clone(), watch.clone() );
            let mut watches = if let Option::Some( mut watches) = self.selection_to_lane.remove(&watch.selection )
            {
                watches
            } else {
                vec![]
            };

            watches.push(watch.clone());
            self.selection_to_lane.insert( watch.selection.clone(), watches );

            let skel = self.skel.clone();

            tokio::spawn(async move {

                async fn find_next(skel: &StarSkel, watch: &WatchLane ) -> Result<NextKind,Error> {
                    match &watch.selection.topic {
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

                match find_next(&skel,&watch).await {
                    Ok(next) => {
                        skel.watch_api.tx.try_send( WatchCall::Next { selection: watch.selection, next }).unwrap_or_default();
                    }
                    Err(error) => {
                        error!("Watch Error: {}", error.to_string() );
                    }
                }


            });



        } else {
            error!("proto lanes cannot Watch");
        }
    }



    fn next( &mut self, selection: WatchSelection, next: NextKind ) {
        if !self.selection_to_next.contains_key(&selection ) {
            let next = NextWatch::new(next, selection.clone() );
            self.selection_to_next.insert(selection.clone(), next.clone() );

            if let NextKind::Lane(lane) = &next.kind {
                let watch = next.clone().into();
                self.skel.lane_muxer_api.forward_frame(LaneKey::Ultima(lane.clone()), Frame::Watch(WatchFrame::Watch(watch)) ).unwrap_or_default();
            }
        }
    }

    fn un_watch( &mut self, key: WatchKey)  {
        if let Option::Some(watch) = self.key_to_lane.remove(&key) {
            if let Option::Some( mut watches) = self.selection_to_lane.remove(&watch.selection )
            {
                watches.retain( |w| w.key != watch.key );
                if watches.is_empty() {
                    if let Option::Some(next ) = self.selection_to_next.remove(&watch.selection ) {
                        if let NextKind::Lane(lane) = next.kind {
                            self.skel.lane_muxer_api.forward_frame(LaneKey::Ultima(lane.clone()), Frame::Watch(WatchFrame::UnWatch(next.key)) ).unwrap_or_default();
                        }
                    }

                } else {
                    self.selection_to_lane.insert( watch.selection.clone(), watches );
                }
            }
        }
    }


    fn listen( &mut self, selection: WatchSelection, result_tx: oneshot::Sender<WatchListener> )  {
        let stub = WatchStub{
            key: WatchKey::new_v4(),
            selection
        };

        let (tx,rx) = mpsc::channel(256);

        let listener = WatchListener::new(stub.clone(),self.skel.watch_api.clone(), rx );

        let mut map = match self.listeners.remove(&stub.selection ) {
            None => HashMap::new(),
            Some(map) => map
        };

        map.insert(stub.key.clone(), tx );
        self.listeners.insert( stub.selection, map );

        result_tx.send(listener).unwrap_or_default();
    }




    fn un_listen( &mut self, stub: WatchStub )  {

        match self.listeners.remove(&stub.selection ) {
            None => {}
            Some(mut map) => {
                map.remove( &stub.key );
                if !map.is_empty() {
                    self.listeners.insert( stub.selection, map );
                }
            }
        };

        self.un_watch(stub.key);
    }

    fn notify(&self, notification: Notification ) {
        let mut lanes = HashSet::new();
        if let Option::Some(watch_lanes) = self.selection_to_lane.get(&notification.selection) {
            for watch_lane in watch_lanes {
                lanes.insert( watch_lane.lane.clone() );
            }
        }

        for lane in lanes {
            self.skel.lane_muxer_api.forward_frame(LaneKey::Ultima(lane), Frame::Watch(WatchFrame::Notify(notification.clone())));
        }

        if let Option::Some(listeners) = self.listeners.get(&notification.selection ) {
            for (k,tx) in listeners {
                if !tx.is_closed() {
                    tx.try_send(notification.clone()).unwrap_or_default();
                } else {
                    self.skel.watch_api.un_listen( WatchStub{key:k.clone(),selection: notification.selection.clone() });
                }
            }
        }
    }


}

#[derive(Clone,Hash,Eq,PartialEq)]
pub struct NextWatch {
    pub key: WatchKey,
    pub kind: NextKind,
    pub selection: WatchSelection
}

impl NextWatch {
    pub fn new(kind: NextKind, selection: WatchSelection) -> Self {
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
            selection: self.selection
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
    pub selection: WatchSelection
}