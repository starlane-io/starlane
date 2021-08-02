pub mod star;
pub mod resource;

use std::time::Duration;

use lru::LruCache;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use starlane_resources::ResourceIdentifier;

use crate::frame::{RegistryAction, Reply, SimpleReply, StarMessagePayload};
use crate::message::{Fail, ProtoStarMessage};
use crate::resource::{ResourceAddress, ResourceKey, ResourceRecord, ResourceType};
use crate::star::{LogId, Request, ResourceRegistryBacking, Set, Star, StarCommand, StarKey, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};

