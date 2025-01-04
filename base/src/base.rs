pub mod context;
mod err;

use starlane_hyperspace::base::provider::Provider;
mod root {
    pub use starlane_hyperspace::base::*;

}


pub struct Base<P,F> where P:Platform, F: Foundation{
  platform: P,
  foundation: F
}


pub trait Foundation: root::Foundation {}
pub trait Platform: root::Platform {}












