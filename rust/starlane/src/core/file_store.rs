use std::collections::HashSet;
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;
use std::str::FromStr;
use std::sync::Arc;
use std::path::PathBuf;

use rusqlite::Connection;
use tokio::sync::mpsc;

use crate::core::Host;
use crate::error::Error;
use crate::file::{FileAccess, FileEvent};
use crate::keys::{ResourceKey, FileSystemKey};
use crate::message::Fail;
use crate::resource::{AssignResourceStateSrc, DataTransfer, MemoryDataTransfer, Path, Resource, ResourceAddress, ResourceAssign, ResourceStateSrc, ResourceType, ResourceCreationChamber, FileKind, ResourceStub, ResourceCreate, ResourceArchetype, ResourceKind, AddressCreationSrc, KeyCreationSrc};
use crate::resource::store::{ResourceStore, ResourceStoreAction, ResourceStoreCommand, ResourceStoreResult};
use crate::star::StarSkel;

use std::fs;
use crate::util;

pub struct FileStoreHost {
    skel: StarSkel,
    file_access: FileAccess,
    store: ResourceStore,
}

impl FileStoreHost {
    pub async fn new(skel: StarSkel, file_access: FileAccess)->Result<Self,Error>{

        let mut file_access = file_access.with_path( "filesystems".to_string() ).await?;
        let rtn = FileStoreHost {
            skel: skel,
            file_access: file_access,
            store: ResourceStore::new().await,
        };

        rtn.watch().await?;

        Ok(rtn)
    }

    async fn watch(&self) -> Result<(),Error>{
        let mut event_rx = self.file_access.watch().await?;
        let store = self.store.clone();
        let dir = PathBuf::from(self.file_access.path());
        let root_path = fs::canonicalize(&dir)?.to_str().ok_or("turning path to string")?.to_string();
        let store = self.store.clone();
        let skel = self.skel.clone();
        tokio::spawn( async move {
            while let Option::Some(event) = event_rx.recv().await {
                Self::handle_event(root_path.clone(), event, store.clone(), skel.clone()).await.unwrap();
            }
        } );
        Ok(())
    }

    async fn handle_event(  root_path: String, event: FileEvent, store: ResourceStore, skel: StarSkel ) -> Result<(),Error>{

        println!("event path {}", event.path );
        println!("root  path {}", root_path );
        let mut path = event.path.clone();
        for _ in 0..root_path.len() {
            path.remove(0);
        }

        println!("chomped path {}", path );

        // remove leading / for filesystem
        path.remove(0);
        let mut split = path.split("/");
        let filesystem = split.next().ok_or("expected at least one directory for the filesystem")?;
        println!("filesystem: {}",filesystem);
        let mut file_path = String::new();
        for part in split {
            file_path.push_str("/");
            file_path.push_str(part);
        }
        println!("file: {}", file_path );

        println!("REceived Event: {:?}",event );
        let filesystem_key = ResourceKey::FileSystem(FileSystemKey::from_str(filesystem )?);

        FileStoreHost::ensure_file(filesystem_key, file_path,  event.file_kind, store, skel ).await?;

        Ok(())
        // first get filesystem
    }

    async fn ensure_file(filesystem_key: ResourceKey, file_path: String, kind: FileKind, store: ResourceStore, skel: StarSkel ) -> Result<(),Error> {


println!("ENSURING FILE...");
        let filesystem= store.get(filesystem_key.clone()).await?.ok_or(format!("expected filesystem to be present in hosted environment: {}",filesystem_key.to_string()))?;
println!("...did it work?");
        let filesystem: ResourceStub = filesystem.into();

        let archetype = ResourceArchetype{
            kind: ResourceKind::File(kind),
            specific: None,
            config: None
        };

        let create = ResourceCreate{
            key: KeyCreationSrc::None,
            parent: filesystem.key.clone(),
            archetype: archetype,
            address: AddressCreationSrc::Append(file_path),
            src: AssignResourceStateSrc::Hosted,
            registry_info: Option::None,
            owner: Option::None,
        };

        let rx = ResourceCreationChamber::new(filesystem, create,skel.clone() ).await;

        let x = util::wait_for_it_whatever(rx).await??;
        Ok(())
    }
}



#[async_trait]
impl Host for FileStoreHost {

    async fn assign(&mut self, assign: ResourceAssign<AssignResourceStateSrc>) -> Result<Resource, Fail> {
println!("$$$$ FILE RESOURCE ASSIGN: {}", assign.stub.archetype.kind );
        // if there is Initialization to do for assignment THIS is where we do it
       let data_transfer= match assign.state_src{
            AssignResourceStateSrc::Direct(data) => {
                let data_transfer:Arc<dyn DataTransfer> = Arc::new(MemoryDataTransfer::new(data));
                data_transfer
            },
            AssignResourceStateSrc::Hosted => {
                Arc::new(MemoryDataTransfer::none())
            }
        };

        match assign.stub.key.resource_type(){
            ResourceType::FileSystem => {
                // here we just ensure that a directory exists for the filesystem
                if let ResourceKey::FileSystem(filesystem_key) = &assign.stub.key {
                    let path = Path::new(format!("/{}",filesystem_key.to_string().as_str()).as_str() )?;
                    self.file_access.mkdir(&path).await?;
                }
            }
            ResourceType::File => {
                // we don't do anything for File yet...

            }
            rt => {
                return Err(Fail::WrongResourceType { expected: HashSet::from_iter(vec![ResourceType::FileSystem,ResourceType::File]), received: rt });
            }
        }

        let assign = ResourceAssign{
            stub: assign.stub,
            state_src: data_transfer
        };

        Ok(self.store.put( assign ).await?)
    }

    async fn get(&self, key: ResourceKey) -> Result<Option<Resource>, Fail> {
        self.store.get(key).await
    }

}