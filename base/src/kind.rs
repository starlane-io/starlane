use std::hash::Hash;
use strum_macros::EnumDiscriminants;
use starlane_space::parse::CamelCase;

pub use starlane_hyperspace::base::provider::{ProviderKind, ProviderKindDef};


/// used by the [
pub trait BaseKinds {
   type FoundationKind: Clone+Eq+PartialEq+Hash+Send+Sync+?Sized;
   type PlatformKind: Clone+Eq+PartialEq+Hash+Send+Sync+?Sized;
   type ProviderKind: Clone+Eq+PartialEq+Hash+Send+Sync+?Sized;
}




pub struct Exact {
   strata: StrataDef
}


#[derive(Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Strata))]
#[strum_discriminants(derive(Hash,strum_macros::Display))]
pub enum StrataDef {
   Super(SuperSubStrataDef),
   Base(BaseSubStrataDef),
}

#[derive(Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(SuperSubStrata))]
#[strum_discriminants(derive(Hash,strum_macros::Display))]
pub enum SuperSubStrataDef {
   Space,
   Hyper
}

#[derive(Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(BaseSubStrata))]
#[strum_discriminants(derive(Hash, strum_macros::Display))]
pub enum BaseSubStrataDef {
   Platform(PlatformKind),
   Foundation(FoundationKind)
}



#[derive(Clone, Debug, Eq, Hash, PartialEq, strum_macros::Display)]
#[non_exhaustive]
pub enum PlatformKind {
   Starlane
}


impl Default for PlatformKind {
   fn default() -> Self {
      PlatformKind::Starlane
   }
}



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

mod concrete {
   mod root { pub use super::super::*; }

   pub struct BaseKinds;

   impl root::BaseKinds for BaseKinds {
      type FoundationKind = ();
      type PlatformKind = ();
      type ProviderKind = ();
   }

}