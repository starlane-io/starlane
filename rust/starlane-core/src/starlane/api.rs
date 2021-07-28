use std::{thread};
use std::convert::{TryFrom, TryInto};
use std::marker::PhantomData;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;



use semver::Version;



use tokio::runtime::{Handle};
use tokio::sync::mpsc;

use starlane_resources::ResourceIdentifier;

use crate::cache::ProtoArtifactCachesFactory;
use crate::error::Error;
use crate::frame::{StarPattern, WindAction};

use crate::message::{Fail};
use crate::message::resource::{
    MessageFrom, ProtoMessage, ResourceRequestMessage, ResourceResponseMessage,
};
use crate::resource::{AddressCreationSrc, ArtifactBundleKind, AssignResourceStateSrc, DataTransfer, FieldSelection, KeyCreationSrc, LocalStateSetSrc, Path, RemoteDataSrc, ResourceAddress, ResourceArchetype, ResourceCreate, ResourceCreateStrategy, ResourceKind, ResourceRecord, ResourceRegistryInfo, ResourceSelector, ResourceStub, ResourceType, ArtifactBundlePath};

use crate::resource::ArtifactBundleAddress;

use crate::resource::file_system::FileSystemState;
use crate::resource::FileKind;
use crate::resource::ResourceKey;
use crate::resource::space::SpaceState;
use crate::resource::sub_space::SubSpaceState;
use crate::resource::user::UserState;
use crate::star::{Request, StarCommand, StarKind, Wind};

use crate::starlane::StarlaneCommand;

#[derive(Clone)]
pub struct StarlaneApi {
    star_tx: mpsc::Sender<StarCommand>,
    starlane_tx: Option<mpsc::Sender<StarlaneCommand>>
}

impl StarlaneApi {
    pub async fn create_artifact_bundle(&self, path: &ArtifactBundlePath, data: Arc<Vec<u8>>) -> Result<ArtifactBundleApi,Fail> {
        let address: ResourceAddress = path.clone().into();

        let subspace_address = address.parent().ok_or("expected parent")?.parent().ok_or("expected parent")?;
        let subspace_api= self.get_sub_space(subspace_address.into()).await?;

        let mut creation = subspace_api.create_artifact_bundle_versions(address.parent().unwrap().name().as_str())?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        let artifact_bundle_versions_api = creation.submit().await?;

        let version = semver::Version::from_str( address.name().as_str() )?;
        let mut creation = artifact_bundle_versions_api.create_artifact_bundle(version, data )?;
        creation.set_strategy(ResourceCreateStrategy::Ensure);
        creation.submit().await
    }
}

impl StarlaneApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>) -> Self {
        Self {
            star_tx,
            starlane_tx: Option::None
        }
    }

    pub fn with_starlane_ctrl(star_tx: mpsc::Sender<StarCommand>, starlane_tx: mpsc::Sender<StarlaneCommand>) -> Self {
        Self {
            star_tx,
            starlane_tx: Option::Some(starlane_tx)
        }
    }

    pub async fn to_key( &self, identifier: ResourceIdentifier ) -> Result<ResourceKey,Fail> {
        match identifier {
            ResourceIdentifier::Key(key) => Ok(key),
            ResourceIdentifier::Address(address) => {
                self.fetch_resource_key(address).await
            }
        }
    }

    pub fn shutdown(&self) -> Result<(),Error>{
        self.starlane_tx.as_ref().ok_or("this api does not have access to the StarlaneMachine and therefore cannot do a shutdown")?.try_send(StarlaneCommand::Shutdown);
        Ok(())
    }

    pub async fn timeout<T>(
        rx: tokio::sync::oneshot::Receiver<Result<T, Fail>>,
    ) -> Result<T, Fail> {
        match tokio::time::timeout(Duration::from_secs(15), rx).await {
            Ok(result) => match result {
                Ok(result) => result,
                Err(_err) => Err(Fail::ChannelRecvErr),
            },
            Err(err) => {
                println!("elapsed error: {}", err);
                Err(Fail::Timeout)
            }
        }
    }

    pub async fn ping_gateway(&self) -> Result<(),Fail> {

        let (wind,gateway_search_rx) = Wind::new(StarPattern::StarKind(StarKind::Gateway), WindAction::SearchHits);
        self.star_tx.send( StarCommand::WindInit(wind)).await;

        let result = tokio::time::timeout( Duration::from_secs(5), gateway_search_rx ).await;
        result??;
        Ok(())
    }

    pub async fn fetch_resource_address(&self, key: ResourceKey) -> Result<ResourceAddress, Fail> {
        match self.fetch_resource_record(key.into()).await {
            Ok(record) => Ok(record.stub.address),
            Err(fail) => Err(fail),
        }
    }

    pub async fn fetch_resource_key(&self, address: ResourceAddress) -> Result<ResourceKey, Fail> {
        match self.fetch_resource_record(address.into()).await {
            Ok(record) => Ok(record.stub.key),
            Err(fail) => Err(fail),
        }
    }

    pub async fn fetch_resource_record(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<ResourceRecord, Fail> {
        let (request, rx) = Request::new(identifier);
        self.star_tx.send(StarCommand::ResourceRecordRequest(request)).await;
        Self::timeout(rx).await
    }

    pub async fn get_caches(&self) -> Result<Arc<ProtoArtifactCachesFactory>, Fail> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.star_tx.send(StarCommand::GetCaches(tx)).await;
        Ok(rx.await?)
    }

    /*
    pub async fn get_child_resource_manager(&self, key: ResourceKey ) -> Result<ChildResourceManager,Fail> {
        let (request,rx)  = Request::new(key);
        self.star_tx.send( StarCommand::GetResourceManager(request)).await;
        Ok(rx.await??)
    }

     */

    pub async fn create_resource( &self, create: ResourceCreate ) -> Result<ResourceRecord,Fail> {
        let create = create.to_keyed(self.clone() ).await?;

        let mut proto = ProtoMessage::new();
        proto.to( create.parent.clone().into() );
        proto.from( MessageFrom::Inject );
        proto.payload = Option::Some(ResourceRequestMessage::Create(create));
        let reply = proto.reply();
        let proto = proto.to_proto_star_message().await?;
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await?;

        let result = Self::timeout(reply).await?;

        match result.payload{
            ResourceResponseMessage::Resource(Option::Some(resource)) => {
                Ok(resource)
            }
            _ => Err(Fail::expected("ResourceResponseMessage::Resource(Option::Some(resource))"))
        }
    }

    pub async fn select(&self, parent_resource: &ResourceIdentifier, mut selector: ResourceSelector ) -> Result<Vec<ResourceRecord>,Fail> {
        let resource = parent_resource.clone();

        selector.add_field( FieldSelection::Parent(resource.clone()));

        // before sending
        let selector = selector.to_keyed(self.clone()).await?;

        let mut proto = ProtoMessage::new();
        proto.to( resource );
        proto.from( MessageFrom::Inject );
        proto.payload = Option::Some(ResourceRequestMessage::Select(selector));
        let reply = proto.reply();
        let proto = proto.to_proto_star_message().await?;
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await?;

        let result = Self::timeout(reply).await?;

        match result.payload{
            ResourceResponseMessage::Resources(resources) => {
                Ok(resources)
            }
            _ => Err(Fail::expected("ResourceResponseMessage::Resource(Option::Some(resource))"))
        }
    }

    pub async fn list( &self, identifier: &ResourceIdentifier ) -> Result<Vec<ResourceRecord>,Fail> {
        let selector = ResourceSelector::new();
        self.select(identifier, selector).await
    }


    /*
    pub async fn create_resource(&self, create: ResourceCreate) -> Result<ResourceStub, Fail> {
        let parent_location = match &create.parent {
            ResourceKey::Root => ResourceRecord::new(ResourceStub::nothing(), StarKey::central()),
            _ => {
                let (request, rx) = Request::new(create.parent.clone().into());
                self.star_tx
                    .send(StarCommand::ResourceRecordRequest(request))
                    .await;
                StarlaneApi::timeout(rx).await?
            }
        };

        let mut proto = ProtoStarMessage::new();
        proto.to(parent_location.location.host.into());
        proto.payload =
            StarMessagePayload::ResourceManager(ChildManagerResourceAction::Create(create));
        let result = proto.get_ok_result().await;
        self.star_tx
            .send(StarCommand::SendProtoMessage(proto))
            .await;
        match result.await? {
            StarMessagePayload::Reply(SimpleReply::Ok(Reply::Resource(record))) => Ok(record.stub),
            StarMessagePayload::Reply(SimpleReply::Fail(fail)) => Err(fail),
            payload => {
                println!("create_resource: unexpected payload: {}", payload);
                Err(Fail::Error(
                    format!("create_resource: unexpected payload: {}", payload).into(),
                ))
            }
        }
    }
    */

    pub async fn create_api<API>(&self, create: ResourceCreate) -> Result<API, Fail>
    where
        API: TryFrom<ResourceApi>,
    {
        let resource_api = ResourceApi {
            stub: self.create_resource(create).await?.stub,
            star_tx: self.star_tx.clone(),
        };

        let api = API::try_from(resource_api);

        match api {
            Ok(api) => Ok(api),
            Err(_error) => Err(Fail::Error("catastrophic converstion error".into())),
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

    pub fn get_resource_state(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<Option<Arc<Vec<u8>>>, Fail> {
        println!("get_resource_state block_on");
        let state_src = self.get_resource_state_src(identifier)?;
        match state_src {
            RemoteDataSrc::None => Ok(Option::None),
            RemoteDataSrc::Memory(data) => Ok(Option::Some(data)),
        }
    }

    pub fn get_resource_state_src(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<RemoteDataSrc, Fail> {
        let star_tx = self.star_tx.clone();

        let handle = Handle::current();
        handle.block_on(async {
            let mut proto = ProtoMessage::new();
            proto.payload = Option::Some(ResourceRequestMessage::State);
            proto.to = Option::Some(identifier);
            proto.from = Option::Some(MessageFrom::Inject);
            let reply = proto.reply();
            let proto = proto.to_proto_star_message().await?;
            star_tx.send(StarCommand::SendProtoMessage(proto)).await?;

            let result = Self::timeout(reply).await?;

            match result.payload {
                ResourceResponseMessage::State(data) => Ok(data),
                _ => Err(Fail::expected("ResourceResponseMessage::State(data)")),
            }
        })
    }

    pub fn create_space(&self, name: &str, display: &str) -> Result<Creation<SpaceApi>, Fail> {
        let state = SpaceState::new(display);
        let state_data = state.try_into()?;
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate {
            parent: ResourceKey::Root.into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Space(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Space,
                specific: None,
                config: None,
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.clone(), create))
    }

    pub async fn get_space(&self, identifier: ResourceIdentifier) -> Result<SpaceApi, Fail> {
        let record = self.fetch_resource_record(identifier).await?;
        Ok(SpaceApi::new(self.star_tx.clone(), record.stub)?)
    }

    pub async fn get_sub_space(&self, identifier: ResourceIdentifier) -> Result<SubSpaceApi, Fail> {
        let record = self.fetch_resource_record(identifier).await?;
        Ok(SubSpaceApi::new(self.star_tx.clone(), record.stub)?)
    }
}

pub struct SpaceApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl SpaceApi {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.stub.address.clone()
    }

    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::Space {
            return Err(format!(
                "wrong key resource type for SpaceApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.address.resource_type() != ResourceType::Space {
            return Err(format!(
                "wrong address resource type for SpaceApi: {}",
                stub.address.resource_type().to_string()
            )
            .into());
        }

        Ok(SpaceApi {
            stub: stub,
            star_tx: star_tx,
        })
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.star_tx.clone() )
    }

    pub fn create_user(&self, email: &str) -> Result<Creation<UserApi>, Fail> {
        let state = UserState::new(email.to_string());
        let state_data = state.try_into()?;
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
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn create_sub_space(&self, sub_space: &str) -> Result<Creation<SubSpaceApi>, Fail> {
        let state = SubSpaceState::new(sub_space);
        let state_data = state.try_into()?;
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
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn create_domain(&self, domain: &str) -> Result<Creation<DomainApi>, Fail> {
        let resource_src = AssignResourceStateSrc::None;
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(domain.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Domain,
                specific: None,
                config: None,
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.starlane_api(), create))
    }
}

pub struct SubSpaceApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl SubSpaceApi {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.stub.address.clone()
    }

    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::SubSpace {
            return Err(format!(
                "wrong key resource type for SubSpaceApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.address.resource_type() != ResourceType::SubSpace {
            return Err(format!(
                "wrong address resource type for SubSpaceApi: {}",
                stub.address.resource_type().to_string()
            )
            .into());
        }

        Ok(SubSpaceApi {
            stub: stub,
            star_tx: star_tx,
        })
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.star_tx.clone())
    }

    pub fn create_file_system(&self, name: &str) -> Result<Creation<FileSystemApi>, Fail> {
        let state = FileSystemState::new();
        let state_data = state.try_into()?;
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::FileSystem,
                specific: None,
                config: None,
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn create_artifact_bundle_versions(
        &self,
        name: &str,
    ) -> Result<Creation<ArtifactBundleVersionsApi>, Fail> {
        let resource_src = AssignResourceStateSrc::None;

        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::ArtifactBundleVersions,
                specific: None,
                config: None,
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.starlane_api(), create))
    }
}

pub struct FileSystemApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl FileSystemApi {
    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.stub.address.clone()
    }

    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::FileSystem {
            return Err(format!(
                "wrong key resource type for FileSystemApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.address.resource_type() != ResourceType::FileSystem {
            return Err(format!(
                "wrong address resource type for FileSystemApi: {}",
                stub.address.resource_type().to_string()
            )
            .into());
        }

        Ok(FileSystemApi {
            stub: stub,
            star_tx: star_tx,
        })
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.star_tx.clone() )
    }

    pub fn create_file_from_string(
        &self,
        path: &Path,
        string: String,
    ) -> Result<Creation<FileApi>, Fail> {
        self.create_file(path, Arc::new(string.into_bytes()))
    }

    pub fn create_file(&self, path: &Path, data: Arc<Vec<u8>>) -> Result<Creation<FileApi>, Fail> {
        // at this time the only way to 'create' a file state is to load the entire thing into memory
        // in the future we want options like "Stream" which will allow us to stream the state contents, etc.
        //        let resource_src = AssignResourceStateSrc::Direct(data.get()?);
        let resource_src = AssignResourceStateSrc::Direct(data);
        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(path.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::File(FileKind::File),
                specific: None,
                config: None,
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.starlane_api(), create))
    }
}

pub struct FileApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl FileApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::File {
            return Err(format!(
                "wrong key resource type for FileApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.address.resource_type() != ResourceType::File {
            return Err(format!(
                "wrong address resource type for FileApi: {}",
                stub.address.resource_type().to_string()
            )
            .into());
        }

        Ok(FileApi {
            star_tx: star_tx,
            stub: stub,
        })
    }
}

pub struct ArtifactBundleVersionsApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl ArtifactBundleVersionsApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::ArtifactBundleVersions{
            return Err(format!(
                "wrong key resource type for ArtifactVersionsBundleApi: {}",
                stub.key.resource_type().to_string()
            )
                .into());
        }
        if stub.address.resource_type() != ResourceType::ArtifactBundleVersions{
            return Err(format!(
                "wrong address resource type for ArtifactBundleVersionsApi: {}",
                stub.address.resource_type().to_string()
            )
                .into());
        }

        Ok(Self{
            star_tx: star_tx,
            stub: stub,
        })
    }

    pub fn create_artifact_bundle(
        &self,
        version: Version,
        data: Arc<Vec<u8>>,
    ) -> Result<Creation<ArtifactBundleApi>, Fail> {
        let resource_src = AssignResourceStateSrc::Direct(data);
        // hacked to FINAL
        let kind: ArtifactBundleKind = ArtifactBundleKind::Final;

        let create = ResourceCreate {
            parent: self.stub.key.clone().into(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(version.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::ArtifactBundle(kind),
                specific: None,
                config: None,
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create,
        };
        Ok(Creation::new(self.starlane_api(), create))
    }

    pub fn starlane_api(&self) -> StarlaneApi {
        StarlaneApi::new(self.star_tx.clone() )
    }

}

pub struct ArtifactBundleApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl ArtifactBundleApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::ArtifactBundle {
            return Err(format!(
                "wrong key resource type for ArtifactBundleApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.address.resource_type() != ResourceType::ArtifactBundle {
            return Err(format!(
                "wrong address resource type for ArtifactBundleApi: {}",
                stub.address.resource_type().to_string()
            )
            .into());
        }

        Ok(ArtifactBundleApi {
            star_tx: star_tx,
            stub: stub,
        })
    }
}
pub struct UserApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl UserApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::User {
            return Err(format!(
                "wrong key resource type for UserApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.address.resource_type() != ResourceType::User {
            return Err(format!(
                "wrong address resource type for UserApi: {}",
                stub.address.resource_type().to_string()
            )
            .into());
        }

        Ok(UserApi {
            star_tx: star_tx,
            stub: stub,
        })
    }
}

pub struct DomainApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>,
}

impl DomainApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub) -> Result<Self, Error> {
        if stub.key.resource_type() != ResourceType::Domain {
            return Err(format!(
                "wrong key resource type for DomainApi: {}",
                stub.key.resource_type().to_string()
            )
            .into());
        }
        if stub.address.resource_type() != ResourceType::Domain {
            return Err(format!(
                "wrong address resource type for DomainApi: {}",
                stub.address.resource_type().to_string()
            )
            .into());
        }

        Ok(DomainApi {
            star_tx: star_tx,
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

    pub async fn submit(self) -> Result<API, Fail> {
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
    star_tx: mpsc::Sender<StarCommand>,
}

impl TryFrom<ResourceApi> for FileSystemApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for FileApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for ArtifactBundleApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for ArtifactBundleVersionsApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}


impl TryFrom<ResourceApi> for SubSpaceApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for SpaceApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for UserApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}

impl TryFrom<ResourceApi> for DomainApi {
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new(value.star_tx, value.stub)?)
    }
}

#[derive(Debug)]
pub enum StarlaneAction {
    GetState {
        identifier: ResourceIdentifier,
        tx: tokio::sync::oneshot::Sender<Result<Option<Arc<Vec<u8>>>, Fail>>,
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
                tx.send(self.api.get_resource_state(identifier));
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
    ) -> Result<Option<Arc<Vec<u8>>>, Fail> {
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
