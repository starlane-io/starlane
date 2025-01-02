use starlane_hyperspace::provider::Provider;
use crate::base::platform::config;

pub trait Platform {

    type Config: config::PlatformConfig+Send+Sync;

    type Provider: Provider+Send+Sync;

}