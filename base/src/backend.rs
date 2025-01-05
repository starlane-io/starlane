use std::hash::Hash;
use std::result;

pub mod runner;
pub mod handler;
pub mod call;
mod relay;
mod watch;
mod kind;
mod err;

pub trait Backend: Sync+Send {
    type Method: Clone+Eq+PartialEq+Hash+Send+Sync+?Sized;
    type Result: Result+?Sized;
}

pub trait Result:  Send+Sync+Into<result::Result<Self::Ok,Self::Error>>{
    type Ok: Sync+Send+Sync+?Sized;
    type Error: Sync+Send+Sync+?Sized;
}

pub mod provider {
    use thiserror::Error;
    use crate::backend::watch::ActivityWatcher;
    use crate::status::Stage;
    use crate::backend::call as backend;

    pub struct Backend;

    impl backend::Backend for Backend {
        type Method = Method;
        type Result = Result;
    }

    #[derive(Clone,Debug,Eq,PartialEq,Hash)]
    pub enum Method
    where
    {
        Probe,
        Goal(Goal),
    }

    #[derive(Clone,Debug,Eq,PartialEq,Hash)]
    pub enum Goal {
        None,
        Offline,
        Stage(Stage),
        /// [Status::Ready]
        Ready
    }


    pub enum Result {
        Ok,
        Err()
    }


    /// will return
    pub struct RequestActivityWatcher {
        method: Method,
        watcher:  tokio::sync::oneshot::Sender<ActivityWatcher>,
    }

    #[derive(Error)]
    pub enum Error {
        None
    }
}