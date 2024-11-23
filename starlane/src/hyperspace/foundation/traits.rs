use std::collections::HashSet;
use itertools::Itertools;
use serde::Deserialize;
use strum::IntoEnumIterator;
use crate::hyperspace::foundation::settings::{ProtoFoundationSettings, RawSettings};
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::ProtoFoundationBuilder;
use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind, IKind, ProviderKey, ProviderKind};



#[async_trait]
pub trait Foundation: Send + Sync
{
    fn create(builder: ProtoFoundationBuilder) -> Result<impl Foundation+Sized,FoundationErr>;

    fn kind(&self) -> FoundationKind {
        Self::foundation_kind()
    }

    fn foundation_kind() -> FoundationKind;


    /*
    fn dependency(&self, kind: &DependencyKind ) -> Result<impl Dependency,FoundationErr>;

     */

    /*
    /// install any 3rd party dependencies this foundation requires to be minimally operable
    async fn install_foundation_required_dependencies(& mut self) -> Result<(), FoundationErr>;

     */


    fn dependencies(&self) -> HashSet<&'static str> {
        let mut set: HashSet<&'static str> = HashSet::new();
        for kind in  DependencyKind::iter() {
            set.insert(kind.as_str());
        }
        set
    }



}


pub trait Dependency {

    fn kind(&self) -> &DependencyKind;


    async fn install(&self) -> Result<(), FoundationErr> {
        Ok(())
    }

    /*
    async fn provision(&self, config: ProtoProviderConfig ) -> Result<impl Provider,FoundationErr> {
        Err(FoundationErr::provider_not_available( config.kind.clone() ))
    }

     */


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
                    Err(FoundationErr::prov_err(key, format!("provider kind '{}' is not available for dependency: '{}'", ext.to_string(), self.kind().to_string()).to_string()))
                }
            }
        }
    }



}

pub trait Provider {
    async fn initialize(&mut self) -> Result<(), FoundationErr>;
}