use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use futures::FutureExt;
use semver::Version;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::oneshot::error::RecvError;
use tokio::time::error::Elapsed;

use crate::cache::ProtoArtifactCachesFactory;
use crate::error::Error;
use crate::frame::{StarPattern, TraversalAction, ResourceRegistryRequest, StarMessagePayload};
use crate::resource::{Kind, ResourceType, AssignResourceStateSrc, ResourceRecord};
use crate::resource::FileKind;
use crate::star::{Request, StarCommand, StarKind, StarSkel, StarKey};
use crate::star::shell::search::{SearchInit, SearchHits};
use crate::star::surface::SurfaceApi;
use crate::starlane::StarlaneCommand;
use crate::watch::{WatchResourceSelector, Watcher};
use crate::message::{ProtoStarMessage, ProtoStarMessageTo, ReplyKind, Reply};
use crate::artifact::ArtifactBundle;
use crate::resources::message::{ProtoRequest, MessageFrom};
use kube::ResourceExt;
use mesh_portal_serde::version::latest::command::common::{SetLabel, StateSrc};
use mesh_portal_serde::version::latest::entity::request::create::{AddressSegmentTemplate, AddressTemplate, Create, KindTemplate, Strategy, Template};
use mesh_portal_serde::version::latest::entity::request::{Rc, RcCommand, ReqEntity};
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::payload::{Payload, PayloadMap, Primitive};
use mesh_portal_serde::version::latest::resource::ResourceStub;
use mesh_portal_versions::version::v0_0_1::id::Tks;

use crate::fail::{Fail, StarlaneFailure};
use crate::cache::RootItemCacheCall::Get;

#[derive(Clone)]
pub struct StarlaneApi {
    surface_api: SurfaceApi,
    starlane_tx: Option<mpsc::Sender<StarlaneCommand>>,
}

impl StarlaneApi {



}




impl StarlaneApi {
    pub fn new(surface_api: SurfaceApi) -> Self {
        Self::new_with_options(surface_api, Option::None)
    }
    fn new_with_options(
        surface_api: SurfaceApi,
        starlane_tx: Option<mpsc::Sender<StarlaneCommand>>,
    ) -> Self {
        Self {
            surface_api,
            starlane_tx,
        }
    }

    pub fn with_starlane_ctrl(
        surface_api: SurfaceApi,
        starlane_tx: mpsc::Sender<StarlaneCommand>,
    ) -> Self {
        Self::new_with_options(surface_api, Option::Some(starlane_tx))
    }

    pub fn shutdown(&self) -> Result<(), Error> {
        self.starlane_tx.as_ref().ok_or("this api does not have access to the StarlaneMachine and therefore cannot do a shutdown")?.try_send(StarlaneCommand::Shutdown);
        Ok(())
    }

    pub async fn timeout<T>(
        rx: tokio::sync::oneshot::Receiver<Result<T, Error>>,
    ) -> Result<T, Error> {
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(result) => match result {
                Ok(result) => result,
                Err(_err) => Err("recv error".into()),
            },
            Err(err) => {
                eprintln!("elapsed error: {}", err);
                Err("timeout".into())
            }
        }
    }

    pub async fn fetch_resource_record(
        &self,
        address: Address,
    ) -> Result<ResourceRecord, Error> {
        self.surface_api.locate(address).await
    }

    pub async fn get_caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Error> {
        Ok(self.surface_api.get_caches().await?)
    }

    pub async fn create(&self, create: Create) -> Result<ResourceRecord, Error> {

        let mut proto = ProtoRequest::new();
        proto.to(create.template.address.parent.clone());
        let command = RcCommand::Create( create );
        proto.entity( ReqEntity::Rc(Rc::new(command)));
        let proto = proto.try_into()?;

        let reply = self
            .surface_api
            .exchange_proto_star_message(proto, ReplyKind::Response, "StarlaneApi: create_resource")
            .await?;

        match reply{
            Reply::Record(record) => Ok(record),
            _ => unimplemented!("StarlaneApi::create_resource() did not receive the expected reply from surface_api")
        }
    }

    pub async fn star_search(
        &self,
        star_pattern: StarPattern
    ) -> Result<SearchHits, Error> {

        let hits = self.surface_api.star_search(star_pattern).await?;
        Ok(hits)
    }


    pub async fn watch(
        &self,
        selector: WatchResourceSelector,
    ) -> Result<Watcher, Error> {
        self.surface_api.watch( selector ).await
    }


    pub async fn create_api<API>(&self, create: Create ) -> Result<API, Error>
    where
        API: TryFrom<ResourceApi>,
    {
        let resource_api = ResourceApi {
            stub: self.create(create).await?.stub,
            surface_api: self.surface_api.clone(),
        };

        let api = API::try_from(resource_api);

        match api {
            Ok(api) => Ok(api),
            Err(error) => Err("catastrophic conversion error when attempting to try_convert api".into()),
        }
    }

    pub async fn create_space(&self, name: &str, title: &str) -> Result<Creation<SpaceApi>, Error> {
        let address_template = AddressTemplate{
            parent: Address::root(),
            child_segment_template: AddressSegmentTemplate::Exact(name.to_string())
        };

        let kind_template = KindTemplate {
            resource_type: "Space".to_string(),
            kind: None,
            specific: None
        };

        let template = Template::new(address_template,kind_template );
        let mut properties = PayloadMap::new();
        properties.insert( "title".to_string(), Payload::Primitive(Primitive::Text(title.to_string())));
        let create = Create {
            template,
            state: StateSrc::Stateless,
            properties,
            strategy: Strategy::Create,
            registry: Default::default()
        };

        let creation = Creation::new( self.clone(), create );
        Ok(creation)
    }


    pub async fn get_space(&self, address: Address) -> Result<SpaceApi, Error> {
        let record = self.fetch_resource_record(address).await?;
        Ok(SpaceApi::new(self.surface_api.clone(), record.stub)?)
    }

    pub async fn get_state( &self, address: Address ) -> Result<Payload,Error> {
        let mut request = ProtoRequest::new();
        request.to( address );
        request.from( MessageFrom::Inject );
        request.entity(ReqEntity::Rc(Rc::new(RcCommand::Get )));
        let response = self.surface_api.exchange(request).await?;
        Ok(response.entity.ok_or()?)
    }
}

pub struct SpaceApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl SpaceApi {

    pub fn address(&self) -> Address {
        self.stub.address.clone()
    }

    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.kind.resource_type() != ResourceType::Space.to_string() {
            return Err(format!(
                "wrong kind resource type for SpaceApi: {}",
                stub.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(SpaceApi { stub, surface_api })
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone())
    }

}




pub struct Creation<API>
where
    API: TryFrom<ResourceApi>,
{
    api: StarlaneApi,
    create: Create,
    phantom: PhantomData<API>,
}

impl<API> Creation<API>
where
    API: TryFrom<ResourceApi>,
{
    pub fn new(api: StarlaneApi, create: Create ) -> Self {
        Self {
            api,
            create,
            phantom: PhantomData {},
        }
    }

    pub async fn submit(self) -> Result<API, Error> {
        self.api.create_api(self.create).await
    }

    pub fn set_strategy(&mut self, strategy: Strategy) {
        self.create.strategy = strategy;
    }

    pub fn set_state(&mut self, payload: Payload ) {
        self.create.state = StateSrc::StatefulDirect(payload);
    }

    pub fn set_label(&mut self, key: &str) {
        let key = key.to_string();
        self.create.registry.push(SetLabel::Set(key));
    }

    pub fn set_label_with_value(&mut self, key: &str, value: &str) {
        let key = key.to_string();
        let value = value.to_string();
        self.create.registry.push(SetLabel::SetValue {key,value} );
    }

    pub fn set_property( &mut self, key:&str, value: Payload ) {
       self.create.properties.insert(key.to_string(), value );
    }
}

pub struct ResourceApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}




impl TryFrom<ResourceApi> for SpaceApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}


#[derive(Debug)]
pub enum StarlaneAction {
    GetState {
        address: Address,
        tx: tokio::sync::oneshot::Sender<Result<Payload, Error>>,
    },
}

pub struct StarlaneApiRunner {
    api: StarlaneApi,
    rx: tokio::sync::mpsc::Receiver<StarlaneAction>,
}

impl StarlaneApiRunner {
    pub fn new(api: StarlaneApi) -> tokio::sync::mpsc::Sender<StarlaneAction> {
        let (tx, rx) = tokio::sync::mpsc::channel(16);

        let runner = StarlaneApiRunner { api: api, rx: rx };

        runner.run();

        tx
    }

    fn run(mut self) {
        thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                while let Option::Some(action) = self.rx.recv().await {
                    self.process(action).await;
                }
            });
        });
    }

    async fn process(&self, action: StarlaneAction) {
        match action {
            StarlaneAction::GetState { address: identifier, tx } => {
                tx.send(self.api.get_state(identifier).await );
            }
        }
    }
}

#[derive(Clone)]
pub struct StarlaneApiRelay {
    tx: tokio::sync::mpsc::Sender<StarlaneAction>,
}

impl StarlaneApiRelay {
    /*
    pub async fn get_resource_state(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<DataSet<BinSrc>, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(StarlaneAction::GetState {
                address: identifier,
                tx: tx,
            })
            .await;
        rx.await?
    }
     */
}

impl Into<StarlaneApiRelay> for StarlaneApi {
    fn into(self) -> StarlaneApiRelay {
        StarlaneApiRelay {
            tx: StarlaneApiRunner::new(self),
        }
    }
}
