use std::convert::{TryFrom, TryInto};
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceType, ResourceAssign, AssignResourceStateSrc, Kind, FileKind};
use crate::star::core::resource::driver::ResourceCoreDriver;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Notification, Change, Topic, WatchSelector, Property};
use crate::message::delivery::Delivery;
use crate::html::html_error_code;
use crate::frame::{StarMessagePayload, StarMessage};

use std::str::FromStr;
use mesh_portal::version::latest::command::common::StateSrc;
use mesh_portal::version::latest::entity::request::create::{AddressSegmentTemplate, AddressTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal::version::latest::entity::request::{Action, Rc};
use mesh_portal::version::latest::id::{Address, AddressAndKind, KindParts};
use mesh_portal::version::latest::messaging::Request;
use mesh_portal_versions::version::v0_0_1::command::common::SetProperties;

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
impl ResourceCoreDriver for FileCoreManager {
    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {

        let kind : Kind = TryFrom::try_from(assign.stub.kind)?;
        if let Kind::File(file_kind) = kind
        {
            match file_kind {
                FileKind::Dir => {
                    // stateless
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
                    self.store.put( assign.stub.address.clone(), state.clone() ).await?;

                    let selector = WatchSelector{
                        topic: Topic::Resource(assign.stub.address),
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


    fn resource_type(&self) -> ResourceType {
        ResourceType::File
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
impl ResourceCoreDriver for FileSystemManager {
    fn resource_type(&self) -> ResourceType {
        ResourceType::FileSystem
    }

    async fn assign(
        &mut self,
        assign: ResourceAssign,
    ) -> Result<(), Error> {
        match assign.state {
            StateSrc::Stateless => {}
            _ => {
                return Err("must be stateless or empty create args".into());
            }
        };


        let root_address_and_kind = AddressAndKind {
            address: Address::from_str( format!( "{}:/",assign.stub.address.to_string()).as_str())?,
            kind: KindParts { resource_type: "File".to_string(), kind: Option::Some("Dir".to_string()), specific: None }
        };

        let skel = self.skel.clone();
        tokio::spawn( async move {
            let create = Create {
                template: Template {
                    address: AddressTemplate { parent: assign.stub.address.clone(), child_segment_template: AddressSegmentTemplate::Exact(root_address_and_kind.address.last_segment().expect("expected final segment").to_string()) },
                    kind: KindTemplate { resource_type: root_address_and_kind.kind.resource_type.clone(), kind: root_address_and_kind.kind.kind.clone(), specific: None }
                },
                state: StateSrc::Stateless,
                properties: SetProperties::new(),
                strategy: Strategy::Create,
                registry: Default::default()
            };

            let action = Action::Rc(Rc::Create(create));
            let core = action.into();
            let request = Request::new(core, assign.stub.address.clone(), assign.stub.address.clone());
            let response = skel.messaging_api.exchange(request).await;
        });
        Ok(())
    }



}
