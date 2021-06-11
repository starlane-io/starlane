use std::convert::TryInto;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};

use crate::frame::{ChildManagerResourceAction, Reply, SimpleReply, StarMessagePayload};
use crate::keys::ResourceKey;
use crate::message::{Fail, ProtoStarMessage};
use crate::resource::{AddressCreationSrc, AssignResourceStateSrc, KeyCreationSrc, ResourceAddress, ResourceArchetype, ResourceCreate, ResourceKind, ResourceRecord, ResourceType, Path, LocalDataSrc, DataTransfer, ResourceIdentifier, ResourceStub };
use crate::resource::space::SpaceState;
use crate::resource::sub_space::SubSpaceState;
use crate::resource::user::UserState;
use crate::star::{Request, StarCommand, StarKey};
use crate::error::Error;
use crate::resource::file_system::FileSystemState;
use std::sync::Arc;


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

    pub async fn create_resource( &self, create: ResourceCreate ) -> Result<ResourceStub,Fail> {
        let parent_location = match &create.parent{
            ResourceKey::Nothing => {
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

    pub async fn create_space( &self, name: &str, display: &str )-> Result<SpaceApi,Fail> {
        let state= SpaceState::new(name.clone(), display);
        let state_data =  state.try_into()?;
        let resource_src = AssignResourceStateSrc::Direct(state_data);
        let create = ResourceCreate{
            parent: ResourceKey::Nothing,
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Space(name.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::Space,
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None
        };
        let stub = self.create_resource(create).await?;
        Ok(SpaceApi::new( self.star_tx.clone(), stub)?)
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

    pub async fn create_user( &self, email: &str )-> Result<UserApi,Fail> {
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
            owner: None
        };
        let stub = self.starlane_api().create_resource(create).await?;
        Ok(UserApi::new( self.star_tx.clone(), stub)?)
    }

    pub async fn create_sub_space( &self, sub_space: &str )-> Result<SubSpaceApi,Fail> {
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
            owner: None
        };
        let stub = self.starlane_api().create_resource(create).await?;
        Ok(SubSpaceApi::new( self.star_tx.clone(), stub)?)
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

    pub async fn create_file_system( &self, name: &str)-> Result<FileSystemApi,Fail> {
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
            owner: None
        };
        let stub = self.starlane_api().create_resource(create).await?;
        Ok(FileSystemApi::new(self.star_tx.clone(),stub)?)
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


    /*
    pub async fn create_file( &self, path: &Path, data: Box<dyn DataTransfer> )-> Result<FileApi,Fail> {
        // at this time the only way to 'create' a file state is to load the entire thing into memory
        // in the future we want options like "Stream" which will allow us to stream the state contents, etc.
//        let resource_src = AssignResourceStateSrc::Direct(data.get()?);
        let resource_src = AssignResourceStateSrc::Direct(Arc::new(vec!()));
        let create = ResourceCreate{
            parent: self.stub.key.clone(),
            key: KeyCreationSrc::None,
            address: AddressCreationSrc::Append(path.to_string()),
            archetype: ResourceArchetype {
                kind: ResourceKind::FileSystem,
                specific: None,
                config: None
            },
            src: resource_src,
            registry_info: None,
            owner: None
        };
        let stub = self.starlane_api().create_resource(create).await?;
        Ok(FileApi::new( self.star_tx.clone(), stub )?)
    }

     */

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
