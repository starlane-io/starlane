#![allow(warnings)]

//#![feature(hasher_prefixfree_extras)]
use once_cell::sync::Lazy;
use std::str::FromStr;
use async_trait::async_trait;
use base::kind::FoundationKind;
use starlane_hyperspace::provider::{Provider, ProviderKind};
use std::sync::Arc;
use starlane_hyperspace::registry::Registry;
use starlane_space::progress::Progress;
use tokio::sync::watch::Receiver;
use crate::base::err::BaseErr;
use crate::base::foundation::FoundationSafety;
use crate::base::registry;

pub mod base;

#[cfg(test)]
pub mod test;

pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());



pub fn init() {

    {
        use rustls::crypto::aws_lc_rs::default_provider;
        default_provider()
            .install_default()
            .expect("crypto provider could not be installed");
    }
}

/// ['Foundation'] is an abstraction for managing infrastructure.
#[async_trait]
pub trait Foundation: Sync + Send {

    /// [Foundation::Config] should be a `concrete` implementation of [base::config::FoundationConfig]
    type Config: base::config::FoundationConfig + ?Sized;

    /// [Foundation::Provider] Should be [`Provider`] or a custom `trait` that implements [`Provider`] ... it should not be a concrete implementation
    type Provider: Provider + ?Sized;

    fn kind(&self) -> FoundationKind;

    fn config(&self) -> Arc<Self::Config>;

    fn status(&self) -> Status;


    async fn status_detail(&self) -> Result<StatusDetail, BaseErr>;

    fn status_watcher(&self) -> Arc<tokio::sync::watch::Receiver<Status>>;

    /// synchronize must be called first.  In this method the [`Foundation`] will check
    /// update the present [Foundation::status] to be consistent with the actual infrastructure
    async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr>;

    /// Install and initialize any Dependencies and/or [`Providers`] that
    /// are required for this Foundation to run (usually this is not much more than whatever
    /// software is required to run the Registry.)
    async fn install(&self, progress: Progress) -> Result<(), BaseErr>;

    /// Returns a [Provider] implementation which
    fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<Self::Dependency>>, BaseErr>;

    /// return a handle to the [`Registry`]
    fn registry(&self) -> Result<registry::Registry, BaseErr>;
}

#[async_trait]
impl<F> Foundation for FoundationSafety<F>
where
    F: Foundation,
{
    type Config = F::Config;
    type Dependency = F::Dependency;

    type Provider = F::Provider;

    fn kind(&self) -> FoundationKind {
        self.foundation.kind()
    }

    fn config(&self) -> Arc<Self::Config> {
        self.foundation.config()
    }

    fn status(&self) -> Status {
        self.status()
    }

    async fn status_detail(&self) -> Result<StatusDetail, BaseErr> {
        todo!()
    }

    fn status_watcher(&self) -> Arc<Receiver<Status>> {
        self.foundation.status_watcher()
    }

    async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr> {
        self.foundation.synchronize(progress).await
    }

    async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
        if self.status().phase == Phase::Unknown {
            Err(BaseErr::unknown_state("install"))
        } else {
            self.foundation.install(progress).await
        }
    }

    fn provider(&self, kind: &DependencyKind) -> Result<Option<Box<Self::Dependency>>, BaseErr> {
        if self.status().phase == Phase::Unknown {
            Err(BaseErr::unknown_state("dependency"))
        } else {
            self.foundation.provider(kind)
        }
    }

    fn registry(&self) -> Result<registry::Registry, BaseErr> {
        if self.status().phase == Phase::Unknown {
            Err(BaseErr::unknown_state("registry"))
        } else {
            self.foundation.registry()
        }
    }
}