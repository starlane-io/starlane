use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::RwLock;
use crate::{UniErr};
use crate::id::Point;

pub struct StateCache<C>
where
    C: State,
{
    pub states: Arc<DashMap<Point, Arc<RwLock<C>>>>,
}

impl<C> StateCache<C> where C: State {}

pub trait StateFactory: Send + Sync {
    fn create(&self) -> Box<dyn State>;
}

pub trait State: Send + Sync {
    fn deserialize<DS>(from: Vec<u8>) -> Result<DS, UniErr>
    where
        DS: State,
        Self: Sized;
    fn serialize(self) -> Vec<u8>;
}
