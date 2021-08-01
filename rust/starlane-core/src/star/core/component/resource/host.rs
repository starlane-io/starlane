use crate::core::InertHost;
use crate::data::{BinSrc, DataSet};
use crate::error::Error;
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, Resource, ResourceAssign, ResourceKey, ResourceType, ResourceKind, ResourceAddress};
use crate::resource::state_store::StateStore;
use crate::star::StarSkel;
use actix_web::App;
use crate::artifact::ArtifactRef;
use clap::ArgMatches;

#[async_trait]
pub trait Host: Send + Sync {
    async fn assign(
        &mut self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<(), Fail>;
    async fn get(&self, key: ResourceKey) -> Result<DataSet<BinSrc>, Fail>;
}

pub fn create_host_for( rt: ResourceType, skel: StarSkel ) -> Result<Box<dyn Host>,Error> {
    match rt {
       ResourceType::Space => Ok(SpaceHost::new(skel)),
       _ => Ok(DefaultHost::new(skel))
    }
}

async fn get_arg_matches(skel: &StarSkel, kind: &ResourceKind, args: String ) -> Result<ArgMatches,Error> {
    /*
    let artifact = kind.init_clap_config().expect("expected init clap config");
    let artifact: ResourceAddress =  artifact.into();
    info!("got init clapConfig");
    let mut cache = skel.caches.create();
    println!("artifact is::: {}",artifact.to_string());
    info!("got self.skel.caches.create()");
    let artifact_ref = ArtifactRef::new(artifact.clone(), ArtifactKind::Raw );
    match cache.cache(vec![artifact_ref]).await {
        Ok(_) => {}
        Err(err) => {error!("{}",err);
            return Err(err.into());
        }
    };
    info!("artifact cached: {}", artifact.to_string() );
    let caches = cache.to_caches().await?;
    info!("cache.to_caches()." );
    let yaml_config = caches.raw.get(&artifact).ok_or("expected artifact")?;
    info!("caches.raw.get()." );
    let data = (*yaml_config.data()).clone();
    let data = String::from_utf8(data)?;
    info!("String::from_utf8()." );
    let yaml = clap::YamlLoader::load_from_str(data.as_str() )?;
    info!("clap::YamlLoader::load_from_str(data.as_str() )" );
    let yaml = yaml.get(0).ok_or("expected at least one Yaml expression")?.clone();
    info!("at least one yaml" );
    let mut app = App::from(&yaml);
    info!("App::from(&yaml)" );
    let matches = app.get_matches_from_safe(args.split(" "))?;
    Ok(app)
     */

    unimplemented!()
}


#[derive(Debug)]
pub struct SpaceHost {
    skel: StarSkel,
    store: StateStore,
}

impl SpaceHost {
    pub async fn new(skel: StarSkel) -> Self {
        SpaceHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for SpaceHost {
    async fn assign(
        &mut self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<(), Fail> {
        // if there is Initialization to do for assignment THIS is where we do it
        let state = match assign.state_src {
            AssignResourceStateSrc::Direct(data) => {
                data
            }
            AssignResourceStateSrc::AlreadyHosted => DataSet::new(),
            AssignResourceStateSrc::None => {
                return Fail::Error("Space cannot be Stateless")
            }
            AssignResourceStateSrc::CreateArgs(ref args) =>  {
                unimplemented!()
            }
        };

        let assign = ResourceAssign {
            stub: assign.stub,
            state_src: state,
        };

        Ok(self.store.put(assign).await?)
    }

    async fn get(&self, key: ResourceKey) -> Result<DataSet<BinSrc>, Fail> {
        self.store.get(key).await
    }

}




#[derive(Debug)]
pub struct DefaultHost {
    skel: StarSkel,
    store: StateStore,
}

impl DefaultHost {
    pub async fn new(skel: StarSkel) -> Self {
        DefaultHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for DefaultHost {
    async fn assign(
        &mut self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<Resource, Fail> {
        // if there is Initialization to do for assignment THIS is where we do it
        let state = match assign.state_src {
            AssignResourceStateSrc::Direct(data) => {
                data
            }
            AssignResourceStateSrc::AlreadyHosted => DataSet::new(),
            AssignResourceStateSrc::None => DataSet::new(),
            AssignResourceStateSrc::CreateArgs(ref args) =>  {
                if args.trim().is_empty() && assign.stub.archetype.kind.init_clap_config().is_none() {
                    DataSet::new()
                } else if assign.stub.archetype.kind.init_clap_config().is_none(){
                    return Err(format!("resource {} does not take init args",assign.archetype().kind.to_string()).into());
                }
                else {
                    /*
info!("enter");
                    let artifact = assign.archetype().kind.init_clap_config().expect("expected init clap config");
                    let artifact: ResourceAddress =  artifact.into();
info!("got init clapConfig");
                    let mut cache = self.skel.caches.create();
println!("artifact is::: {}",artifact.to_string());
info!("got self.skel.caches.create()");
                    let artifact_ref = ArtifactRef::new(artifact.clone(), ArtifactKind::Raw );
                    match cache.cache(vec![artifact_ref]).await {
                        Ok(_) => {}
                        Err(err) => {error!("{}",err);
                            return Err(err.into());
                        }
                    };
info!("artifact cached: {}", artifact.to_string() );
                    let caches = cache.to_caches().await?;
info!("cache.to_caches()." );
                    let yaml_config = caches.raw.get(&artifact).ok_or("expected artifact")?;
info!("caches.raw.get()." );
                    let data = (*yaml_config.data()).clone();
                    let data = String::from_utf8(data)?;
info!("String::from_utf8()." );
                    let yaml = clap::YamlLoader::load_from_str(data.as_str() )?;
info!("clap::YamlLoader::load_from_str(data.as_str() )" );
                    let yaml = yaml.get(0).ok_or("expected at least one Yaml expression")?.clone();
info!("at least one yaml" );
                    let mut app = App::from(&yaml);
info!("App::from(&yaml)" );
                    let matches = app.get_matches_from_safe(args.split(" "))?;
                    // now not sure what to do with matches
println!("seems to have worked....");
                    MemoryDataTransfer::none()
                     */
                    unimplemented!()
                }
            }


        };

        let assign = ResourceAssign {
            stub: assign.stub,
            state_src: state,
        };

        self.store.put(assign).await?;
        Ok(())
    }

    async fn get(&self, key: ResourceKey) -> Result<DataSet<BinSrc>, Fail> {
        self.store.get(key).await
    }

}
