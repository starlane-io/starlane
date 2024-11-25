use std::collections::HashSet;
use itertools::Itertools;
use serde::Deserialize;
use strum::IntoEnumIterator;
use crate::hyperspace::foundation::config::Config;
use crate::hyperspace::foundation::settings::{ProtoFoundationSettings, RawSettings};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, ProviderKey, ProviderKind};
use crate::hyperspace::foundation::util::Map;

#[async_trait]
pub trait Foundation: Send + Sync where Self::Config: Config<FoundationKind>
{
    type Config;

    fn create(config: Self::Config) -> Result<impl Foundation<Config=Self::Config>+Sized,FoundationErr>;

    /// a convenience method for getting the FoundationKin before
    /// and actual instance of this trait is created.
    fn kind() -> FoundationKind;
}

#[async_trait]
pub trait Dependency: Send + Sync where Self::Config :Config<DependencyKind>
{
    type Config;

    fn create(config: Self::Config) -> Result<impl Dependency<Config=Self::Config>+Sized,FoundationErr>;

    fn kind() -> DependencyKind;

    async fn install(&self) -> Result<(), FoundationErr> {
        Ok(())
    }

    /// implementers of this Trait should provide a vec of valid provider kinds
    fn provider_kinds(&self) -> HashSet<&'static str> {
        HashSet::new()
    }

    fn has_provisioner(&self, kind: &ProviderKind) -> Result<(),FoundationErr> {
        let providers = self.provider_kinds();
        match kind {
            kind => {
                let ext = kind.to_string();
                if providers.contains(ext.as_str()) {
                    Ok(())
                } else {
                    let key = ProviderKey::new(self.kind().clone(), kind.clone());
                    Err(FoundationErr::prov_err(key, format!("provider kind '{}' is not available for implementation: '{}'", ext.to_string(), self.kind().to_string()).to_string()))
                }
            }
        }
    }
}


pub trait Provider {
    /// a convenience method for getting the ProviderKind before
    /// and actual instance of this trait is created.
    fn provider_kind() -> ProviderKind;

    fn kind(&self) -> ProviderKind{
        Self::provider_kind()
    }

    async fn initialize(&mut self) -> Result<(), FoundationErr>;
}