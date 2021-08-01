use std::collections::HashSet;
use std::convert::TryInto;
use std::fs::File;
use std::io::Write;
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use tempdir::TempDir;
use tokio::sync::Mutex;

use starlane_resources::ResourceIdentifier;


use crate::star::core::resource::state::StateStore;
use crate::star::core::resource::host::Host;
use crate::star::StarSkel;
use crate::util;
/*
=======
use crate::resource::state_store::StateStore;
use crate::star::core::component::resource::host::Host;
use crate::star::StarSkel;
use crate::util;

>>>>>>> f2361a20ec5930eab8327e64fbc6e3b3d95d08d0:rust/starlane-core/src/core/artifact.rs
pub struct ArtifactHost {
    skel: StarSkel,
    file_access: FileAccess,
    store: StateStore,
    mutex: Arc<Mutex<u8>>,
}

impl ArtifactHost {
    pub async fn new(skel: StarSkel, file_access: FileAccess) -> Result<Self, Error> {
        let file_access = file_access.with_path("bundles".to_string())?;
        let rtn = ArtifactHost {
            skel: skel.clone(),
            file_access: file_access,
            store: StateStore::new(skel).await,
            mutex: Arc::new(Mutex::new(0)),
        };

        Ok(rtn)
    }



    fn bundle_key(key: ResourceKey) -> Result<ArtifactBundleKey, Fail> {
        let bundle_key = match key {
            ResourceKey::ArtifactBundle(key) => key,
            ResourceKey::Artifact(artifact) => artifact.parent().unwrap().try_into()?,
            key => {
                return Err(Fail::WrongResourceType {
                    expected: HashSet::from_iter(vec![
                        ResourceType::ArtifactBundle,
                        ResourceType::Artifact,
                    ]),
                    received: key.resource_type(),
                })
            }
        };

        Ok(bundle_key)
    }

    fn bundle_path(key: ResourceKey) -> Result<Path, Fail> {
        Ok(Path::from_str(
            format!("/{}", key.to_snake_case().as_str()).as_str(),
        )?)
    }

    fn zip_bundle_path(key: ResourceKey) -> Result<Path, Fail> {
        let bundle_path = Self::bundle_path(key)?;
        Ok(bundle_path.cat(&Path::from_str("bundle.zip")?)?)
    }

    async fn ensure_bundle_dir(&mut self, key: ResourceKey) -> Result<(), Fail> {
        if key.resource_type() == ResourceType::ArtifactBundleVersions {
            return Ok(());
        }

        let path = Self::bundle_path(key)?;
        self.file_access.mkdir(&path).await?;
        Ok(())
    }

    fn validate(assign: &ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>) -> Result<(), Fail> {
        match &assign.stub.key {
            ResourceKey::ArtifactBundle(_) => Ok(()),
            ResourceKey::Artifact(_) => Ok(()),
            ResourceKey::ArtifactBundleVersions(_) => Ok(()),
            key => {
                Err(Fail::WrongResourceType {
                    expected: HashSet::from_iter(vec![
                        ResourceType::ArtifactBundle,
                        ResourceType::Artifact,
                    ]),
                    received: key.resource_type(),
                })
            }
        }

    }

    async fn ensure_artifact(
        parent: ResourceStub,
        artifact_path: String,
        skel: StarSkel,
    ) -> Result<(), Error> {
        let archetype = ResourceArchetype {
            kind: ResourceKind::Artifact(ArtifactKind::Raw),
            specific: None,
            config: None,
        };

        let create = ResourceCreate {
            key: KeyCreationSrc::None,
            parent: parent.key.clone().into(),
            archetype: archetype,
            address: AddressCreationSrc::Append(artifact_path),
            state_src: AssignResourceStateSrc::AlreadyHosted,
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::Ensure,
        };

        let rx = ResourceCreationChamber::new(parent, create, skel.clone()).await;

        let assign = util::wait_for_it_whatever(rx).await??;
        let stub = assign.stub.clone();
        let (action,rx) = StarCoreAction::new(StarCoreCommand::Assign(assign.try_into()?));
/*
        skel.core_tx.send( action ).await;

        util::wait_for_it_whatever(rx).await??;

        let resource_record = ResourceRecord::new(stub, skel.info.key.clone() );
        let registration = ResourceRegistration::new(resource_record, Option::None );
        skel.registry.as_ref().expect("expected resource register to be available on ArtifactStore").register(registration).await;

        Ok(())
        joj
 */
        unimplemented!()
    }
}

#[async_trait]
impl Host for ArtifactHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<(), Fail> {

        Self::validate(&assign)?;

        // if there is Initialization to do for assignment THIS is where we do it
        self.ensure_bundle_dir(assign.stub.key.clone()).await?;

        let artifacts = match assign.stub.key.resource_type() {
            ResourceType::ArtifactBundle => {
                match &assign.state_src {
                    AssignResourceStateSrc::Direct(state_set_src) => {

                        let src = state_set_src.get(&"content".to_string() ).cloned().ok_or("expected content state aspect for ArtifactBundle")?;
                        let content= src.to_bin(self.skel.machine.bin_context() )?;
                        let artifacts = get_artifacts(content.clone() )?;
                        let path = Self::bundle_path(assign.key())?;
                        let mut file_access = self.file_access.with_path(path.to_relative())?;
                        file_access
                            .write(&Path::from_str("/bundle.zip")?, content)
                            .await?;

                        artifacts
                    }
                    AssignResourceStateSrc::AlreadyHosted => {
                        // do nothing, the file should already be present in the filesystem detected by the watcher and
                        // this call to assign is just making sure the database registry is updated
                        vec![]
                    }
                    AssignResourceStateSrc::None => {
                        return Err(Fail::Error(
                            "ArtifactBundle state should never be None".to_string(),
                        ));
                    }
                    AssignResourceStateSrc::CreateArgs(_) => {
                        return Err(Fail::Error(
                            "ArtifactBundle cannot be created from CreateArgs".to_string(),
                        ));
                    }
                }
            }
            ResourceType::Artifact => {
                vec![]
            }
            ResourceType::ArtifactBundleVersions=> {
                vec![]
            }
            rt => {
                return Err(Fail::WrongResourceType {
                    expected: HashSet::from_iter(vec![
                        ResourceType::ArtifactBundle,
                        ResourceType::Artifact,
                    ]),
                    received: rt,
                });
            }
        };


        let assign = ResourceAssign {
            stub: assign.stub.clone(),
            state_src: DataSet::new(),
        };

        self.store.put(assign.clone()).await?;

        Ok(())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        todo!()
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Fail> {
        self.store.get(key).await
    }

    /*
    async fn state(&self, key: ResourceKey) -> Result<DataSet<BinSrc>, Fail> {
        if let Ok(Option::Some(resource)) = self.store.get(key.clone()).await {
            match key.resource_type() {
                ResourceType::File => {
                    let filesystem_key: FileSystemKey = resource
                        .key()
                        .ancestor_of_type(ResourceType::FileSystem)?
                        .try_into()?;
                    let filesystem_path =
                        Path::from_str(format!("/{}", filesystem_key.to_string().as_str()).as_str())?;
                    let path = format!(
                        "{}{}",
                        filesystem_path.to_string(),
                        resource.address().last_to_string()
                    );
                    let data = self
                        .file_access
                        .read(&Path::from_str(path.as_str())?)
                        .await?;
                    let mut state = DataSet::new();
                    state.insert("content".to_string(), BinSrc::Memory(data) );
                    Ok(state)
                }
                _ => Ok(DataSet::new()),
            }
        } else {
            Err(Fail::ResourceNotFound(key.into()))
        }
    }

     */

    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Fail> {
        unimplemented!("I don't know how to DELETE yet.");
        Ok(())
    }

    fn shutdown(&self) {
        self.store.close();
        self.file_access.close();
    }

}

fn get_artifacts(data: Arc<Vec<u8>>) -> Result<Vec<String>, Fail> {
    let temp_dir = TempDir::new("zipcheck")?;

    let temp_path = temp_dir.path().clone();
    let file_path = temp_path.with_file_name("file.zip");
    let mut file = File::create(file_path.as_path())?;
    file.write_all(data.as_slice())?;

    let file = File::open(file_path.as_path())?;
    let archive = zip::ZipArchive::new(file);
    match archive {
        Ok(mut archive) => {
            let mut artifacts = vec![];
            for i in 0..archive.len() {
                let file = archive.by_index(i).unwrap();
                if !file.name().ends_with("/") {
                    artifacts.push(file.name().to_string())
                }
            }
            Ok(artifacts)
        }
        Err(_error) => Err(Fail::InvalidResourceState(
            "ArtifactBundle must be a properly formatted Zip file.".to_string(),
        )),
    }
}



 */