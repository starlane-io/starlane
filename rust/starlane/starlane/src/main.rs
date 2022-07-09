use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;
use cosmic_api::id::StarSub;
use cosmic_api::{ArtifactApi, RegistryApi};
use cosmic_api::substance::substance::Token;
use cosmic_artifact::Artifacts;
use cosmic_registry_postgres::Registry;
use cosmic_star::driver::DriversBuilder;
use cosmic_star::platform::Platform;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref STARLANE_PORT: usize = std::env::var("STARLANE_PORT").unwrap_or("4343".to_string()).parse::<usize>().unwrap_or(4343);
    pub static ref STARLANE_DATA_DIR: String= std::env::var("STARLANE_DATA_DIR").unwrap_or("data".to_string());
    pub static ref STARLANE_CACHE_DIR: String = std::env::var("STARLANE_CACHE_DIR").unwrap_or("data".to_string());
    pub static ref STARLANE_TOKEN: String = std::env::var("STARLANE_TOKEN").unwrap_or(Uuid::new_v4().to_string());
}
#[no_mangle]
pub extern "C" fn cosmic_uuid() -> String
{
    Uuid::new_v4().to_string()
}


#[no_mangle]
pub extern "C" fn cosmic_timestamp() -> DateTime<Utc>{
    Utc::now()
}


fn main() {
    println!("Hello, world!");
}

pub struct Starlane {
   registry: Arc<Registry>,
   artifacts: Arc<Artifacts>
}

impl Platform for Starlane {
    fn drivers_builder(&self, kind: &StarSub) -> DriversBuilder {
        match kind {
            StarSub::Central => {}
            StarSub::Super => {}
            StarSub::Nexus => {}
            StarSub::Maelstrom => {}
            StarSub::Scribe => {}
            StarSub::Jump => {}
            StarSub::Fold => {}
        }
        DriversBuilder::new()
    }

    fn token(&self) -> Token {
        Token::new(STARLANE_TOKEN.to_string())
    }

    fn registry(&self) -> Arc<dyn RegistryApi<E>>  {
        self.registry.clone()
    }

    fn artifacts(&self) -> Arc<dyn ArtifactApi> {
       self.artifacts.clone()
    }

    fn start_services(&self, entry_router: &mut cosmic_hyperlane::InterchangeEntryRouter) {
        todo!()
    }

    fn default_implementation(template: &KindTemplate) -> Result<Kind, PostErr> {
        let base: BaseKind = BaseKind::from_str(template.base.to_string().as_str())?;
        Ok(match base {
            BaseKind::Root => Kind::Root,
            BaseKind::Space => Kind::Space,
            BaseKind::Base => match &template.sub {
                None => {
                    return Err("kind must be set for Base".into());
                }
                Some(sub_kind) => {
                    let kind = BaseSubKind::from_str(sub_kind.as_str())?;
                    if template.specific.is_some() {
                        return Err("BaseKind cannot have a Specific".into());
                    }
                    return Ok(Kind::Base(kind));
                }
            },
            BaseKind::User => Kind::User,
            BaseKind::App => Kind::App,
            BaseKind::Mechtron => Kind::Mechtron,
            BaseKind::FileSystem => Kind::FileSystem,
            BaseKind::File => match &template.sub {
                None => return Err("expected kind for File".into()),
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
                    return Err("expected Sub for Artirtact".into());
                }
                Some(sub ) => {
                    let artifact_kind = ArtifactSubKind::from_str(sub.as_str())?;
                    return Ok(Kind::Artifact(artifact_kind));
                }
            },
            BaseKind::Control => Kind::Control,
            BaseKind::UserBase => match &template.sub {
                None => {
                    return Err("SubKind must be set for UserBase<?>".into());
                }
                Some(sub) => {
                    let specific = Specific::from_str("starlane.io:redhat.com:keycloak:community:18.0.0")?;
                    let sub = UserBaseSubKind::OAuth(specific);
                    Kind::UserBase(sub)
                }
            },
            BaseKind::Repo => Kind::Repo,
            BaseKind::Portal => Kind::Portal,
            BaseKind::Star =>  {
                unimplemented!()
            }
        })
    }
}