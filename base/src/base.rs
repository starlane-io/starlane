pub mod config;
pub mod context;

use std::sync::Arc;
use async_trait::async_trait;
use starlane_hyperspace::provider::{Provider, ProviderKind};
use starlane_space::progress::Progress;
use starlane_space::status::{Status, StatusReporter, StatusWatcher};
use crate::{err, status};
use crate::err::BaseErr;
use crate::kind::FoundationKind;


pub struct Base<P,F> where P:Platform, F: Foundation{
  platform: P,
  foundation: F
}




// ['Foundation'] is an abstraction for managing infrastructure.
#[async_trait]
pub trait Foundation: Sync + Send {


    /// [crate::Foundation::Config] should be a `concrete` implementation of [config::FoundationConfig]
    type Config: config::FoundationConfig + ?Sized;

    /// [crate::Foundation::Provider] Should be [Provider] or a custom `trait` that implements [Provider]
    type Provider: Provider+ ?Sized;
    /// a

    fn kind(&self) -> FoundationKind;

    fn config(&self) -> Arc<Self::Config>;

    fn status(&self) -> status::Status;

    async fn status_detail(&self) -> Result<status::StatusDetail, err::BaseErr>;

    fn status_watcher(&self) -> StatusWatcher;

    /// [crate::Foundation::probe] synchronize [crate::Foundation]'s model from that of the external services
    /// and return a [Status].  [crate::Foundation::probe] should also rebuild the [Provider][StatusDetail]
    /// model and update [StatusReporter]
    async fn probe(&self) -> Status;

    /// Take action to bring this [crate::Foundation] to [Status::Ready] if not already. A [crate::Foundation]
    /// is considered ready when all [Provider] dependencies are [Status::Ready].
    async fn ready(&self, progress: Progress) -> Result<(), BaseErr>;

    /// Returns a [Provider] implementation which
    fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<Self::Provider>>, BaseErr>;

}

pub trait Platform:  {

    type Config: config::PlatformConfig +Send+Sync;

    type Provider: Provider+Send+Sync;

}





