use crate::hyperspace::driver::DriversBuilder;
use crate::hyperspace::foundation::Foundation;
use crate::hyperspace::hyperlane::{
    HyperAuthenticator, HyperGateSelector, HyperwayEndpointFactory,
};
use crate::hyperspace::machine::{Machine, MachineApi, MachineTemplate};
use crate::hyperspace::reg::PgRegistryConfig;
use crate::hyperspace::reg::Registry;
use crate::space::artifact::asynch::Artifacts;
use crate::space::command::direct::create::KindTemplate;
use crate::space::err::SpaceErr;
use crate::space::kind::{
    ArtifactSubKind, BaseKind, FileSubKind, Kind, Specific, StarSub, UserBaseSubKind,
    UserBaseSubKindBase,
};
use crate::space::loc::{MachineName, StarKey, ToBaseKind};
use crate::space::log::Logger;
use crate::space::particle::property::{PropertiesConfig, PropertiesConfigBuilder};
use crate::space::settings::Timeouts;
use anyhow::anyhow;
use async_trait::async_trait;
use starlane_primitive_macros::logger;
use std::str::FromStr;
use std::sync::Arc;

#[async_trait]
pub trait Platform: Send + Sync + Sized + Clone
where
    Self::Err: std::error::Error + Send + Sync + From<anyhow::Error>,
    Self: 'static,
    Self::StarAuth: HyperAuthenticator,
    Self::RemoteStarConnectionFactory: HyperwayEndpointFactory,
    Self::Foundation: Foundation + Clone + Send + Sync,
    Self::Config: PlatformConfig,
{
    type Err;
    type StarAuth;
    type RemoteStarConnectionFactory;
    type Foundation;
    type Config;

    fn config(&self) -> &Self::Config;

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
                builder.add_point("config", true, true).unwrap();
                builder.build().unwrap()
            }
            BaseKind::Host => {
                builder.add_point("bin", true, true).unwrap();
                builder.build().unwrap()
            }
            _ => builder.build().unwrap(),
        }
    }

    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder;
    async fn global_registry(&self) -> Result<Registry, Self::Err>;
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

pub trait PlatformConfig: Clone + Send + Sync
where
    Self::RegistryConfig: Clone + Sized + Send + Sync + 'static,
{
    type RegistryConfig;

    fn can_scorch(&self) -> bool;
    fn can_nuke(&self) -> bool;

    fn registry(&self) -> &Self::RegistryConfig;

    fn home(&self) -> &String;

    fn data_dir(&self) -> &String;
}
