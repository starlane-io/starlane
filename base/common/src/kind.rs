use starlane_space::parse::CamelCase;

/// reexport from [starlane_hyperspace]
pub use starlane_hyperspace::provider::{ProviderKind,ProviderKindDef};

#[derive(Clone, Debug, Eq, Hash, PartialEq, strum_macros::Display)]
#[non_exhaustive]
pub enum FoundationKind {
   /// A great foundation for local development. The [Provider] implementations create and
   /// manage external services through `Docker`
   DockerDesktop,
   /// This variant is a placeholder. Starlane's day of reckoning will be the day a
   /// `KubernetesFoundation` implementation is released in the wild
   Kubernetes,
   /// [FoundationKind::Skel] variant is just only used in the [crate::foundation::skel]
   /// implementations templates that are meant to be cloned and customised to support new
   /// platforms. The [FoundationKind::Skel] variant is hidden unless the `skel` feature
   /// flag is true... in practice there is no reason to include the skel examples in
   /// any build, however, enabling skel in your IDE is quite helpful because without it
   /// developers can't browse through the code by clicking references
   #[cfg(feature = "skel")]
   Skel,
   _Ext(CamelCase)
}