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

use crate::particle::{KindBase, AssignParticleStateSrc, ParticleAssign, Kind, ArtifactSubKind};
use crate::star::core::resource::driver::ParticleCoreDriver;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::util;
use crate::error::Error;

use crate::message::delivery::Delivery;
use mesh_portal::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal::version::latest::entity::request::create::{PointSegFactory, PointTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal::version::latest::entity::request::{Method, Rc};
use mesh_portal::version::latest::id::{Point, AddressAndKind, KindParts, RouteSegment};
use mesh_portal::version::latest::messaging::Request;
use mesh_portal::version::latest::payload::{Payload};
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
impl ParticleCoreDriver for ArtifactBundleCoreDriver {
    fn kind(&self) -> KindBase {
        KindBase::ArtifactBundle
    }

    async fn assign(
        &mut self,
        assign: ParticleAssign,
    ) -> Result<(), Error> {
        let state = match &assign.state {
            StateSrc::Payload(data) => {
                data.clone()
            },
            StateSrc::None => {
                return Err("ArtifactBundle cannot be stateless".into())
            },

        };

        if let Payload::Bin(zip ) = state.clone() {

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

            let mut point_and_kind_set = HashSet::new();
            for artifact in artifacts {
                let mut path = String::new();
                let segments = artifact.split("/");
                let segments :Vec<&str> = segments.collect();
                for (index,segment) in segments.iter().enumerate() {
                    path.push_str(segment);
                    if index < segments.len()-1 {
                        path.push_str("/");
                    }
                    let point = Point::from_str( format!("{}:/{}", assign.details.stub.point.to_string(), path.as_str()).as_str() )?;
                    let kind = if index < segments.len()-1 {
                        KindParts { kind: "Artifact".to_string(), sub_kind: Option::Some("Dir".to_string()), specific: None }
                    }  else {
                        KindParts { kind: "Artifact".to_string(), sub_kind: Option::Some("Raw".to_string()), specific: None }
                    };
                    let point_and_kind = AddressAndKind {
                        point,
                        kind
                    };
                    point_and_kind_set.insert( point_and_kind );
                }

            }

            let root_point_and_kind = AddressAndKind {
               point: Point::from_str( format!("{}:/", assign.details.stub.point.to_string()).as_str())?,
               kind: KindParts { kind: "Artifact".to_string(), sub_kind: Option::Some("Dir".to_string()), specific: None }
            };


            point_and_kind_set.insert( root_point_and_kind );

            let mut point_and_kind_set: Vec<AddressAndKind>  = point_and_kind_set.into_iter().collect();

            // shortest first will ensure that dirs are created before files
            point_and_kind_set.sort_by(|a,b|{
                if a.point.to_string().len() > b.point.to_string().len() {
                    Ordering::Greater
                } else if a.point.to_string().len() < b.point.to_string().len() {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            });

            {
                let skel = self.skel.clone();
                let assign = assign.clone();
                tokio::spawn(async move {
                    for point_and_kind in point_and_kind_set {
                        let parent = point_and_kind.point.parent().expect("expected parent");
                        let result:Result<Kind,mesh_portal::error::MsgErr> = TryFrom::try_from(point_and_kind.kind.clone());
                        match result {
                            Ok(kind) => {
                                let state = match kind {
                                    Kind::Artifact(ArtifactSubKind::Dir) => {
                                        StateSrc::None
                                    }
                                    Kind::Artifact(_) => {
                                        let mut path = point_and_kind.point.filepath().expect("expecting non Dir artifact to have a filepath");
                                        // convert to relative path
                                        path.remove(0);
                                        match archive.by_name(path.as_str()) {
                                            Ok(mut file) => {
                                                let mut buf = vec![];
                                                file.read_to_end(&mut buf);
                                                let bin = Arc::new(buf);
                                                let payload = Payload::Bin(bin);
                                                StateSrc::Payload(payload)
                                            }
                                            Err(err) => {
                                                eprintln!("Artifact archive error: {}", err.to_string() );
                                                StateSrc::None
                                            }
                                        }
                                    }
                                    _ => {panic!("unexpected knd");}
                                };

                                let create = Create {
                                    template: Template {
                                        point: PointTemplate { parent: parent.clone(), child_segment_template: PointSegFactory::Exact(point_and_kind.point.last_segment().expect("expected final segment").to_string()) },
                                        kind: KindTemplate { kind: point_and_kind.kind.kind.clone(), sub_kind: point_and_kind.kind.sub_kind.clone(), specific: None }
                                    },
                                    state,
                                    properties: SetProperties::new(),
                                    strategy: Strategy::Commit,
                                    registry: Default::default()
                                };

                                let action = Method::Cmd(Rc::Create(create));
                                let core = action.into();
                                let request = Request::new(core, assign.details.stub.point.clone(), parent);
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

        self.store.put(assign.details.stub.point, state ).await?;

        // need to unzip and create Artifacts for each...



        Ok(())
    }



    async fn get(&self, point: Point) -> Result<Payload,Error> {
        self.store.get(point).await
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
impl ParticleCoreDriver for ArtifactManager{
    fn kind(&self) -> KindBase {
        KindBase::Artifact
    }

    async fn assign(
        &mut self,
        assign: ParticleAssign,
    ) -> Result<(), Error> {
        let kind : Kind = TryFrom::try_from(assign.details.stub.kind)?;
        if let Kind::Artifact(artifact_kind) = kind
        {
            match artifact_kind {
                ArtifactSubKind::Dir => {
                    // stateless
                    Ok(())
                }
                _ => {
                    let state = match &assign.state {
                        StateSrc::Payload(data) => {
                            data.clone()
                        },
                        StateSrc::None => {
                            return Err("Artifact cannot be stateless".into())
                        },
                    };
                    self.store.put(assign.details.stub.point.clone(), state ).await?;
                    Ok(())
                }
            }
        } else {
            Err("Artifact Manager unexpected kind".into() )
        }
    } // assign



    async fn get(&self, point: Point) -> Result<Payload,Error> {
        self.store.get(point).await
    }

}
