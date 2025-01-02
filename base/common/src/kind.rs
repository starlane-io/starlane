use starlane_space::parse::CamelCase;

/// reexport [ProviderKind] from [starlane_hyperspace]
pub use starlane_hyperspace::provider::ProviderKind;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum FoundationKind {
   /// A great foundation for local development. The [Provider] implementations create and
   /// manage external services through `Docker`
   DockerDesktop,
   _Ext(CamelCase)
}