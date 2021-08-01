



use std::sync::Arc;







use starlane_resources::{ResourceIdentifier};



use crate::core::Host;



use crate::message::Fail;


use crate::resource::{ArtifactKind, AssignResourceStateSrc, LocalStateSetSrc, Names, RemoteDataSrc, Resource, ResourceAddress, ResourceArchetype, ResourceAssign, ResourceKind, ResourceKey};

use crate::resource::state_store::{
    StateStore,
};

use crate::star::StarSkel;
use std::collections::HashMap;
use crate::data::{DataSet, BinSrc};

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
    ) -> Result<(), Fail> {
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

        Ok(self.store.put(assign).await?)
    }

    async fn get(&self, key: ResourceKey) -> Result<DataSet<BinSrc>, Fail> {
        self.store.get(key).await
    }

    /*
    async fn state(&self, identifier: ResourceKey) -> Result<DataSet<BinSrc>, Fail> {
        if let Option::Some(resource) = self.store.get(identifier.clone()).await? {
            Ok(resource.state_src())
        } else {
            Err(Fail::ResourceNotFound(identifier.into()))
        }

    }

     */

    /*jjjj
    async fn state(&self, identifier: ResourceIdentifier) -> Result<RemoteDataSrc, Fail> {
        if let Option::Some(resource) = self.store.get(identifier.clone()).await? {
            Ok(RemoteDataSrc::Memory(resource.state_src().get().await?))
        } else {
            Err(Fail::ResourceNotFound(identifier))
        }
    }
     */

    async fn delete(&self, _identifier: ResourceKey ) -> Result<(), Fail> {
        unimplemented!()
    }
}
