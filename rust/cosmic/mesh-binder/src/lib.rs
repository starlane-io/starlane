use std::sync::{Arc, RwLock};
use lru::LruCache;

#[derive(Clone)]
pub struct Binder {
  pub cache: Arc<RwLock<BinderCache>>
}

impl Binder {
    pub fn new() -> Self {
        Self{
            cache: Arc::new(RwLock::new(BinderCache::new() ))
        }
    }
}

struct BinderCache {
    pub bind_config_cache: LruCache<String,CachedItem>
}

impl BinderCache {
    pub fn new() -> Self {
        Self {
            bind_config_cache: LruCache::new()
        }
    }
}