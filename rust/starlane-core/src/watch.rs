use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use starlane_resources::data::{BinSrc, DataSet};
use starlane_resources::ResourceIdentifier;

use crate::resource::ResourceKey;
use crate::star::shell::watch::WatchApi;
use crate::star::StarKey;
use starlane_resources::status::Status;
use std::hash::Hash;

pub type WatchKey = Uuid;

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Watch{
    pub key: WatchKey,
    pub selector: WatchSelector
}

impl Watch {
    pub fn new(selection: WatchSelector) -> Self {
        Self {
            key: WatchKey::new_v4(),
            selector: selection
        }
    }
}

#[derive(Debug,Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct WatchResourceSelector {
    pub resource: ResourceIdentifier,
    pub property: Property
}

impl WatchResourceSelector {
    pub fn new( resource: ResourceIdentifier, property: Property ) -> Self {
        Self {
            resource,
            property
        }
    }
}

#[derive(Debug,Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct WatchSelector {
  pub topic: Topic,
  pub property: Property
}

#[derive(Debug,Clone,Serialize,Deserialize,strum_macros::Display,Hash,Eq,PartialEq)]
pub enum Topic{
    Resource(ResourceKey),
    Star(StarKey),
}

#[derive(Debug,Clone,Serialize,Deserialize,strum_macros::Display,Hash,Eq,PartialEq)]
pub enum Property {
    State,
    Child,
    Status
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct Notification{
    pub selector: WatchSelector,
    pub changes: Vec<Change>
}

impl Notification {
    pub fn new( selector: WatchSelector, change: Change ) -> Self {
        Self {
            selector,
            changes: vec![change]
        }
    }
}

#[derive(Debug,Clone,Serialize,Deserialize,strum_macros::Display)]
pub enum Change {
    State(DataSet<BinSrc>),
    Children(Vec<ChildChange>),
    Status(Status)
}

#[derive(Debug,Clone,Serialize,Deserialize,strum_macros::Display)]
pub enum ChildChange{
    Add(Topic),
    Remove(Topic)
}

#[derive(Clone,Serialize,Deserialize,Hash,Eq,PartialEq)]
pub struct WatchStub{
    pub key: WatchKey,
    pub selection: WatchSelector
}

pub struct Watcher {
    stub: WatchStub,
    watch_api: WatchApi,
    pub rx: mpsc::Receiver<Notification>
}

impl Watcher {
    pub fn new( stub: WatchStub, watch_api: WatchApi, rx: mpsc::Receiver<Notification> ) -> Self {
        Self{
            stub,
            watch_api,
            rx
        }
    }
}


impl Drop for Watcher {
    fn drop(&mut self) {
        self.watch_api.un_listen(self.stub.clone());
        self.rx.close();
    }
}