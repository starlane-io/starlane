pub mod context;

use starlane_hyperspace::base::Foundation;
use starlane_hyperspace::base::provider::Provider;
mod root {
    pub use starlane_hyperspace::base::*;
}


pub struct Base<P> where P:root::Platform<Err=(), RemoteStarConnectionFactory=(), StarAuth=()>{
  platform: P,
  foundation: Foundation
}














