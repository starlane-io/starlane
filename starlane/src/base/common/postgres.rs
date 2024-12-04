use crate::base::foundation;
use crate::base::err::BaseErr;
use crate::base::foundation::util::{IntoSer, Map, SerMap};
use crate::base::foundation::Dependency;
use crate::space::parse::CamelCase;
use derive_name::Name;
use futures::TryFutureExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use crate::base;
use crate::base::kind::{DependencyKind, Kind, PostgresKind, ProviderKind};




pub mod concrete {
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct PostgresClusterCoreConfig {
        pub port: u16,
        pub data_dir: String,
        pub username: String,
        pub password: String,
        pub providers: HashMap<CamelCase, Arc<ProviderConfig>>,
    }

    impl PostgresClusterCoreConfig {
        pub fn create(config: Map) -> Result<Self, FoundationErr> {
            let port: u16 = config
                .from_field_opt("port")
                .map_err(FoundationErr::config_err)?
                .map_or(5432u16, |port| port);
            let username: String = config
                .from_field_opt("username")
                .map_err(FoundationErr::config_err)?
                .map_or("postgres".to_string(), |username| username);
            let password: String = config
                .from_field_opt("password")
                .map_err(FoundationErr::config_err)?
                .map_or("postgres".to_string(), |password| password);
            let data_dir: String = config.from_field("data_dir")?;

            let mut providers = config.parse_same("providers")?;
            let mut providers: HashMap<CamelCase, Arc<ProviderConfig>> = providers.into_iter().map(|(key, value)| (key, Arc::new(value))).collect();
            let registry_kind = CamelCase::from_str("Registry")?;
            if !providers.contains_key(&registry_kind) {
                providers.insert(registry_kind, Arc::new(ProviderConfig::default_registry()));
            }

            Ok(PostgresClusterCoreConfig {
                port,
                data_dir,
                username,
                password,
                providers,
            })
        }

        pub fn create_as_trait(config: Map) -> Result<Arc<dyn DependencyConfig>, FoundationErr> {
            Ok(Self::create(config)?.into_trait())
        }

        pub fn into_trait(self) -> Arc<dyn DependencyConfig> {
            todo!();
            /*        let config = Arc::new(self);
                    config as Arc<dyn DependencyConfig>

             */
        }
    }

    pub trait DependencyConfig: base::config::DependencyConfig {}

    impl base::config::DependencyConfig for PostgresClusterCoreConfig {
        fn kind(&self) -> &DependencyKind {
            &DependencyKind::PostgresCluster
        }

        fn volumes(&self) -> HashMap<String, String> {
            let mut map = HashMap::new();
            map.insert("data".to_string(), self.data_dir.clone());
            map
        }

        fn require(&self) -> Vec<Kind> {
            foundation::default_requirements()
        }

        fn clone_me(&self) -> Arc<dyn base::config::DependencyConfig> {
            Arc::new(self.clone())
        }

        type ProviderConfig = ();
    }

    impl IntoSer for PostgresClusterCoreConfig {
        fn into_ser(&self) -> Box<dyn SerMap> {
            todo!()
            //        self.clone() as Box<dyn SerMap>
        }
    }

    impl base::config::ProviderConfigSrc for PostgresClusterCoreConfig {
        type Config = Arc<ProviderConfig>;
        fn providers(&self) -> Result<HashMap<CamelCase, Self::Config>, FoundationErr> {
            Ok(self.providers.clone())
        }

        fn provider(&self, kind: &CamelCase) -> Result<Option<&Self::Config>, FoundationErr> {
            Ok(self.providers.get(kind))
        }
    }

    #[derive(
        Name,
        Clone,
        Debug,
        Eq,
        PartialEq,
        Hash,
        strum_macros::Display,
        strum_macros::EnumString,
        strum_macros::IntoStaticStr,
        Serialize,
        Deserialize,
    )]
    pub enum PostgresSeed {
        Registry,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct ProviderConfig {
        pub kind: PostgresKind,
        pub database: Option<String>,
        pub seed: Option<PostgresSeed>,
    }

    impl ProviderConfig {
        pub fn default_registry() -> Self {
            Self {
                kind: PostgresKind::Registry,
                database: Some("/var/lib/postgresql/data".to_string()),
                seed: Some(PostgresSeed::Registry),
            }
        }
    }

    impl IntoSer for ProviderConfig {
        fn into_ser(&self) -> Box<dyn SerMap> {
            Box::new(self.clone()) as Box<dyn SerMap>
        }
    }

    impl base::config::ProviderConfig for ProviderConfig {
        fn kind(&self) -> &ProviderKind {
            todo!()
        }

        fn clone_me(&self) -> Arc<dyn base::config::ProviderConfig> {
            Arc::new(self.clone())
        }
    }

}