use crate::config;

pub trait PlatformConfig: config::BaseConfig {}

pub trait ProviderConfig: config::ProviderConfig {}
