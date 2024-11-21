use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::process;
use serde_yaml::Value;
use crate::env::STARLANE_CONFIG;
use crate::hyperspace::foundation::config::ProtoFoundationConfig;
use crate::hyperspace::foundation::{DependencyKind, FoundationErr, FoundationKind, ProviderKind};
use crate::hyperspace::foundation::runner::Runner;
use crate::hyperspace::foundation::traits::{Dependency, Foundation, Provider};
use crate::hyperspace::reg::Registry;

/// # FOUNDATION
/// A ['Foundation'] provides abstracted control over the services and dependencies that drive Starlane.
/// Presently there is only the ['DockerDesktopFoundation'] which uses a local Docker Service
/// to pull dependent Docker Images, run docker instances and in general enables the Starlane [`Platform`]
/// manage the lifecycle of arbitrary services.
///
/// There are two sub concepts that ['Foundation'] provides: ['Dependency'] and  ['Provider'].
/// The [`FoundationConfig`] enumerates dependencies which are typically things that don't ship
/// with the Starlane binary.  Common examples are: Postgres, Keycloak, Docker.  Each foundation
/// implementation must know how to ready that Dependency and potentially even launch an
/// instance of that Dependency.  For Example: Postgres Database is a common dependency especially
/// because the default Starlane [`Registry`] (and at the time of this writing the only Registry support).
/// The Postgres [`Dependency`] ensures that Postgres is accessible and properly configured for the
/// Starlane Platform.
///
/// ## ADDING DEPENDENCIES
/// Additional Dependencies can be added via [`Foundation::add_dependency`]  The Foundation
/// implementation must understand how to get the given [`DependencyKind`] and it's entirely possible
/// that the supported Dependencies differ from Foundation to Foundation.
///
/// ## PROVIDER
/// A [`Dependency`] has a one to many child concept called a [`Provider`] (poorly named!)  Not all Dependencies
/// have a Provider.  A Provider is something of an instance of a given Dependency.... For example:
/// The Postgres Cluster [`DependencyKind::Postgres`]  (talking the actual postgresql software which can serve multiple databases)
/// The Postgres Dependency may have multiple Databases ([`ProviderKind::Database`]).  The provider
/// utilizes a common Dependency to provide a specific service etc.
///
/// ## THE REGISTRY
/// There is one special dependency that the Foundation must manage which is the [`Foundation::registry`]
/// the Starlane Registry is the only required dependency from the vanilla Starlane installation
///
type CreateFoundation =  dyn FnMut(Value) -> Result<dyn Foundation,FoundationErr> + Sync + Send+ 'static;
type CreateDep =  dyn FnMut(Value) -> Result<dyn Dependency,FoundationErr> + Sync + Send+ 'static;
type CreateProvider =  dyn FnMut(Value) -> Result<dyn Provider,FoundationErr> + Sync + Send+ 'static;

pub static FOUNDATIONS: Lazy<HashMap<FoundationKind, CreateFoundation>> =
    Lazy::new(|| {
        let mut foundations = HashMap::new();
//        foundations.insert(FoundationKind::DockerDesktop, DockerDesktopFoundation::create );
        foundations
    });
static FOUNDATION: Lazy<dyn Foundation> =
    Lazy::new(|| {
        let foundation_config = STARLANE_CONFIG.foundation.clone();
        let foundation = match create_foundation(foundation_config) {
            Ok(foundation) => foundation,
            Err(err) => {
                let msg = format!("[PANIC] Starlane instance cannot create Foundation.  Caused by: '{}'", err.is_fatal()).to_string();
                let logger = logger!(Point::global_foundation());
                logger.error(msg);
                process::exit(1);
            }
        };
       foundation
    });

fn create_foundation(config: ProtoFoundationConfig) -> Result<impl Foundation,FoundationErr> {

    Ok(foundation)
}

/// should be called and retained by [`Platform`]
pub(crate) fn foundation(config: ProtoFoundationConfig) -> Result<impl Foundation,FoundationErr>  {
    let foundation = FOUNDATIONS.get(&config.kind).ok_or(FoundationErr::foundation_not_available(config.kind))?(config);
    let foundation = Runner::new(foundation);

    foundation
}