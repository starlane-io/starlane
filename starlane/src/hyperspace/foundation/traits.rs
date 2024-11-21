use std::collections::HashSet;
use crate::hyperspace::foundation::{DependencyKind, FoundationErr, FoundationKind, ProviderKey, ProviderKind, RegistryProvider};
use crate::hyperspace::foundation::config::{ProtoDependencyConfig, ProtoProviderConfig};

#[async_trait]
pub trait Foundation: Send + Sync
{

    fn kind(&self) -> FoundationKind;

    fn dependency(&self, kind: &DependencyKind ) -> Result<impl Dependency,FoundationErr>;

    /// install any 3rd party dependencies this foundation requires to be minimally operable
    async fn install_foundation_required_dependencies(& mut self) -> Result<(), FoundationErr>;

    /// install a named dependency.  For example the dependency might be "Postgres." The implementing Foundation must
    /// be capable of installing that dependency.  The foundation will make the dependency available after installation
    /// although the method of installing the dependency is under the complete control of the Foundation.  For example:
    /// a LocalDevelopmentFoundation might have an embedded Postgres Database and perhaps another foundation: DockerDesktopFoundation
    /// may actually launch a Postgres Docker image and maybe a KubernetesFoundation may actually install a Postgres Operator ...
    async fn add_dependency(&mut self, config: ProtoDependencyConfig ) -> Result<impl Dependency, FoundationErr>;

    /// return the RegistryFoundation
    fn registry(&self) -> &mut impl RegistryProvider;
}

pub trait Dependency {

    fn kind(&self) -> &DependencyKind;


    async fn install(&self) -> Result<(), FoundationErr> {
        Ok(())
    }

    async fn provision(&self, config: ProtoProviderConfig ) -> Result<impl Provider,FoundationErr> {
        Err(FoundationErr::provider_not_available( config.kind.clone() ))
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
                    Err(FoundationErr::prov_err(key, format!("provider kind '{}' is not available for dependency: '{}'", ext.to_string(), self.kind().to_string()).to_string()))
                }
            }
        }
    }



}

pub trait Provider {
    async fn initialize(&mut self) -> Result<(), FoundationErr>;
}