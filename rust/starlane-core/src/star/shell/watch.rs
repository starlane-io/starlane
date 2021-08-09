use crate::error::Error;
use crate::frame::{Frame, Reply, ReplyKind, StarMessage, ProtoFrame};
use crate::message::resource::ProtoMessage;
use crate::message::{Fail, MessageId, ProtoStarMessage, ProtoStarMessageTo};
use crate::star::core::message::CoreMessageCall;
use crate::star::{StarSkel, StarKey};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use tokio::sync::{mpsc, oneshot};
use tokio::time::Duration;
use crate::lane::{UltimaLaneKey, LaneKey, LaneSession};
use crate::star::variant::FrameVerdict;
use crate::watch::{Notification, WatchSelection, WatchKey, Watch, WatchListener, WatchStub};
use std::collections::{HashMap, HashSet};
use mysql::uuid::Uuid;
use std::collections::hash_map::RandomState;
use tokio::sync::mpsc::Sender;

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

    pub fn watch(&self, watch: Watch, lane: UltimaLaneKey ) {
        self.tx.try_send(WatchCall::Watch{watch,lane} ).unwrap_or_default();
    }

    pub fn un_watch(&self, key: WatchKey ) {
        self.tx.try_send(WatchCall::UnWatch(key) ).unwrap_or_default();
    }

    pub fn listen( &self, selection: WatchSelection ) -> WatchListener {

    }

    pub fn un_listen( &self, stub: WatchStub ) {

    }
}

pub enum WatchCall {
    Fire(Notification),
    Watch{watch: Watch, lane: UltimaLaneKey },
    UnWatch(WatchKey),
    Listen{selection: WatchSelection, tx: oneshot::Sender<WatchListener>},
    UnListen(WatchStub),
}

impl Call for WatchCall {}

pub struct WatchComponent {
    skel: StarSkel,
    watch_key_to_lane: HashMap<WatchKey,UltimaLaneKey>,
    selection_to_watch_key: HashMap<WatchSelection,Vec<WatchKey>>,
    listeners: HashMap<WatchSelection,HashMap<WatchKey,mpsc::Sender<Notification>>>
}

impl WatchComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<WatchCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), watch_key_to_lane: Default::default(), selection_to_watch_key: Default::default(), listeners: Default::default() }),
            skel.watch_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<WatchCall> for WatchComponent {
    async fn process(&mut self, call: WatchCall) {
        match call {
            WatchCall::Fire(notification) => {}
            WatchCall::Watch { watch, lane } => {}
            WatchCall::UnWatch(_) => {}
            WatchCall::Listen { selection, tx } => {}
            WatchCall::UnListen(_) => {}
        }
    }
}

impl WatchComponent {

    fn watch( &mut self, watch: Watch, lane: UltimaLaneKey ) {
        self.watch_key_to_lane.insert( watch.key, lane  );
        let watch_keys = self.selection_to_watch_key.remove(&watch.selection );
        let mut watch_keys = match watch_keys {
            None => {
                vec![]
            }
            Some(watch_keys) => watch_keys
        };
        watch_keys.push(watch.key);
        self.selection_to_watch_key.insert(watch.selection,watch_keys );
    }

    fn fire( &self, notification: Notification ) {
        let mut lanes = HashSet::new();
        if let Option::Some(watch_keys) = self.selection_to_watch_key.get(&notification.selection) {
            for watch_key in watch_keys {
                if let Option::Some(lane) = self.watch_key_to_lane.get(watch_key) {
                    lanes.insert(lane.clone() );
                }
            }
        }

        for lane in lanes {
            self.skel.lane_muxer_api.forward_frame(LaneKey::Ultima(lane), Frame::Notification(notification.clone()))
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

    fn listen( &mut self, selection: Selection, result_tx: oneshot::Sender<WatchListener> )  {
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

}

struct CompositeWatcher {
    pub watch: Watch,
    pub children: Vec<WatchKey>
}
