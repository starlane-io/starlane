pub mod config;
pub mod err;
pub mod provider;

use crate::driver::DriversBuilder;
use crate::hyperlane::{HyperAuthenticator, HyperGateSelector, HyperwayEndpointFactory};
use crate::machine::{Machine, MachineApi, MachineTemplate};
use crate::registry::{Registry, RegistryConfig};
use anyhow::anyhow;
use async_trait::async_trait;
use starlane_macros::logger;
use starlane_space::artifact::asynch::Artifacts;
use starlane_space::command::direct::create::KindTemplate;
use starlane_space::err::SpaceErr;
use starlane_space::kind::{
    ArtifactSubKind, BaseKind, FileSubKind, Kind, Specific, StarSub, UserBaseSubKind,
    UserBaseSubKindBase,
};
use starlane_space::loc::{MachineName, StarKey, ToBaseKind};
use starlane_space::log::Logger;
use starlane_space::particle::property::{PropertiesConfig, PropertiesConfigBuilder};
use starlane_space::settings::Timeouts;
use std::str::FromStr;
use std::sync::Arc;
use starlane_space::progress::Progress;
use starlane_space::status;
use starlane_space::status::{Entity, EntityReadier, Status, StatusProbe, StatusReporter, StatusResult, StatusWatcher};
use err::BaseErr;
use crate::base::config::{BaseConfig, BaseSubConfig, FoundationConfig, ProviderConfig};
use crate::base::provider::context::FoundationContext;
use crate::base::provider::{Provider, ProviderKindDisc, ProviderKind};

pub trait BaseSub: Send + Sync {

}

#[async_trait]
pub trait Platform: BaseSub + Sized + Clone
where Self: 'static,
{
    
    type Config;
    
    type Err: std::error::Error + Send + Sync + From<anyhow::Error>+?Sized;
    type StarAuth: HyperAuthenticator+?Sized;
    type RemoteStarConnectionFactory: HyperwayEndpointFactory+Sized;
    
    fn config(&self) -> & Self::Config;

    async fn machine(&self) -> Result<MachineApi, Self::Err> {
        Ok(Machine::new_api(self.clone()).await?)
    }

    /// delete the registry
    async fn scorch(&self) -> Result<(), Self::Err>;

    /// exactly like `scorch` except the `context` is also deleted
    async fn nuke(&self) -> Result<(), Self::Err> {
        /*
        if !self.config().can_nuke() {
            Err(anyhow!("in config '{}' can_nuke=false", config_path()))?;
        }
        self.scorch().await?;
        Ok(())

         */
        todo!("nuke is disabled until the packaging reorg settles down")
    }

    fn star_auth(&self, star: &StarKey) -> Result<Self::StarAuth, Self::Err>;

    fn remote_connection_factory_for_star(
        &self,
        star: &StarKey,
    ) -> Result<Self::RemoteStarConnectionFactory, Self::Err>;

    fn machine_template(&self) -> MachineTemplate;
    fn machine_name(&self) -> MachineName;

    //    fn select_service(&self, kind: &KindSelector, star: &StarKey, point: &Point ) ->

    fn properties_config(&self, kind: &Kind) -> PropertiesConfig {
        let mut builder = PropertiesConfigBuilder::new();
        builder.kind(kind.clone());
        match kind.to_base() {
            BaseKind::Mechtron => {
                builder.add_property("config", true, true).unwrap();
                builder.build().unwrap()
            }
            BaseKind::Host => {
                builder.add_property("bin", true, true).unwrap();
                builder.build().unwrap()
            }
            _ => builder.build().unwrap(),
        }
    }

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder;
    async fn global_registry(&self) -> Result<&Registry, Self::Err>;
    async fn star_registry(&self, star: &StarKey) -> Result<Registry, Self::Err>;
    fn artifact_hub(&self) -> Artifacts;
    async fn start_services(&self, gate: &Arc<HyperGateSelector>) {}
    fn logger(&self) -> Logger {
        logger!()
    }

    fn web_port(&self) -> Result<u16, Self::Err> {
        Ok(8080u16)
    }

    fn data_dir(&self) -> String {
        "./data/".to_string()
    }

    fn select_kind(&self, template: &KindTemplate) -> Result<Kind, SpaceErr> {
        let base: BaseKind = BaseKind::from_str(template.base.to_string().as_str())?;
        Ok(match base {
            BaseKind::Root => Kind::Root,
            BaseKind::Space => Kind::Space,
            BaseKind::Base => Kind::Base,
            BaseKind::User => Kind::User,
            BaseKind::App => Kind::App,
            BaseKind::Mechtron => Kind::Mechtron,
            BaseKind::FileStore => Kind::FileStore,
            BaseKind::File => match &template.sub {
                None => return Err(SpaceErr::KindNotAvailable(template.clone())),
                Some(kind) => {
                    let file_kind = FileSubKind::from_str(kind.as_str())?;
                    return Ok(Kind::File(file_kind));
                }
            },
            BaseKind::Database => {
                unimplemented!("need to write a SpecificPattern matcher...")
            }
            BaseKind::BundleSeries => Kind::BundleSeries,
            BaseKind::Bundle => Kind::Bundle,
            BaseKind::Artifact => match &template.sub {
                None => {
                    return Err(SpaceErr::expect_sub::<ArtifactSubKind>(BaseKind::Artifact));
                }
                Some(sub) => {
                    let artifact_kind = ArtifactSubKind::from_str(sub.as_str())?;
                    return Ok(Kind::Artifact(artifact_kind));
                }
            },
            BaseKind::Control => Kind::Control,
            BaseKind::UserBase => match &template.sub {
                None => {
                    return Err(SpaceErr::expect_sub::<UserBaseSubKindBase>(
                        BaseKind::UserBase,
                    ));
                }
                Some(sub) => {
                    let specific =
                        Specific::from_str("old.io:redhat.com:keycloak:community:18.0.0")?;
                    let sub = UserBaseSubKind::OAuth(specific);
                    Kind::UserBase(sub)
                }
            },
            BaseKind::Repo => Kind::Repo,
            BaseKind::Portal => Kind::Portal,
            BaseKind::Star => {
                return Err(SpaceErr::unimplemented(
                    "stars cannot be created via the template",
                ))
            }
            BaseKind::Driver => Kind::Driver,
            BaseKind::Global => Kind::Global,
            BaseKind::Host => Kind::Host,
            BaseKind::Guest => Kind::Guest,
            BaseKind::Registry => Kind::Registry,
            BaseKind::WebServer => Kind::WebServer,
            BaseKind::Foundation => Kind::Foundation,
            BaseKind::Dependency => Kind::Dependency,
            BaseKind::Provider => Kind::Provider,
        })
    }

    fn log<R>(result: Result<R, Self::Err>) -> Result<R, Self::Err> {
        if let Err(err) = result {
            println!("ERR: {}", err.to_string());
            Err(err)
        } else {
            result
        }
    }

    fn log_ctx<R>(ctx: &str, result: Result<R, Self::Err>) -> Result<R, Self::Err> {
        if let Err(err) = result {
            println!("{}: {}", ctx, err.to_string());
            Err(err)
        } else {
            result
        }
    }

    fn log_deep<R, E: ToString>(
        ctx: &str,
        result: Result<Result<R, Self::Err>, E>,
    ) -> Result<Result<R, Self::Err>, E> {
        match &result {
            Ok(Err(err)) => {
                println!("{}: {}", ctx, err.to_string());
            }
            Err(err) => {
                println!("{}: {}", ctx, err.to_string());
            }
            Ok(_) => {}
        }
        result
    }
}

pub struct Settings {
    pub timeouts: Timeouts,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timeouts: Default::default(),
        }
    }
}

pub trait PlatformConfig: BaseSubConfig
{
    type RegistryConfig: RegistryConfig+?Sized;

    fn can_scorch(&self) -> bool;
    fn can_nuke(&self) -> bool;

    fn registry(&self) -> &Self::RegistryConfig;

    fn home(&self) -> &String;

    fn enviro(&self) -> &String;
}



#[async_trait ]
pub trait Foundation: BaseSub {
    fn status(&self) -> StatusResult {
        self.status_watcher().borrow().clone()
    }
    async fn status_detail(&self) -> status::StatusDetail;

    fn status_watcher(&self) -> &StatusWatcher;

    /// [crate::Foundation::probe] synchronize [crate::Foundation]'s model from that of the external services
    /// and return a [Status].  [crate::Foundation::probe] should also rebuild the [Provider][StatusDetail]
    /// model and update [StatusReporter]
    async fn probe(&self) -> StatusResult;

    /// Take action to bring this [crate::Foundation] to [Status::Ready] if not already. A [crate::Foundation]
    /// is considered ready when all [Provider] dependencies are [Status::Ready].
    async fn ready(&self, progress: Progress) -> StatusResult;

    /// Returns a [Provider] by this [Foundation]
    fn provider<P>(&self, kind: &ProviderKind) -> Result<Option<& P>, BaseErr> where P: Provider+EntityReadier;
}




