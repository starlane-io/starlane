use async_trait::async_trait;
use crate::status::Status;
use crate::Foundation;

/// [Partial]s are generic definitions that can be inherited by [crate::config], [crate::common] and
/// [crate::foundation] definitions.  Whereas a [crate::common] definition describes the abstract
/// traits of a particular resource a [Partial] defines traits that may apply to multiple
/// [crate::config], [crate::common] or [crate::foundation] definitions.
///
/// Example:
/// the `common` definitions for `Postgres` and `Keycloak` require a persistent storage directory
/// to be defined.  For the sake of code reuse it makes sense to break out definitions that
/// would apply to two or more `common` or `foundation` definitions
/// a trait definition for the configuration of mounted volumes
///
/// check out the partial starter template: [skel]

use downcast_rs::{impl_downcast, Downcast, DowncastSync};
use tokio::sync::watch;

/// The partial starter template
#[cfg(feature="skel")]
pub mod skel;
pub mod config;


/// trait for a partial that has a status
#[async_trait]
pub trait Partial: Downcast {
    type Config: config::PartialConfig + ?Sized;
    fn status(&self) -> Status {
        self.status_watcher().borrow().clone()
    }
    fn status_watcher(&self) -> watch::Receiver<Status>;
}

impl_downcast!(Partial assoc Config);