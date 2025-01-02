use crate::foundation;
use crate::provider;
use crate::kind;

/// [foundation::skel] provides a starter custom implementation of a [foundation]
/// here you can see the recommended extension technique of re-implementing each of the traits
/// and then providing a `concrete` child mod where the concrete implementations of the
/// same suite of APIs are implemented,
///
/// please copy this file to a new mod and customize as needed

/// # FINAL TRAIT DEFINITIONS
/// this implementation should first define the `Final Trait Definitions` for this Foundation implementation.
///
/// # mod concrete
/// the [concrete] mod defines this [Foundation]'s *actual* [FoundationConfig] & functionality
///
/// this trait is for extending [foundation::Foundation] API and constraining generic traits like [FoundationConfig] so
/// that foundation implementations can better customize their traits for whatever is required.
///
/// # NEW ASSOCIATED TYPES & METHOD's INTRODUCED IN [Foundation]
/// Take careful notice that new `Associated Types` are introduced in [Foundation] trait defs.
/// For example: The [Foundation] [config::ProviderConfig] trait a new associated type:
/// [ProviderConfig::MountsConfig] and a new method: [ProviderConfig::mounts]. Since every
/// [Foundation] must support mounting volumes it is added at this very abstract level
///
/// For illustration purposes a fictitious a [Foundation] implementation:
/// [kind::FoundationKind::Skel] which looks a lot like a Foundation for a unix filesystem...
///

pub trait FoundationConfig: foundation::config::FoundationConfig<ProviderConfig =dyn ProviderConfig<MountsConfig: partial::MountsConfig>> {}


pub trait ProviderConfig: foundation::config::ProviderConfig {
    type MountsConfig: partial::MountsConfig;
    fn mounts(&self) -> &Self::MountsConfig;
}

pub trait Foundation: foundation::Foundation {}
pub trait Provider: provider::Provider<Config=dyn ProviderConfig> {}

/// We need extend some partials to provide new functionality
pub mod partial {
    mod my {
        pub use super::super::*;
    }

    use async_trait::async_trait;
    /// here is the continued implementation of the [mount] partial defined in: [crate::partial::skel]
    use crate::err;
    use crate::foundation;
    use crate::partial;
    use partial::skel as mount;


    /// we create this trait just in case we need to custom traits for this partial with this feature
    pub trait Partial: partial::Partial {}

    pub trait PartialConfig: partial::config::PartialConfig {}

    /// here we add a few odds and ends for the [Mounts] partial that are
    /// required for this particular [foundation::Foundation]
    pub trait MountsConfig: mount::MountsConfig {
        /// Returns the `$user` to own child Volumes
        fn owner(&self) -> String;

        /// Returns the `$permissons` to be set for child Volumes in octal
        fn permissons(&self) -> u16;
    }
    #[async_trait]
    pub trait Mounts: mount::Mounts<Config: MountsConfig, Volume: Volume> {
        /// the concrete implementation of Mounts for this Foundation
        /// will call [Mounts::chown] which will set the permission sof all child volumes
        /// to `$permissions` where the value of `$permission` be the octal value return
        /// of [MountsConfig::permissons]
        /// ```bash
        /// chmod $permissions $volume
        /// ```
        async fn chown(&self) -> Result<(), err::BaseErr>;
        async fn chmod(&self) -> Result<(), err::BaseErr>;
    }
    pub trait VolumeConfig: mount::VolumeConfig {}
    pub trait Volume: mount::Volume<Config: VolumeConfig> {}
}

pub mod concrete {
    use std::sync::Arc;
    use async_trait::async_trait;
    use crate::provider::Provider;
    use crate::foundation;
    use serde::{Deserialize, Serialize};
    use tokio::sync::watch::Receiver;
    use starlane_hyperspace::provider::ProviderKind;
    use starlane_space::progress::Progress;
    use crate::err::BaseErr;
    use crate::kind::FoundationKind;
    use crate::status::Status;
    use crate::status::StatusDetail;

    /// reference the above a `my` mod implementation ... (which is actually [foundation::skel]
    /// the root of this very file!
    mod my {
        pub use super::super::*;
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct FoundationConfig {}

    impl my::FoundationConfig for FoundationConfig {}

    impl foundation::config::FoundationConfig for FoundationConfig {}

    /// Finally! after all the levels of trait inheritance we have reach an *actual* implementation
    /// of [Foundation]
    pub struct Foundation {
        config: Arc<<Self as foundation::Foundation>::Config>,
    }
    impl my::Foundation for Foundation {}

    #[async_trait]
    impl foundation::Foundation for Foundation {
        type Config = FoundationConfig;
        type Provider = my_provider::Provider;

        fn kind(&self) -> FoundationKind {
            todo!()
        }

        fn config(&self) -> Arc<Self::Config> {
            todo!()
        }

        fn status(&self) -> Status {
            todo!()
        }

        async fn status_detail(&self) -> Result<StatusDetail, BaseErr> {
            todo!()
        }

        fn status_watcher(&self) -> Arc<Receiver<Status>> {
            todo!()
        }

        async fn probe(&self, progress: Progress) -> Result<Status, BaseErr> {
            todo!()
        }

        async fn ready(&self, progress: Progress) -> Result<(), BaseErr> {
            todo!()
        }

        fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<Self::Provider>>, BaseErr> {
            todo!()
        }
    }

    /// [super::my_provider] is a concrete
    pub mod my_provider {

        use serde::{Deserialize, Serialize};
        use std::sync::Arc;
        use async_trait::async_trait;
        use starlane_hyperspace::provider::err::ProviderErr;
        use starlane_hyperspace::provider::ProviderKindDef;
        use starlane_space::status::{StatusEntity, StatusWatcher};
        use crate::foundation;
        use crate::config;
        use crate::kind;
        use crate::status::Status;
        use crate::status::StatusDetail;
        mod dependency {
            pub use super::super::*;
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct ProviderConfig {}

        impl foundation::skel::ProviderConfig for ProviderConfig {
            type MountsConfig = ();

            fn mounts(&self) -> &Self::MountsConfig {
                todo!()
            }
        }

        impl foundation::config::ProviderConfig for ProviderConfig {}

        impl config::ProviderConfig for ProviderConfig {
            fn kind(&self) -> &kind::ProviderKind {
                todo!()
            }
        }

        pub struct Provider {}

        impl foundation::skel::Provider for Provider {}

        impl StatusEntity for Provider {
            fn status(&self) -> Status {
                todo!()
            }

            fn status_detail(&self) -> StatusDetail {
                todo!()
            }

            fn status_watcher(&self) -> StatusWatcher {
                todo!()
            }

            fn probe(&self) -> StatusWatcher {
                todo!()
            }
        }

        #[async_trait]
        impl foundation::Provider for Provider<> {
            type Config = ProviderConfig;
            type Item = ();

            fn kind(&self) -> ProviderKindDef {
                todo!()
            }

            fn config(&self) -> Arc<Self::Config> {
                todo!()
            }

            async fn probe(&self) -> Result<(), ProviderErr> {
                todo!()
            }

            async fn ready(&self) -> Result<Self::Item, ProviderErr> {
                todo!()
            }
        }

        impl dependency::Provider for Provider {}
    }

    /*
    /// [super::my_dependency] is just a generic mod name for a [Dependency] variant.
    /// when implementing this pattern probably give it a name that differentiates if from
    /// other dependencies.  For example: if the hypothetical implementation is for [FoundationKind::Kubernetes]
    /// the various concrete dependency implementations should have meaningful names like: `postgres`,
    /// `keycloak`, `s3`, `kafka` ...  and of course instead of one custom dependency variant
    /// multiple implementations can and should be implemented for this Foundation
    pub mod my_dependency {
        use super::my;

        use crate::foundation;
        use serde::{Deserialize, Serialize};
        use std::collections::HashMap;

        pub trait ProviderConfig: my::ProviderConfig {}

        /// notice that we do not constraint the provider's associated type `Config` to
        /// providers in [foundation::skel::concrete::my_dependency]... this is because all
        /// Foundation resources must have uniform trait bounds meaning that traits defined
        /// at the foundation level are defining the `final` interface...
        pub trait Provider: my::Provider {
            fn volumes(&self) -> HashMap<String, String> {
                todo!()
            }
        }



    }

     */

    pub mod partial {
        use super::my::partial as my;
        pub mod mounts {
            use async_trait::async_trait;
            use super::my;
            use crate::err::BaseErr;
            use crate::status::Status;
            use crate::partial::skel as root;
            use crate::partial;
            use serde::{Deserialize, Serialize};
            use tokio::sync::watch::Receiver;

            #[derive(Clone, Debug, Serialize, Deserialize)]
            pub struct MountsConfig {
                pub owner: String,
                pub permissions: u16,
            }

            impl root::MountsConfig for MountsConfig {
                type VolumeConfig = VolumeConfig;

                fn volumes(&self) -> Vec<Self::VolumeConfig> {
                    todo!()
                }
            }

            impl partial::config::PartialConfig for MountsConfig {}

            impl my::PartialConfig for MountsConfig {}

            impl my::MountsConfig for MountsConfig {
                fn owner(&self) -> String {
                    "somebody".to_string()
                }

                fn permissons(&self) -> u16 {
                    0o755
                }
            }

            #[derive(Clone, Debug, Serialize, Deserialize)]
            pub struct VolumeConfig {}

            impl root::VolumeConfig for VolumeConfig {
                fn name(&self) -> String {
                    todo!()
                }

                fn path(&self) -> String {
                    todo!()
                }
            }

            impl partial::config::PartialConfig for VolumeConfig {}

            impl my::VolumeConfig for VolumeConfig {}

            pub struct Mounts {}

            impl partial::Partial for Mounts {
                type Config = MountsConfig;

                fn status_watcher(&self) -> Receiver<Status> {
                    todo!()
                }
            }

            impl my::Partial for Mounts {}
            impl root::Mounts for Mounts {
                type Volume = Volume;

                fn volumes(&self) -> Vec<Self::Volume> {
                    todo!()
                }
            }


            #[async_trait]
            impl my::Mounts for Mounts {
                async fn chown(&self) -> Result<(), BaseErr> {
                    todo!()
                }

                async fn chmod(&self) -> Result<(), BaseErr> {
                    todo!()
                }
            }

            pub struct Volume {}

            impl my::Volume for Volume {}

            impl root::Volume for Volume {}

            impl partial::Partial for Volume {
                type Config = VolumeConfig;

                fn status_watcher(&self) -> Receiver<Status> {
                    todo!()
                }
            }
        }
    }
}
