use async_trait::async_trait;
use crate::status::Status;
use crate::Foundation;

/// [Partial]s are generic definitions that can be inherited by [crate::config], [crate::base] and
/// [crate::foundation] definitions.  Whereas a [crate::base] definition describes the abstract
/// traits of a particular resource a [Partial] defines traits that may apply to multiple
/// [crate::config], [crate::base] or [crate::foundation] definitions.
///
/// Example:
/// the `base` definitions for `Postgres` and `KeyCloak` require a persistent storage directory
/// to be defined.  For the sake of code reuse it makes sense to break out definitions that
/// would apply to two or more `base` or `foundation` definitions
/// a trait definition for the configuration of mounted volumes
///
/// check out the partial starter template: [skel]

use downcast_rs::{impl_downcast, Downcast, DowncastSync};
use tokio::sync::watch;

/// The partial starter template
pub mod config;
pub mod mount;

/// trait for a partial that has a status
#[async_trait]
pub trait Partial: Downcast {
    type Config: config::PartialConfig + ?Sized;
}

impl_downcast!(Partial assoc Config);