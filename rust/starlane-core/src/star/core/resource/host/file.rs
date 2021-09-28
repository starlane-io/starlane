use std::convert::TryInto;
use std::sync::Arc;

use clap::{App, AppSettings};
use yaml_rust::Yaml;

use starlane_resources::{AssignKind, AssignResourceStateSrc, Resource, ResourceAssign, ResourcePath};
use starlane_resources::message::{ResourcePortMessage, Message, ResourceRequestMessage};
use starlane_resources::data::{BinSrc, DataSet, Meta};
use starlane_resources::message::Fail;

use crate::artifact::ArtifactRef;
use crate::error::Error;
use crate::resource::{ArtifactKind, ResourceAddress, ResourceKey, ResourceType};
use crate::resource::create_args::{artifact_bundle_address, create_args_artifact_bundle, space_address};
use crate::star::core::resource::host::Host;
use crate::star::core::resource::state::StateStore;
use crate::star::StarSkel;
use crate::watch::{Notification, Change, Topic, WatchSelector, Property};
use crate::message::resource::Delivery;
use starlane_resources::http::HttpRequest;
use starlane_resources::http::HttpResponse;
use crate::html::html_error_code;
use crate::frame::{Reply, StarMessagePayload, MessagePayload, StarMessage};

use std::str::FromStr;

#[derive(Debug)]
pub struct FileHost {
    skel: StarSkel,
    store: StateStore,
}

impl FileHost {
    pub async fn new(skel: StarSkel) -> Self {
        FileHost {
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for FileHost {
    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Error> {
        let state = match assign.state_src {
            AssignResourceStateSrc::Direct(data) => data,
            AssignResourceStateSrc::Stateless => return Err("File cannot be stateless".into()),
            _ => {
                return Err("File must specify Direct state".into() )
            }
        };

        let state= self.store.put(assign.stub.key.clone(), state ).await?;

        let selector = WatchSelector{
            topic: Topic::Resource(assign.stub.key),
            property: Property::State
        };

        self.skel.watch_api.fire( Notification::new(selector, Change::State(state.clone()) ));

        Ok(state)
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

    async fn get_state(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Error> {
        self.store.get(key).await
    }


    async fn update_state(&self,key: ResourceKey, state: DataSet<BinSrc> ) -> Result<(),Error> {

        self.store.put( key.clone(), state.clone() ).await?;

        let selector = WatchSelector{
            topic: Topic::Resource(key),
            property: Property::State
        };

        self.skel.watch_api.fire( Notification::new(selector, Change::State(state.clone()) ));

        Ok(())
    }


    async fn delete(&self, _identifier: ResourceKey) -> Result<(), Error> {
        unimplemented!()
    }

    fn resource_type(&self) -> ResourceType {
        ResourceType::File
    }

    async fn http_message(&self, key: ResourceKey, delivery: Delivery<Message<HttpRequest>>) -> Result<(),Error> {

       let state = self.store.get(key).await?.ok_or("expected state to be in the store")?;
       let content = state.get("content").ok_or("expected file to have content")?.clone();
       let mut response = HttpResponse::new();
       response.status = 200;
       response.body = Option::Some(content);
       delivery.reply(Reply::HttpResponse(response));

       Ok(())
    }

}


pub struct FileSystemHost {
    skel: StarSkel,
    store: StateStore,
}

impl FileSystemHost{
    pub async fn new( skel: StarSkel ) -> Self {

        FileSystemHost{
            skel: skel.clone(),
            store: StateStore::new(skel).await,
        }
    }
}

#[async_trait]
impl Host for FileSystemHost {
    fn resource_type(&self) -> ResourceType {
        ResourceType::FileSystem
    }

    async fn assign(
        &self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<DataSet<BinSrc>, Error> {
        match assign.state_src {
            AssignResourceStateSrc::Stateless => {}
            AssignResourceStateSrc::CreateArgs(_) => {}
            _ => {
                return Err("must be stateless or empty create args".into());
            }
        };

        Ok(DataSet::new())
    }

    async fn has(&self, key: ResourceKey) -> bool {
        match self.store.has(key).await {
            Ok(v) => v,
            Err(_) => false,
        }
    }

    async fn get_state(&self, key: ResourceKey) -> Result<Option<DataSet<BinSrc>>, Error> {
        self.store.get(key).await
    }



    async fn delete(&self, key: ResourceKey) -> Result<(), Error> {
        todo!()
    }

    async fn http_message(&self, key: ResourceKey, delivery: Delivery<Message<HttpRequest>>) -> Result<(),Error> {
        let record = self.skel.resource_locator_api.locate(key.into()).await?;

        let filepath = if delivery.payload.payload.path.ends_with("/") {
            format!("{}:{}index.html", record.stub.address.to_string(),delivery.payload.payload.path )
        } else {
            format!("{}:{}", record.stub.address.to_string(),delivery.payload.payload.path )
        };

        eprintln!("FILEPATH: {}", filepath );
        let filepath = ResourcePath::from_str(filepath.as_str())?;
        let mut message = delivery.payload.clone();
        message.to = filepath.into();
        let mut star_message:StarMessage = delivery.into();
        star_message.payload = StarMessagePayload::MessagePayload(MessagePayload::HttpRequest(message));
        self.skel.router_api.route(star_message);
        Ok(())
    }



}
