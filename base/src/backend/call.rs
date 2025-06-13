use std::hash::Hash;
use crate::backend::Backend;
use crate::backend::provider::RequestActivityWatcher;


pub struct Call<M> {
    method: M,
    watcher: Option<RequestActivityWatcher>,
}


