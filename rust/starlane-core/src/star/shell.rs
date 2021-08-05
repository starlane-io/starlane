use futures::channel::oneshot;
use lru::LruCache;
use tokio::sync::mpsc;
use tokio::time::Duration;

use starlane_resources::ResourceIdentifier;

use crate::frame::{RegistryAction, Reply, SimpleReply, StarMessagePayload};
use crate::message::{Fail, ProtoStarMessage};
use crate::resource::{ResourceAddress, ResourceId, ResourceKey, ResourceRecord, ResourceType};
use crate::star::{LogId, Set, Star, StarCommand, StarKey, StarKind, StarSkel};
use crate::star::Request;
use crate::util::{AsyncProcessor, AsyncRunner, Call};

pub mod lanes;
pub mod locator;
pub mod message;
pub mod pledge;
pub mod router;
pub mod search;
pub mod golden;
