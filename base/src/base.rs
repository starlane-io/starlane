pub mod context;

use starlane_hyperspace::base::provider::Provider;
mod root {
    pub use starlane_hyperspace::base::*;
}


pub struct Base<P,F> where P:Platform, F: Foundation{
  platform: P,
  foundation: F
}


pub type Foundation = dyn root::Foundation;
pub type Platform = dyn root::Platform<Err=(), Foundation=(), ProviderKind=(), RemoteStarConnectionFactory=(), StarAuth=()>;












