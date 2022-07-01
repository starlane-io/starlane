use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::star::core::particle::driver::ParticleCoreDriver;
use crate::star::core::particle::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Change, Notification, Property, Topic, WatchSelector};
use crate::message::delivery::Delivery;
use crate::frame::{StarMessage, StarMessagePayload};

use std::str::FromStr;
use mesh_portal::version::latest::command::common::{SetProperties, StateSrc};
use mesh_portal::version::latest::entity::request::create::{Create, KindTemplate, PointSegFactory, PointTemplate, Strategy, Template};
use mesh_portal::version::latest::entity::request::{Method, Rc, ReqCore};
use mesh_portal::version::latest::id::{AddressAndKind, KindParts, Point};
use mesh_portal::version::latest::messaging::ReqShell;
use mesh_portal::version::latest::sys::Assign;
use mesh_portal_versions::version::v0_0_1::id::{ArtifactSubKind, FileSubKind};
use mesh_portal_versions::version::v0_0_1::id::id::{Kind, BaseKind};
use mesh_portal_versions::version::v0_0_1::wave::PingProto;

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

        if let Kind::File(file_kind) = &assign.details.stub.kind
        {
            match file_kind {
                FileSubKind::Dir => {
                    // stateless
                }
                _ => {
                    let state = match &assign.state {
                        StateSrc::Substance(data) => {
                            data.clone()
                        },
                        StateSrc::None => {
                            return Err("Artifact cannot be stateless".into())
                        },
                    };
                    self.store.put(assign.details.stub.point.clone(), *state.clone() ).await?;

                    let selector = WatchSelector{
                        topic: Topic::Point(assign.details.stub.point),
                        property: Property::State
                    };

                    self.skel.watch_api.fire( Notification::new(selector, Change::State(*state) ));

                }
            }
        } else {
            return Err("File Manager unexpected kind".into() );
        }


        Ok(())
    }


    fn kind(&self) -> BaseKind {
        BaseKind::File
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
    fn kind(&self) -> BaseKind {
        BaseKind::FileSystem
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
            point: Point::from_str( format!("{}:/", assign.details.stub.point.to_string()).as_str())?,
            kind: Kind::File(FileSubKind::Dir)
        };

        let skel = self.skel.clone();
        tokio::spawn( async move {
            let create = Create {
                template: Template {
                    point: PointTemplate { parent: assign.details.stub.point.clone(), child_segment_template: PointSegFactory::Exact(root_point_and_kind.point.last_segment().expect("expected final segment").to_string()) },
                    kind: KindTemplate { base: root_point_and_kind.kind.base(), sub: root_point_and_kind.kind.sub().into(), specific: None }
                },
                state: StateSrc::None,
                properties: SetProperties::new(),
                strategy: Strategy::Commit,
                registry: Default::default()
            };

            let request : ReqCore = create.into();
            let request = ReqShell::new(request, assign.details.stub.point.clone(), Point::global_executor() );
            skel.messaging_api.request(request).await;
        });
        Ok(())
    }



}
