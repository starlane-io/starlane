use crate::base;
use base::foundation;

/// [`foundation::skel`] provides a starter custom implementation of a [foundation`]
/// here you can see the recommended extension technique of re-implementing each of the traits
/// and then providing a [`concrete`] child mod where the concrete implementations of the
/// same suite of APIs are implemented,
///
/// please copy this file to a new mod and customize as needed

/// # FINAL TRAIT DEFINITIONS
/// this implementation should first define the `Final Trait Definitions` for this Foundation implementation.
///
/// # [mod concrete]
/// `mod concrete` defines this foundation's *actual* config & functionality
///
/// this trait is for extending [`foundation::Foundation`] API and constraining generic traits like [`FoundationConfig`] so
/// that foundation implementations can better customize their traits for whatever is required.
///
/// #ASSOCIATED TYPES
/// Take careful notice the slight pattern shift when definition `Associated Types` at the `Foundation Kind `
///
///
///

pub trait FoundationConfig: foundation::config::FoundationConfig<DependencyConfig=dyn DependencyConfig<MountsConfig=concrete::partial::mounts::MountsConfig, ProviderConfig=dyn ProviderConfig>> {}

pub trait DependencyConfig: foundation::config::DependencyConfig<ProviderConfig=dyn ProviderConfig> {
    type MountsConfig: partial::MountsConfig;

    fn mounts(&self) -> &Self::MountsConfig;
}

pub trait ProviderConfig: foundation::config::ProviderConfig {}

pub trait Foundation: foundation::Foundation {}
pub trait Dependency: foundation::Dependency<Config=dyn DependencyConfig<MountsConfig=concrete::partial::mounts::MountsConfig>, Provider=dyn Provider> {}
pub trait Provider: foundation::Provider<Config=dyn ProviderConfig> {}

pub mod partial {
    mod my {
        pub use super::super::*;
    }
    /// here is the continued implementation of the `mount` partial defined here: [partial::skel]
    use crate::base;
    use base::err;
    use base::foundation;
    use base::partial;
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
    use crate::base;
    use crate::base::err::BaseErr;
    use crate::base::foundation::kind::FoundationKind;
    use crate::base::foundation::skel::concrete::partial::mounts;
    use crate::base::foundation::status::{Status, StatusDetail};
    use crate::base::foundation::Provider;
    use crate::base::kind::{DependencyKind, Kind};
    use crate::base::registry::Registry;
    use crate::space::progress::Progress;
    use base::foundation;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use tokio::sync::watch::Receiver;

    ///  reference the above a [`my`] implementation ...
    mod my {
        pub use super::super::*;
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct FoundationConfig {}

    impl my::FoundationConfig for FoundationConfig {}

    impl foundation::config::FoundationConfig for FoundationConfig {}

    impl base::config::FoundationConfig for FoundationConfig {
        type DependencyConfig = dyn my::DependencyConfig<ProviderConfig=dyn my::ProviderConfig, MountsConfig=mounts::MountsConfig>;

        fn kind(&self) -> FoundationKind {
            todo!()
        }

        fn required(&self) -> Vec<Kind> {
            todo!()
        }

        fn dependency_kinds(&self) -> &Vec<DependencyKind> {
            todo!()
        }

        fn dependency(&self, kind: &DependencyKind) -> Option<&Self::DependencyConfig> {
            todo!()
        }
    }

    /// Finally! after all the levels of trait inheritance we have reach an *actual* implementation
    /// of [Foundation]

    pub struct Foundation {
        config: Arc<<Self as foundation::Foundation>::Config>,
    }
    impl my::Foundation for Foundation {}

    #[async_trait]
    impl foundation::Foundation for Foundation {
        type Config = FoundationConfig;
        type Dependency = dyn my::Dependency;
        type Provider = dyn my::Provider;

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

        async fn synchronize(&self, progress: Progress) -> Result<Status, BaseErr> {
            todo!()
        }

        async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
            todo!()
        }

        fn dependency(&self, kind: &DependencyKind) -> Result<Option<Box<Self::Dependency>>, BaseErr> {
            todo!()
        }

        fn registry(&self) -> Result<Registry, BaseErr> {
            todo!()
        }
    }

    /// [super::my_dependency] is just a generic mod name for a [`Dependency`] variant.
    /// when implementing this pattern probably give it a name that differentiates if from
    /// other dependencies.  For example: if the hypothetical implementation is for [`FoundationKind::Kubernetes`]
    /// the various concrete dependency implementations should have meaningful names like: `postgres`,
    /// `keycloak`, `s3`, `kafka` ...  and of course instead of one custom dependency variant
    /// multiple implementations can and should be implemented for this Foundation
    pub mod my_dependency {
        use super::my;
        use crate::base::err::BaseErr;
        use crate::base::foundation::skel::concrete::partial::mounts;
        use crate::base::foundation::status::Status;
        use crate::base::foundation::LiveService;
        use crate::base::kind::{DependencyKind, Kind, ProviderKind};
        use crate::base::{config, foundation};
        use crate::space::progress::Progress;
        use serde::{Deserialize, Serialize};
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::sync::watch::Receiver;

        #[derive(Clone, Serialize, Deserialize)]
        pub struct DependencyConfig {}

        impl config::DependencyConfig for DependencyConfig {
            type ProviderConfig = dyn my::ProviderConfig;

            fn kind(&self) -> &DependencyKind {
                todo!()
            }

            fn require(&self) -> Vec<Kind> {
                todo!()
            }
        }


        impl foundation::config::DependencyConfig for DependencyConfig {
            fn volumes(&self) -> HashMap<String, String> {
                todo!()
            }
        }

        impl my::DependencyConfig for DependencyConfig {
            type MountsConfig = mounts::MountsConfig;

            fn mounts(&self) -> &Self::MountsConfig {
                todo!()
            }
        }

        pub struct Dependency {}
        #[async_trait]
        impl foundation::Dependency for Dependency {
            type Config = dyn my::DependencyConfig<MountsConfig=mounts::MountsConfig>;
            type Provider = dyn my::Provider;

            fn kind(&self) -> DependencyKind {
                todo!()
            }

            fn config(&self) -> Arc<Self::Config> {
                todo!()
            }

            fn status(&self) -> Status {
                todo!()
            }

            fn status_watcher(&self) -> Arc<Receiver<Status>> {
                todo!()
            }

            async fn download(&self, progress: Progress) -> Result<(), BaseErr> {
                todo!()
            }

            async fn install(&self, progress: Progress) -> Result<(), BaseErr> {
                todo!()
            }

            async fn initialize(&self, progress: Progress) -> Result<(), BaseErr> {
                todo!()
            }

            async fn start(&self, progress: Progress) -> Result<LiveService<DependencyKind>, BaseErr> {
                todo!()
            }

            fn provider(&self, kind: &ProviderKind) -> Result<Option<Box<Self::Provider>>, BaseErr> {
                todo!()
            }
        }
        impl my::Dependency for Dependency {}

        pub trait ProviderConfig: my::ProviderConfig {}

        /// notice that we do not constraint the provider's associated type `Config` to
        /// providers in [foundation::skel::concrete::my_dependency]... this is because all
        /// Foundation resources must have uniform trait bounds meaning that traits defined
        /// at the foundation level are defining the `final` interface...
        pub trait Provider: my::Provider {}


        /// [super::my_provider] follows the same pattern as [`super::my_provider`] except in this case it is for
        /// [crate::base::foundation::Provider] variants
        pub mod my_provider {
            use super::my;
            use crate::base::err::BaseErr;
            use crate::base::foundation;
            use crate::base::foundation::status::Status;
            use crate::base::foundation::LiveService;
            use crate::base::kind::ProviderKind;
            use crate::space::parse::CamelCase;
            use crate::space::progress::Progress;
            use serde::{Deserialize, Serialize};
            use std::sync::Arc;
            use tokio::sync::watch::Receiver;
            mod dependency {
                pub use super::super::*;
            }

            #[derive(Clone, Serialize, Deserialize)]
            pub struct ProviderConfig {}

            impl crate::base::foundation::skel::ProviderConfig for ProviderConfig {}

            impl crate::base::foundation::config::ProviderConfig for ProviderConfig {}

            impl crate::base::config::ProviderConfig for ProviderConfig {
                fn kind(&self) -> &ProviderKind {
                    todo!()
                }
            }

            impl dependency::ProviderConfig for ProviderConfig {}


            pub struct Provider {}

            impl foundation::skel::Provider for Provider {}

            #[async_trait]
            impl foundation::Provider for Provider {
                type Config = dyn my::ProviderConfig;

                fn kind(&self) -> &ProviderKind {
                    todo!()
                }

                fn config(&self) -> Arc<Self::Config> {
                    todo!()
                }

                fn status(&self) -> Status {
                    todo!()
                }

                fn status_watcher(&self) -> Arc<Receiver<Status>> {
                    todo!()
                }

                async fn initialize(&self, progress: Progress) -> Result<(), BaseErr> {
                    todo!()
                }

                async fn start(&self, progress: Progress) -> Result<LiveService<CamelCase>, BaseErr> {
                    todo!()
                }
            }

            impl dependency::Provider for Provider {}
        }
    }

    pub mod partial {
        use super::my::partial as my;
        pub mod mounts {
            use super::my;
            use crate::base::err::BaseErr;
            use crate::base::foundation::status::Status;
            use crate::base::partial::skel as root;
            use crate::base::partial;
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
