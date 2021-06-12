use std::convert::{TryInto, TryFrom};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use crate::frame::{ChildManagerResourceAction, Reply, SimpleReply, StarMessagePayload};
use crate::keys::ResourceKey;
use crate::message::{Fail, ProtoStarMessage};
use crate::resource::{AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, ResourceAddress, ResourceArchetype, ResourceCreate, ResourceKind, ResourceRecord, ResourceType, Path, LocalDataSrc, DataTransfer, ResourceIdentifier, ResourceStub, ResourceCreateStrategy, FileKind, ResourceRegistryInfo};
use crate::resource::space::SpaceState;
use crate::resource::sub_space::SubSpaceState;
use crate::resource::user::UserState;
use crate::star::{Request, StarCommand, StarKey};
use crate::error::Error;
use crate::resource::file_system::FileSystemState;
use std::sync::Arc;
use crate::resource::domain::DomainState;
use crate::message::resource::{ProtoMessage, ResourceRequestMessage, MessageReply, ResourceResponseMessage};
use crate::star::StarCommand::ResourceRecordRequest;
use std::marker::PhantomData;
use crate::frame::ChildManagerResourceAction::Register;


#[derive(Clone)]
pub struct StarlaneApi {
    star_tx: mpsc::Sender<StarCommand>
}


impl StarlaneApi {
    pub fn new( star_tx: mpsc::Sender<StarCommand> ) -> Self {
        StarlaneApi {
            star_tx: star_tx
        }
    }

    pub async fn timeout<T>( rx: oneshot::Receiver<Result<T,Fail>>)->Result<T,Fail>{
        match tokio::time::timeout(Duration::from_secs(15),rx).await {
            Ok(result) => {
               match result {
                   Ok(result) => {result}
                   Err(err) => Err(Fail::ChannelRecvErr)
               }
            }
            Err(err) => {
                Err(Fail::Timeout)
            }
        }
    }

    pub async fn fetch_resource_address(&self, key: ResourceKey) -> Result<ResourceAddress,Fail> {
        match self.fetch_resource_record(key.into()).await
        {
            Ok(record) => Ok(record.stub.address),
            Err(fail) => Err(fail)
        }
    }

    pub async fn fetch_resource_key(&self, key: ResourceKey) -> Result<ResourceKey,Fail> {
        match self.fetch_resource_record(key.into()).await
        {
            Ok(record) => Ok(record.stub.key),
            Err(fail) => Err(fail)
        }
    }


    pub async fn fetch_resource_record(&self, identifier: ResourceIdentifier) -> Result<ResourceRecord,Fail> {
        let (request,rx)  = Request::new(identifier);
        self.star_tx.send( StarCommand::ResourceRecordRequest(request)).await;
        rx.await?
    }


    /*
    pub async fn get_child_resource_manager(&self, key: ResourceKey ) -> Result<ChildResourceManager,Fail> {
        let (request,rx)  = Request::new(key);
        self.star_tx.send( StarCommand::GetResourceManager(request)).await;
        Ok(rx.await??)
    }

     */


    /*
    pub async fn create_resource( &self, create: ResourceCreate ) -> Result<ResourceStub,Fail> {
        let mut proto = ProtoMessage::new();
        proto.to( create.parent.clone().into() );
        proto.payload = Option::Some(ResourceRequestMessage::Create(create));
        let reply = proto.reply();
        let proto = proto.to_proto_star_message().await?;
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await?;

        let result = reply.await??;

        match result.payload{
            ResourceResponseMessage::Resource(Option::Some(resource)) => {
                Ok(resource.stub)
            }
            _ => Err(Fail::Unexpected)
        }
    }

     */

    pub async fn create_api<API>( &self, create: ResourceCreate ) -> Result<API,Fail> where API: TryFrom<ResourceApi>{

        let resource_api = ResourceApi {
            stub: self.create_resource(create).await?,
            star_tx: self.star_tx.clone()
        };

        let api  = API::try_from(resource_api );

        match api {
            Ok(api) => {
                Ok(api)
            }
            Err(error) => {
                Err(Fail::Error("catastrophic converstion error".into()))
            }
        }
    }

    pub async fn create_resource( &self, create: ResourceCreate ) -> Result<ResourceStub,Fail> {
        let parent_location = match &create.parent{
            ResourceKey::Root => {
                ResourceRecord::new(ResourceStub::nothing(), StarKey::central() )
            }
            _ => {
                let (request,rx) = Request::new(create.parent.clone().into() );
                self.star_tx.send( StarCommand::ResourceRecordRequest(request)).await;
                StarlaneApi::timeout(rx).await?
            }
        };

        let mut proto = ProtoStarMessage::new();
        proto.to(parent_location.location.host.into());
        proto.payload = StarMessagePayload::ResourceManager(ChildManagerResourceAction::Create(create));
        let result = proto.get_ok_result().await;
        self.star_tx.send( StarCommand::SendProtoMessage(proto)).await;
        match result.await?
        {
            StarMessagePayload::Reply(SimpleReply::Ok(Reply::Resource(record))) => Ok(record.stub),
            StarMessagePayload::Reply(SimpleReply::Fail(fail)) => Err(fail),
            payload => {
                println!("create_resource: unexpected payload: {}",payload );
                Err(Fail::Error(format!("create_resource: unexpected payload: {}",payload ).into()))
            }
        }
    }


    pub fn create_space( &self, name: &str, display: &str )-> Result<Creation<SpaceApi>,Fail> {
        let state= SpaceState::new(name.clone(), display);
        let state_data =  state.try_into()?;
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate{
            parent: ResourceKey::Root,
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Space(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Space,
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create
        };
        Ok(Creation::new( self.clone(), create ))
    }

    pub async fn get_space( &self, identifier: ResourceIdentifier ) -> Result<SpaceApi,Fail> {
        let record = self.fetch_resource_record(identifier).await?;
        Ok(SpaceApi::new( self.star_tx.clone(), record.stub)?)
    }

    pub async fn get_sub_space( &self, identifier: ResourceIdentifier ) -> Result<SubSpaceApi,Fail> {
        let record = self.fetch_resource_record(identifier).await?;
        Ok(SubSpaceApi::new( self.star_tx.clone(), record.stub)?)
    }
}

pub struct SpaceApi{
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>
}

impl SpaceApi {

    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.stub.address.clone()
    }

    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub ) -> Result<Self,Error> {
        if stub.key.resource_type() != ResourceType::Space{
            return Err(format!("wrong key resource type for SpaceApi: {}", stub.key.resource_type().to_string()).into());
        }
        if stub.address.resource_type() != ResourceType::Space{
            return Err(format!("wrong address resource type for SpaceApi: {}", stub.address.resource_type().to_string()).into());
        }

        Ok(SpaceApi{
            stub: stub,
            star_tx: star_tx,
        })
    }

    pub fn starlane_api( &self )-> StarlaneApi {
        StarlaneApi::new(self.star_tx.clone())
    }

    pub fn create_user( &self, email: &str )-> Result<Creation<UserApi>,Fail> {
        let state = UserState::new(email.to_string() );
        let state_data =  state.try_into()?;
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate{
            parent: self.stub.key.clone(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(email.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::User,
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create
        };
        Ok(Creation::new( self.starlane_api(), create ))
    }

    pub fn create_sub_space( &self, sub_space: &str )-> Result<Creation<SubSpaceApi>,Fail> {
        let state = SubSpaceState::new(sub_space);
        let state_data =  state.try_into()?;
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate{
            parent: self.stub.key.clone(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(sub_space.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::SubSpace,
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create
        };
        Ok(Creation::new( self.starlane_api(), create ))
    }

    pub fn create_domain( &self, domain: &str )-> Result<Creation<DomainApi>,Fail> {
        let resource_src = AssignResourceStateSrc::None;
        let create = ResourceCreate{
            parent: self.stub.key.clone(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(domain.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Domain,
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create
        };
        Ok(Creation::new( self.starlane_api(), create ))
    }
}


pub struct SubSpaceApi{
    stub : ResourceStub,
    star_tx: mpsc::Sender<StarCommand>
}

impl SubSpaceApi {

    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.stub.address.clone()
    }


    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub ) -> Result<Self,Error> {
        if stub.key.resource_type() != ResourceType::SubSpace{
            return Err(format!("wrong key resource type for SubSpaceApi: {}", stub.key.resource_type().to_string()).into());
        }
        if stub.address.resource_type() != ResourceType::SubSpace{
            return Err(format!("wrong address resource type for SubSpaceApi: {}", stub.address.resource_type().to_string()).into());
        }

        Ok(SubSpaceApi{
            stub: stub,
            star_tx: star_tx,
        })
    }

    pub fn starlane_api( &self )-> StarlaneApi {
        StarlaneApi::new(self.star_tx.clone())
    }

    pub fn create_file_system( &self, name: &str )-> Result<Creation<FileSystemApi>,Fail> {
        let state = FileSystemState::new();
        let state_data =  state.try_into()?;
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate{
            parent: self.stub.key.clone(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::FileSystem,
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create
        };
        Ok(Creation::new(self.starlane_api(),create))
    }

}

pub struct FileSystemApi{
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>
}

impl FileSystemApi {

    pub fn key(&self) -> ResourceKey {
        self.stub.key.clone()
    }

    pub fn address(&self) -> ResourceAddress {
        self.stub.address.clone()
    }


    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub ) -> Result<Self,Error> {
        if stub.key.resource_type() != ResourceType::FileSystem{
            return Err(format!("wrong key resource type for FileSystemApi: {}", stub.key.resource_type().to_string()).into());
        }
        if stub.address.resource_type() != ResourceType::FileSystem{
            return Err(format!("wrong address resource type for FileSystemApi: {}", stub.address.resource_type().to_string()).into());
        }

        Ok(FileSystemApi{
            stub: stub,
            star_tx: star_tx,
        })
    }

    pub fn starlane_api( &self )-> StarlaneApi {
        StarlaneApi::new(self.star_tx.clone())
    }

    pub fn create_file_from_string( &self, path: &Path, string: String )-> Result<Creation<FileApi>,Fail> {
        self.create_file( path, Arc::new(string.into_bytes()) )
    }

    pub fn create_file( &self, path: &Path, data: Arc<Vec<u8>> )-> Result<Creation<FileApi>,Fail> {
        // at this time the only way to 'create' a file state is to load the entire thing into memory
        // in the future we want options like "Stream" which will allow us to stream the state contents, etc.
//        let resource_src = AssignResourceStateSrc::Direct(data.get()?);
        let resource_src = AssignResourceStateSrc::Direct(data);
        let create = ResourceCreate{
            parent: self.stub.key.clone(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(path.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::File(FileKind::File),
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None,
            strategy: ResourceCreateStrategy::Create
        };
        Ok(Creation::new(self.starlane_api(),create))
    }

}

pub struct FileApi{
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>
}

impl FileApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub ) -> Result<Self,Error> {
        if stub.key.resource_type() != ResourceType::File{
            return Err(format!("wrong key resource type for FileApi: {}", stub.key.resource_type().to_string()).into());
        }
        if stub.address.resource_type() != ResourceType::File{
            return Err(format!("wrong address resource type for FileApi: {}", stub.address.resource_type().to_string()).into());
        }

        Ok(FileApi{
            star_tx: star_tx,
            stub: stub
        })
    }
}


pub struct UserApi{
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>
}

impl UserApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub ) -> Result<Self,Error> {
        if stub.key.resource_type() != ResourceType::User{
            return Err(format!("wrong key resource type for UserApi: {}", stub.key.resource_type().to_string()).into());
        }
        if stub.address.resource_type() != ResourceType::User{
            return Err(format!("wrong address resource type for UserApi: {}", stub.address.resource_type().to_string()).into());
        }

        Ok(UserApi{
            star_tx: star_tx,
            stub: stub
        })
    }
}


pub struct DomainApi{
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>
}

impl DomainApi {
    pub fn new(star_tx: mpsc::Sender<StarCommand>, stub: ResourceStub ) -> Result<Self,Error> {
        if stub.key.resource_type() != ResourceType::Domain{
            return Err(format!("wrong key resource type for DomainApi: {}", stub.key.resource_type().to_string()).into());
        }
        if stub.address.resource_type() != ResourceType::Domain{
            return Err(format!("wrong address resource type for DomainApi: {}", stub.address.resource_type().to_string()).into());
        }

        Ok(DomainApi{
            star_tx: star_tx,
            stub: stub
        })
    }
}


pub struct Creation<API> where API: TryFrom<ResourceApi> {
    api: StarlaneApi,
    create: ResourceCreate,
    phantom: PhantomData<API>
}

impl <API> Creation <API> where API: TryFrom<ResourceApi> {

    pub fn new( api: StarlaneApi, create: ResourceCreate ) -> Self {
        Self{
            api: api,
            create: create,
            phantom: PhantomData{}
        }
    }

    pub async fn submit(self) -> Result<API,Fail> {
        self.api.create_api(self.create).await
    }

    fn registry_info(&mut self) -> &mut ResourceRegistryInfo {

        if self.create.registry_info.is_none(){
            self.create.registry_info = Option::Some( ResourceRegistryInfo::new() );
        }
        self.create.registry_info.as_mut().unwrap()
    }

    pub fn set_strategy( &mut self, strategy: ResourceCreateStrategy ) {
        self.create.strategy = strategy;
    }

    pub fn add_tag( &mut self, tag: String ) {
        self.registry_info().names.push(tag);
    }

    pub fn add_label( &mut self, key: String, value: String ) {
        self.registry_info().labels.insert( key, value );
    }
}

pub struct ResourceApi {
    stub: ResourceStub,
    star_tx: mpsc::Sender<StarCommand>
}

impl TryFrom<ResourceApi> for FileSystemApi{
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new( value.star_tx, value.stub )?)
    }
}

impl TryFrom<ResourceApi> for FileApi{
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new( value.star_tx, value.stub )?)
    }
}

impl TryFrom<ResourceApi> for SubSpaceApi{
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new( value.star_tx, value.stub )?)
    }
}

impl TryFrom<ResourceApi> for SpaceApi{
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new( value.star_tx, value.stub )?)
    }
}

impl TryFrom<ResourceApi> for UserApi{
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new( value.star_tx, value.stub )?)
    }
}

impl TryFrom<ResourceApi> for DomainApi{
    type Error = Fail;

    fn try_from(value: ResourceApi) -> Result<Self, Self::Error> {
        Ok(Self::new( value.star_tx, value.stub )?)
    }
}


