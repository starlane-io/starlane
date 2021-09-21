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

use starlane_resources::{AddressCreationSrc, AssignResourceStateSrc, FieldSelection, KeyCreationSrc, LocalStateSetSrc, RemoteDataSrc, ResourceArchetype, ResourceCreate, ResourceCreateStrategy, ResourceIdentifier, ResourceRegistryInfo, ResourceSelector, ResourceStub, ResourcePath};
use starlane_resources::ConfigSrc;
use starlane_resources::data::{BinSrc, DataSet, Meta};
use starlane_resources::data::Binary;
use starlane_resources::message::{MessageFrom, ProtoMessage, ResourceRequestMessage, ResourceResponseMessage, ResourcePortMessage, Message, MessageReply};
use starlane_resources::message::Fail;

use crate::cache::ProtoArtifactCachesFactory;
use crate::error::Error;
use crate::frame::{Reply, ReplyKind, StarPattern, TraversalAction};
use crate::resource::{Path, ResourceKind, ResourceRecord, ResourceType, to_keyed_for_reasource_create, to_keyed_for_resource_selector};
use crate::resource::file_system::FileSystemState;
use crate::resource::FileKind;
use crate::resource::ResourceKey;
use crate::resource::sub_space::SubSpaceState;
use crate::resource::user::UserState;
use crate::star::{Request, StarCommand, StarKind, StarSkel};
use crate::star::shell::search::SearchInit;
use crate::star::surface::SurfaceApi;
use crate::starlane::StarlaneCommand;
use starlane_resources::property::{ResourcePropertyValueSelector, DataSetAspectSelector, FieldValueSelector, ResourceValue, ResourceValueSelector, ResourceValues};
use crate::watch::{WatchResourceSelector, Watcher};

#[derive(Clone)]
pub struct StarlaneApi {
    surface_api: SurfaceApi,
    starlane_tx: Option<mpsc::Sender<StarlaneCommand>>,
}

impl StarlaneApi {
    pub async fn create_artifact_bundle(
        &self,
        path: &ResourcePath,
        data: Arc<Vec<u8>>,
    ) -> Result<ArtifactBundleApi, Error> {
        let address: ResourcePath = path.clone().into();

        let subspace_address = address
            .parent()
            .ok_or("expected parent")?
            .parent()
            .ok_or("expected parent")?;
        let subspace_api = self.get_sub_space(subspace_address.into()).await?;

        let mut creation = subspace_api
            .create_artifact_bundle_versions(address.parent().unwrap().name().as_str())?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        let artifact_bundle_versions_api = creation.submit().await?;

        let version = semver::Version::from_str(address.name().as_str())?;
        let mut creation = artifact_bundle_versions_api.create_artifact_bundle(version, data)?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await
    }
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

    pub async fn to_key(&self, identifier: ResourceIdentifier) -> Result<ResourceKey, Error> {
        match identifier {
            ResourceIdentifier::Key(key) => Ok(key),
            ResourceIdentifier::Address(address) => self.fetch_resource_key(address).await,
        }
    }

    pub fn shutdown(&self) -> Result<(), Error> {
        self.starlane_tx.as_ref().ok_or("this api does not have access to the StarlaneMachine and therefore cannot do a shutdown")?.try_send(StarlaneCommand::Shutdown);
        Ok(())
    }

    pub async fn send( &self, message: Message<ResourcePortMessage>, description: &str ) -> Result<Reply,Error> {
        let proto = message.try_into()?;
info!("staring message exchange for {}",description);
        let reply = self.surface_api.exchange(proto, ReplyKind::Port, description ).await?;
info!("received reply for {}",description);

        if ReplyKind::Port.is_match(&reply) {
            Ok(reply)
        } else {
            Err(format!("unexpected reply: {}", reply.to_string()).into())
        }


    }


    pub async fn timeout<T>(
        rx: tokio::sync::oneshot::Receiver<Result<T, Error>>,
    ) -> Result<T, Error> {
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(result) => match result {
                Ok(result) => result,
                Err(_err) => Err(Fail::ChannelRecvErr.into()),
            },
            Err(err) => {
                eprintln!("elapsed error: {}", err);
                Err(Fail::Timeout.into())
            }
        }
    }

    /*
    pub async fn ping_gateway(&self) -> Result<(),Fail> {

        let (wind,gateway_search_rx) = Wind::new(StarPattern::StarKind(StarKind::Gateway), WindAction::SearchHits);
        self.surface_api.send( StarCommand::WindInit(wind)).await;

        let result = tokio::time::timeout( Duration::from_secs(5), gateway_search_rx ).await;
        result??;
        Ok(())
    }
     */

    pub async fn fetch_resource_address(&self, key: ResourceKey) -> Result<ResourcePath, Error> {
        match self.fetch_resource_record(key.into()).await {
            Ok(record) => Ok(record.stub.address),
            Err(fail) => Err(fail.into()),
        }
    }

    pub async fn fetch_resource_key(&self, address: ResourcePath ) -> Result<ResourceKey, Error> {
        match self.fetch_resource_record(address.into()).await {
            Ok(record) => Ok(record.stub.key),
            Err(fail) => Err(fail.into()),
        }
    }

    pub async fn fetch_resource_record(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<ResourceRecord, Error> {
        self.surface_api.locate(identifier).await
    }

    pub async fn get_caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Error> {
        Ok(self.surface_api.get_caches().await?)
    }

    /*
    pub async fn get_child_resource_manager(&self, key: ResourceKey ) -> Result<ChildResourceManager,Fail> {
        let (request,rx)  = Request::new(key);
        self.surface_api.send( StarCommand::GetResourceManager(request)).await;
        Ok(rx.await??)
    }

     */

    pub async fn create_resource(&self, create: ResourceCreate) -> Result<ResourceRecord, Error> {
        let create = to_keyed_for_reasource_create(create, self.clone()).await?;


        let mut proto = ProtoMessage::new();
        proto.to(create.parent.clone().into());
        proto.from(MessageFrom::Inject);
        proto.payload = Option::Some(ResourceRequestMessage::Create(create));
        let proto = proto.try_into()?;

        let reply = self
            .surface_api
            .exchange(proto, ReplyKind::Record, "StarlaneApi: create_resource")
            .await?;

        match reply{
            Reply::Record(record) => Ok(record),
            _ => unimplemented!("StarlaneApi::create_resource() did not receive the expected reply from surface_api")
        }
    }

    pub async fn select(
        &self,
        parent_resource: &ResourceIdentifier,
        mut selector: ResourceSelector,
    ) -> Result<Vec<ResourceRecord>, Error> {
        let resource = parent_resource.clone();

        selector.add_field(FieldSelection::Parent(resource.clone()));

        // before sending
        let selector = to_keyed_for_resource_selector(selector,self.clone()).await?;

        let mut proto = ProtoMessage::new();
        proto.to(resource);
        proto.from(MessageFrom::Inject);
        proto.payload = Option::Some(ResourceRequestMessage::Select(selector));
        let proto = proto.try_into()?;

        let reply = self
            .surface_api
            .exchange(proto, ReplyKind::Records, "StarlaneApi: create_resource")
            .await?;

        match reply{
            Reply::Records(records) => Ok(records),
            _ => unimplemented!("StarlaneApi::create_resource() did not receive the expected reply from surface_api")
        }
    }


    pub async fn select_values(
        &self,
        path: ResourcePath,
        selector: ResourcePropertyValueSelector
    ) -> Result<ResourceValues<ResourceStub>, Error> {

        let mut proto = ProtoMessage::new();
        proto.to(path.into());
        proto.from(MessageFrom::Inject);
        proto.payload = Option::Some(ResourceRequestMessage::SelectValues(selector));
        let proto = proto.try_into()?;

        let reply = self
            .surface_api
            .exchange(proto, ReplyKind::ResourceValues, "StarlaneApi: select_values ")
            .await?;

        match reply{
            Reply::ResourceValues(values) => Ok(values),
            _ => unimplemented!("StarlaneApi::select_values() did not receive the expected reply from surface_api")
        }
    }


    pub async fn watch(
        &self,
        selector: WatchResourceSelector,
    ) -> Result<Watcher, Error> {
        self.surface_api.watch( selector ).await
    }


    pub async fn list(&self, identifier: &ResourceIdentifier) -> Result<Vec<ResourceRecord>, Error> {
        let selector = ResourceSelector::new();
        self.select(identifier, selector).await
    }

    pub async fn create_api<API>(&self, create: ResourceCreate) -> Result<API, Error>
    where
        API: TryFrom<ResourceApi>,
    {
        let resource_api = ResourceApi {
            stub: self.create_resource(create).await?.stub,
            surface_api: self.surface_api.clone(),
        };

        let api = API::try_from(resource_api);

        match api {
            Ok(api) => Ok(api),
            Err(error) => Err(Fail::Error(format!("catastrophic conversion error when attempting to try_convert api").into()).into()),
        }
    }

    /*
    /// this function is acting as a facade for now, later we will not download the entire state in one message
    pub async fn get_resource_state_stream(&self, identifier: ResourceIdentifier ) -> Result<Option<Box<dyn AsyncReadExt>>,Fail> {
        match self.get_resource_state(identifier).await? {
            None => Ok(Option::None),
            Some(data) => {
                let file_path= TempDir::new("sometempdir")?.path().with_file_name("temp.out");
                let mut file = File::create( file_path.as_path() ).await?;
                file.write_all(data.as_slice()).await?;
                let mut file = File::open( file_path.as_path() ).await?;
                Ok(Option::Some(Box::new(file)))
            }
        }
    }
     */

    pub async fn get_resource_state(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<DataSet<BinSrc>, Error> {
        let state_src = self.get_resource_state_src(identifier).await?;
        Ok(state_src)
    }

    pub async fn get_resource_state_src(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<DataSet<BinSrc>, Error> {
        let surface_api = self.surface_api.clone();

            let mut proto = ProtoMessage::new();
            let selector = ResourcePropertyValueSelector::State{
                aspect: DataSetAspectSelector::All,
                field: FieldValueSelector::All
            };
            proto.payload = Option::Some(ResourceRequestMessage::SelectValues(selector.clone()));
            proto.to = Option::Some(identifier);
            proto.from = Option::Some(MessageFrom::Inject);
            let proto = proto.try_into()?;
            let result = surface_api
                .exchange(
                    proto,
                    ReplyKind::ResourceValues,
                    "StarlaneApi::get_resource_state_src()",
                )
                .await;
            match result {
                Ok(Reply::ResourceValues(values)) => {
                   let state = values.values.get(&selector ).ok_or("expected state value")?.clone();
                   match state {
                       ResourceValue::DataSet(state) => {
                           Ok(state)
                       }
                       _ => {
                           Err("expected state to be a DataSet".into())
                       }
                   }
                },
                Err(fail) => Err(fail.into()),
                _ => unimplemented!("StarlaneApi::get_resource_state_src() IMPOSSIBLE!"),
            }

    }

    pub fn create_space(&self, name: &str, display_name: &str) -> Result<Creation<SpaceApi>, Error> {
        let mut meta = Meta::single("display-name", display_name);
        let mut state: DataSet<BinSrc> = DataSet::new();
        state.insert("meta".to_string(), meta.try_into()?);

        let state = AssignResourceStateSrc::Direct(state);
        let create = ResourceCreate {
            parent: ResourceKey::Root.into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Just(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Space,
                specific: None,
                config: None,
            },
            state_src: state,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.clone(), create))
    }

    pub fn create_domain(&self, domain: &str) -> Result<Creation<DomainApi>, Error> {
        let state= AssignResourceStateSrc::Stateless;
        let create = ResourceCreate {
            parent: ResourceKey::Root.into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Just(domain.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Domain,
                specific: None,
                config: None,
            },
            state_src: state,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.clone(), create))
    }

    pub async fn get_space(&self, identifier: ResourceIdentifier) -> Result<SpaceApi, Error> {
        let record = self.fetch_resource_record(identifier).await?;
        Ok(SpaceApi::new(self.surface_api.clone(), record.stub)?)
    }

    pub async fn get_sub_space(&self, identifier: ResourceIdentifier) -> Result<SubSpaceApi, Error> {
        let record = self.fetch_resource_record(identifier).await?;
        Ok(SubSpaceApi::new(self.surface_api.clone(), record.stub)?)
    }
}

pub struct SpaceApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl SpaceApi {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourcePath {
        self.stub.address.clone()
    }

    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::Space {
            return Err(format!(
                "wrong key resource type for SpaceApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::Space {
            return Err(format!(
                "wrong address resource type for SpaceApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(SpaceApi { stub, surface_api })
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone())
    }

    pub fn create_user(&self, email: &str) -> Result<Creation<UserApi>, Error> {
        let mut meta = Meta::single("email", email);
        let mut state_data: DataSet<BinSrc> = DataSet::new();
        state_data.insert("meta".to_string(), meta.try_into()?);
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(email.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::User,
                specific: None,
                config: None,
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn create_sub_space(
        &self,
        sub_space: &str,
        display_name: &str,
    ) -> Result<Creation<SubSpaceApi>, Error> {
        let mut meta = Meta::single("display-name", display_name);
        let mut state_data: DataSet<BinSrc> = DataSet::new();
        state_data.insert("meta".to_string(), meta.try_into()?);
        let resource_src = AssignResourceStateSrc::Direct(state_data);

        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(sub_space.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::SubSpace,
                specific: None,
                config: None,
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }


}

pub struct SubSpaceApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl SubSpaceApi {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourcePath {
        self.stub.address.clone()
    }

    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::SubSpace {
            return Err(format!(
                "wrong key resource type for SubSpaceApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::SubSpace {
            return Err(format!(
                "wrong address resource type for SubSpaceApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(SubSpaceApi {
            stub: stub,
            surface_api: surface_api,
        })
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone())
    }

    pub fn create_app(&self, name: &str, app_config: ResourcePath ) -> Result<Creation<AppApi>, Error> {
        let resource_src = AssignResourceStateSrc::Stateless;
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::App,
                specific: None,
                config: Option::Some(ConfigSrc::Artifact(app_config)),
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }


    pub fn create_file_system(&self, name: &str) -> Result<Creation<FileSystemApi>, Error> {
        let resource_src = AssignResourceStateSrc::Stateless;
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::FileSystem,
                specific: None,
                config: None,
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn create_artifact_bundle_versions(
        &self,
        name: &str,
    ) -> Result<Creation<ArtifactBundleSeriesApi>, Error> {
        let resource_src = AssignResourceStateSrc::Stateless;

        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::ArtifactBundleSeries,
                specific: None,
                config: None,
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }
}

pub struct AppApi{
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl AppApi {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourcePath {
        self.stub.address.clone()
    }

    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::App{
            return Err(format!(
                "wrong key resource type for AppApi: {}",
                stub.key.resource_type().to_string()
            )
                .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::App{
            return Err(format!(
                "wrong address resource type for AppApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
                .into());
        }

        Ok(AppApi{
            stub: stub,
            surface_api: surface_api,
        })
    }

    pub fn create_mechtron(&self, name: &str, config: ResourcePath ) -> Result<Creation<MechtronApi>, Error> {
        let resource_src = AssignResourceStateSrc::Stateless;
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Mechtron,
                specific: None,
                config: Option::Some(ConfigSrc::Artifact(config)),
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone())
    }
}

pub struct MechtronApi{
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl MechtronApi{
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourcePath {
        self.stub.address.clone()
    }

    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::App{
            return Err(format!(
                "wrong key resource type for AppApi: {}",
                stub.key.resource_type().to_string()
            )
                .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::App{
            return Err(format!(
                "wrong address resource type for AppApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
                .into());
        }

        Ok(MechtronApi{
            stub: stub,
            surface_api: surface_api,
        })
    }
    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone())
    }
}

pub struct FileSystemApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl FileSystemApi {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourcePath {
        self.stub.address.clone()
    }

    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::FileSystem {
            return Err(format!(
                "wrong key resource type for FileSystemApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::FileSystem {
            return Err(format!(
                "wrong address resource type for FileSystemApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(FileSystemApi {
            stub: stub,
            surface_api: surface_api,
        })
    }
    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone())
    }

    pub fn create_file_from_string(
        &self,
        path: &Path,
        string: String,
    ) -> Result<Creation<FileApi>, Error> {
        self.create_file(path, Arc::new(string.into_bytes()))
    }

    pub fn create_file(&self, path: &Path, data: Binary) -> Result<Creation<FileApi>, Error> {
        let content = BinSrc::Memory(data);
        let mut state: DataSet<BinSrc> = DataSet::new();
        state.insert("content".to_string(), content);

        // at this time the only way to 'create' a file state is to load the entire thing into memory
        // in the future we want options like "Stream" which will allow us to stream the state contents, etc.
        //        let resource_src = AssignResourceStateSrc::Direct(data.get()?);
        let resource_src = AssignResourceStateSrc::Direct(state);
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(path.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::File(FileKind::File),
                specific: None,
                config: None,
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }
}

pub struct FileApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl FileApi {
    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::File {
            return Err(format!(
                "wrong key resource type for FileApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::File {
            return Err(format!(
                "wrong address resource type for FileApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(FileApi {
            surface_api: surface_api,
            stub: stub,
        })
    }
}

pub struct ArtifactBundleSeriesApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl ArtifactBundleSeriesApi {
    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::ArtifactBundleSeries {
            return Err(format!(
                "wrong key resource type for ArtifactVersionsBundleApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::ArtifactBundleSeries {
            return Err(format!(
                "wrong address resource type for ArtifactBundleSeriesApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(Self {
            surface_api: surface_api,
            stub: stub,
        })
    }

    pub fn create_artifact_bundle(
        &self,
        version: Version,
        data: Arc<Vec<u8>>,
    ) -> Result<Creation<ArtifactBundleApi>, Fail> {
        let content = BinSrc::Memory(data);
        let mut state: DataSet<BinSrc> = DataSet::new();
        state.insert("zip".to_string(), content);

        let resource_src = AssignResourceStateSrc::Direct(state);
        // hacked to FINAL
//        let kind: ArtifactBundleKind = ArtifactBundleKind::Final;

        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(version.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::ArtifactBundle,
                specific: None,
                config: None,
            },
            state_src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
            from: MessageFrom::Inject
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.surface_api.clone())
    }
}

pub struct ArtifactBundleApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl ArtifactBundleApi {
    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::ArtifactBundle {
            return Err(format!(
                "wrong key resource type for ArtifactBundleApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::ArtifactBundle {
            return Err(format!(
                "wrong address resource type for ArtifactBundleApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(ArtifactBundleApi {
            surface_api: surface_api,
            stub: stub,
        })
    }
}
pub struct UserApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl UserApi {
    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::User {
            return Err(format!(
                "wrong key resource type for UserApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::User {
            return Err(format!(
                "wrong address resource type for UserApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(UserApi {
            surface_api: surface_api,
            stub: stub,
        })
    }
}

pub struct DomainApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl DomainApi {
    pub fn new(surface_api: SurfaceApi, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::Domain {
            return Err(format!(
                "wrong key resource type for DomainApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.archetype.kind.resource_type() != ResourceType::Domain {
            return Err(format!(
                "wrong address resource type for DomainApi: {}",
                stub.archetype.kind.resource_type().to_string()
            )
            .into());
        }

        Ok(DomainApi {
            surface_api: surface_api,
            stub: stub,
        })
    }
}

pub struct Creation<API>
where
    API: TryFrom<ResourceApi>,
{
    api: StarlaneApi,
    create: ResourceCreate,
    phantom: PhantomData<API>,
}

impl<API> Creation<API>
where
    API: TryFrom<ResourceApi>,
{
    pub fn new(api: StarlaneApi, create: ResourceCreate) -> Self {
        Self {
            api: api,
            create: create,
            phantom: PhantomData {},
        }
    }

    pub async fn submit(self) -> Result<API, Error> {
        self.api.create_api(self.create).await
    }

    fn registry_info(&mut self) -> &mut ResourceRegistryInfo {
        if self.create.registry_info.is_none() {
            self.create.registry_info = Option::Some(ResourceRegistryInfo::new());
        }
        self.create.registry_info.as_mut().unwrap()
    }

    pub fn set_strategy(&mut self, strategy: ResourceCreateStrategy) {
        self.create.strategy = strategy;
    }

    pub fn add_tag(&mut self, tag: String) {
        self.registry_info().names.push(tag);
    }

    pub fn add_label(&mut self, key: String, value: String) {
        self.registry_info().labels.insert(key, value);
    }
}

pub struct ResourceApi {
    stub: ResourceStub,
    surface_api: SurfaceApi,
}

impl TryFrom<ResourceApi> for FileSystemApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for FileApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for AppApi{
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for MechtronApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for ArtifactBundleApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for ArtifactBundleSeriesApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for SubSpaceApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for SpaceApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for UserApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for DomainApi {
    type Error = Error;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.surface_api, value.stub)?)
    }
}

#[derive(Debug)]
pub enum StarlaneAction {
    GetState {
        identifier: ResourceIdentifier,
        tx: tokio::sync::oneshot::Sender<Result<DataSet<BinSrc>, Error>>,
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
            StarlaneAction::GetState { identifier, tx } => {
                tx.send(self.api.get_resource_state(identifier).await );
            }
        }
    }
}

#[derive(Clone)]
pub struct StarlaneApiRelay {
    tx: tokio::sync::mpsc::Sender<StarlaneAction>,
}

impl StarlaneApiRelay {
    pub async fn get_resource_state(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<DataSet<BinSrc>, Error> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(StarlaneAction::GetState {
                identifier: identifier,
                tx: tx,
            })
            .await;
        rx.await?
    }
}

impl Into<StarlaneApiRelay> for StarlaneApi {
    fn into(self) -> StarlaneApiRelay {
        StarlaneApiRelay {
            tx: StarlaneApiRunner::new(self),
        }
    }
}
