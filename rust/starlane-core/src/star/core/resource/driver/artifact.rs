use std::cmp::Ordering;
use std::collections::{HashSet, HashMap};
use std::convert::{TryFrom, TryInto};
use std::fs::File;
use std::io::{Read, Write};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;

use tempdir::TempDir;
use tokio::sync::Mutex;

use crate::resource::{ResourceType, AssignResourceStateSrc, ResourceAssign, Kind, ArtifactKind};
use crate::star::core::resource::driver::ResourceCoreDriver;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::util;
use crate::error::Error;

use crate::message::delivery::Delivery;
use mesh_portal::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal::version::latest::entity::request::create::{AddressSegmentTemplate, AddressTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal::version::latest::entity::request::{Action, Rc};
use mesh_portal::version::latest::id::{Address, AddressAndKind, KindParts, RouteSegment};
use mesh_portal::version::latest::messaging::Request;
use mesh_portal::version::latest::payload::{Payload, Primitive};
use zip::result::ZipResult;
use crate::file_access::FileAccess;


fn get_artifacts(data: Arc<Vec<u8>>) -> Result<Vec<String>, Error> {
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
        Err(_error) => Err(
            "ArtifactBundle must be a properly formatted Zip file.".into(),
        ),
    }
}

#[derive(Debug)]
pub struct ArtifactBundleCoreDriver {
    skel: StarSkel,
    store: StateStore,
}

impl ArtifactBundleCoreDriver {
    pub async fn new(skel: StarSkel) -> Self {
        ArtifactBundleCoreDriver {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}

#[async_trait]
impl ResourceCoreDriver for ArtifactBundleCoreDriver {
    fn resource_type(&self) -> ResourceType {
        ResourceType::ArtifactBundle
    }

    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        let state = match &assign.state {
            StateSrc::StatefulDirect(data) => {
                data.clone()
            },
            StateSrc::Stateless => {
                return Err("ArtifactBundle cannot be stateless".into())
            },

        };

        if let Payload::Primitive( Primitive::Bin(zip) ) = state.clone() {

            let temp_dir = TempDir::new("zipcheck")?;
            let temp_path = temp_dir.path().clone();
            let file_path = temp_path.with_file_name("file.zip");
            let mut file = File::create(file_path.as_path())?;
            file.write_all(zip.as_slice())?;

            let file = File::open(file_path.as_path())?;
            let mut archive = zip::ZipArchive::new(file)?;
            let mut artifacts = vec![];
            for i in 0..archive.len() {
               let file = archive.by_index(i).unwrap();
                if !file.name().ends_with("/") {
                            artifacts.push(file.name().to_string())
                }
             }

            let mut address_and_kind_set = HashSet::new();
            for artifact in artifacts {
                let mut path = String::new();
                let segments = artifact.split("/");
                let segments :Vec<&str> = segments.collect();
                for (index,segment) in segments.iter().enumerate() {
                    path.push_str(segment);
                    if index < segments.len()-1 {
                        path.push_str("/");
                    }
                    let address = Address::from_str( format!( "{}:/{}",assign.stub.address.to_string(), path.as_str()).as_str() )?;
                    let kind = if index < segments.len()-1 {
                        KindParts { resource_type: "Artifact".to_string(), kind: Option::Some("Dir".to_string()), specific: None }
                    }  else {
                        KindParts { resource_type: "Artifact".to_string(), kind: Option::Some("Raw".to_string()), specific: None }
                    };
                    let address_and_kind = AddressAndKind {
                        address,
                        kind
                    };
                    address_and_kind_set.insert( address_and_kind );
                }

            }

            let root_address_and_kind = AddressAndKind {
               address: Address::from_str( format!( "{}:/",assign.stub.address.to_string()).as_str())?,
               kind: KindParts { resource_type: "Artifact".to_string(), kind: Option::Some("Dir".to_string()), specific: None }
            };


            address_and_kind_set.insert( root_address_and_kind );

            let mut address_and_kind_set: Vec<AddressAndKind>  = address_and_kind_set.into_iter().collect();

            // shortest first will ensure that dirs are created before files
            address_and_kind_set.sort_by(|a,b|{
                if a.address.to_string().len() > b.address.to_string().len() {
                    Ordering::Greater
                } else if a.address.to_string().len() < b.address.to_string().len() {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            {
                let skel = self.skel.clone();
                let assign = assign.clone();
                tokio::spawn(async move {
                    for address_and_kind in address_and_kind_set {
                        let parent = address_and_kind.address.parent().expect("expected parent");
                        let result:Result<Kind,mesh_portal::error::MsgErr> = TryFrom::try_from(address_and_kind.kind.clone());
                        match result {
                            Ok(kind) => {
                                let state = match kind {
                                    Kind::Artifact(ArtifactKind::Dir) => {
                                        StateSrc::Stateless
                                    }
                                    Kind::Artifact(_) => {
                                        let mut path = address_and_kind.address.filepath().expect("expecting non Dir artifact to have a filepath");
                                        // convert to relative path
                                        path.remove(0);
                                        match archive.by_name(path.as_str()) {
                                            Ok(mut file) => {
                                                let mut buf = vec![];
                                                file.read_to_end(&mut buf);
                                                let bin = Arc::new(buf);
                                                let payload = Payload::Primitive(Primitive::Bin(bin));
                                                StateSrc::StatefulDirect(payload)
                                            }
                                            Err(err) => {
                                                eprintln!("Artifact archive error: {}", err.to_string() );
                                                StateSrc::Stateless
                                            }
                                        }
                                    }
                                    _ => {panic!("unexpected knd");}
                                };

                                let create = Create {
                                    template: Template {
                                        address: AddressTemplate { parent: parent.clone(), child_segment_template: AddressSegmentTemplate::Exact(address_and_kind.address.last_segment().expect("expected final segment").to_string()) },
                                        kind: KindTemplate { resource_type: address_and_kind.kind.resource_type.clone(), kind: address_and_kind.kind.kind.clone(), specific: None }
                                    },
                                    state,
                                    properties: SetProperties::new(),
                                    strategy: Strategy::Create,
                                    registry: Default::default()
                                };

                                let action = Action::Rc(Rc::Create(create));
                                let core = action.into();
                                let request = Request::new(core, assign.stub.address.clone(), parent);
                                let response = skel.messaging_api.request(request).await;

                            }
                            Err(err) => {
                                eprintln!("Artifact Kind Error: {}", err.to_string());
                            }
                        };
                    }
                });
            }
        }
        else {
            return Err("ArtifactBundle Manager expected Bin payload".into())
        }

        self.store.put( assign.stub.address, state ).await?;

        // need to unzip and create Artifacts for each...



        Ok(())
    }



    async fn get(&self, address: Address) -> Result<Payload,Error> {
        self.store.get(address).await
    }


}

#[derive(Debug)]
pub struct ArtifactManager {
    skel: StarSkel,
    store: StateStore,
}

impl ArtifactManager{
    pub async fn new(skel: StarSkel) -> Self {
        Self {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}


#[async_trait]
impl ResourceCoreDriver for ArtifactManager{
    fn resource_type(&self) -> ResourceType {
        ResourceType::Artifact
    }

    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        let kind : Kind = TryFrom::try_from(assign.stub.kind)?;
        if let Kind::Artifact(artifact_kind) = kind
        {
            match artifact_kind {
                ArtifactKind::Dir => {
                    // stateless
                    Ok(())
                }
                _ => {
                    let state = match &assign.state {
                        StateSrc::StatefulDirect(data) => {
                            data.clone()
                        },
                        StateSrc::Stateless => {
                            return Err("Artifact cannot be stateless".into())
                        },
                    };
                    self.store.put( assign.stub.address.clone(), state ).await?;
                    Ok(())
                }
            }
        } else {
            Err("Artifact Manager unexpected kind".into() )
        }
    } // assign



    async fn get(&self, address: Address) -> Result<Payload,Error> {
        self.store.get(address).await
    }

}
