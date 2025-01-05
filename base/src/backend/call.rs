use std::hash::Hash;
use crate::backend::Backend;
use crate::backend::provider::RequestActivityWatcher;


pub struct Call<B> where B: Backend
{
    method: B::Method,
    rtn: tokio::sync::oneshot::Sender<B::Result>,
    watcher: Option<RequestActivityWatcher>,
}


