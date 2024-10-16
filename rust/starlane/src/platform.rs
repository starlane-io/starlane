use starlane::space::artifact::asynch::Artifacts;
use starlane::space::kind::{ArtifactSubKind, BaseKind, FileSubKind, Kind, NativeSub, Specific, StarSub, UserBaseSubKind, UserBaseSubKindBase};
use starlane::space::loc::{MachineName, StarKey, ToBaseKind};
use starlane::space::particle::property::{PropertiesConfig, PropertiesConfigBuilder};
use std::sync::Arc;
use starlane::space::command::direct::create::KindTemplate;
use starlane::space::err::SpaceErr;
use starlane::space::log::RootLogger;
use std::str::FromStr;
use starlane::space::settings::Timeouts;
use crate::driver::DriversBuilder;
use crate::hyperlane::{HyperAuthenticator, HyperGateSelector, HyperwayEndpointFactory};
use crate::hyperspace::machine::{Machine, MachineApi, MachineTemplate};
use crate::hyperspace::reg::Registry;

#[async_trait]
pub trait Platform: Send + Sync + Sized + Clone
where
    Self::Err: std::error::Error + Send + Sync,
    Self: 'static,
    Self::RegistryContext: Send + Sync,
    Self::StarAuth: HyperAuthenticator,
    Self::RemoteStarConnectionFactory: HyperwayEndpointFactory,
{
    type Err;
    type RegistryContext;
    type StarAuth;
    type RemoteStarConnectionFactory;

    async fn machine(&self) -> Result<MachineApi,Self::Err> {
        Ok(Machine::new_api(self.clone()).await?)
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
        fn logger(&self) -> RootLogger {
        Default::default()
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

                    return Err(SpaceErr::expect_sub::<UserBaseSubKindBase>(BaseKind::UserBase));
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
                return Err(SpaceErr::unimplemented("stars cannot be created via the template"))
            }
            BaseKind::Driver => Kind::Driver,
            BaseKind::Global => Kind::Global,
            BaseKind::Host => Kind::Host,
            BaseKind::Guest => Kind::Guest,
            BaseKind::Native => Kind::Native(NativeSub::Web),
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