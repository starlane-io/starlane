use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use thiserror::Error;



#[derive(Debug, Error)]
pub enum ProviderErr {
  StateEr,
}

///
#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(StateErr))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum StateErrDetail {
  /// The Panic signals an obstacle that the status entity doesn't know how to resolve.
  /// A Panic state indicates that the Entity has Not reached the desired
  /// [super::status::State::Ready] state and is now idle.
  ///
  /// An entity may recover from a Panic if the panic issue is externally resolved and then
  /// `Entity::synchronize()` is invoked trigger another try-again loop.
  Panic(String),
  /// [StateErr::Fatal] signals an error condition that cannot be recovered from.
  /// Depending upon the context of the status entity reporting [StateErr::Fatal`] possible
  /// actions might be deleting and recreating the entity or shutting down the entire
  /// Starlane process
  Fatal(String)
}

