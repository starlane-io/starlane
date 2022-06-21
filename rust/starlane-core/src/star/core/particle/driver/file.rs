use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::particle::{ArtifactSubKind, FileSubKind, Kind, KindBase, Assign};
use crate::star::core::particle::driver::ParticleCoreDriver;
use crate::star::core::particle::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Change, Notification, Property, Topic, WatchSelector};
use crate::message::delivery::Delivery;
use crate::frame::{StarMessage, StarMessagePayload};

use std::str::FromStr;
use mesh_portal::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal::version::latest::command::request::CmdMethod;
use mesh_portal::version::latest::entity::request::create::{Create, KindTemplate, PointSegFactory, PointTemplate, Strategy, Template};
use mesh_portal::version::latest::entity::request::{Method, Rc, RequestCore};
use mesh_portal::version::latest::id::{AddressAndKind, KindParts, Point};
use mesh_portal::version::latest::messaging::Request;
use mesh_portal_versions::version::v0_0_1::wave::ReqProto;

#[derive(Debug)]
pub struct FileCoreManager {
    skel: StarSkel,
    store: StateStore,
}

impl FileCoreManager {
    pub fn new(skel: StarSkel) -> Self {
        FileCoreManager {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}

#[async_trait]
impl ParticleCoreDriver for FileCoreManager {
    async fn assign(
        &mut self,
        assign: Assign,
    ) -> Result<(), Error> {

        let kind : Kind = TryFrom::try_from(assign.config.stub.kind)?;
        if let Kind::File(file_kind) = kind
        {
            match file_kind {
                FileSubKind::Dir => {
                    // stateless
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
                    self.store.put(assign.config.stub.point.clone(), state.clone() ).await?;

                    let selector = WatchSelector{
                        topic: Topic::Point(assign.config.stub.point),
                        property: Property::State
                    };

                    self.skel.watch_api.fire( Notification::new(selector, Change::State(state) ));

                }
            }
        } else {
            return Err("File Manager unexpected kind".into() );
        }


        Ok(())
    }


    fn kind(&self) -> KindBase {
        KindBase::File
    }

}


pub struct FileSystemManager {
    skel: StarSkel,
    store: StateStore,
}

impl FileSystemManager {
    pub async fn new( skel: StarSkel ) -> Self {

        FileSystemManager {
            skel: skel.clone(),
            store: StateStore::new(skel),
        }
    }
}

#[async_trait]
impl ParticleCoreDriver for FileSystemManager {
    fn kind(&self) -> KindBase {
        KindBase::FileSystem
    }

    async fn assign(
        &mut self,
        assign: Assign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::None => {}
            _ => {
                return Err("must be stateless or empty create args".into());
            }
        };


        let root_point_and_kind = AddressAndKind {
            point: Point::from_str( format!("{}:/", assign.config.stub.point.to_string()).as_str())?,
            kind: KindParts { kind: "File".to_string(), sub_kind: Option::Some("Dir".to_string()), specific: None }
        };

        let skel = self.skel.clone();
        tokio::spawn( async move {
            let create = Create {
                template: Template {
                    point: PointTemplate { parent: assign.config.stub.point.clone(), child_segment_template: PointSegFactory::Exact(root_point_and_kind.point.last_segment().expect("expected final segment").to_string()) },
                    kind: KindTemplate { kind: root_point_and_kind.kind.kind.clone(), sub_kind: root_point_and_kind.kind.sub_kind.clone(), specific: None }
                },
                state: StateSrc::None,
                properties: SetProperties::new(),
                strategy: Strategy::Commit,
                registry: Default::default()
            };

            let request :RequestCore= create.into();
            let request = Request::new(request, assign.config.stub.point.clone(), Point::global_executor() );
            skel.messaging_api.request(request).await;
        });
        Ok(())
    }



}
