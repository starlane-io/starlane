use std::collections::HashSet;

use std::fmt;
use std::fmt::{Debug, Formatter};
use std::fs;
use std::iter::FromIterator;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;


use tokio::sync::{Mutex};

use starlane_resources::ResourceIdentifier;

use crate::star::core::component::resource::host::Host;
use crate::error::Error;
use crate::file_access::{FileAccess, FileEvent};
use crate::message::Fail;
use crate::resource::{
    AddressCreationSrc, AssignResourceStateSrc, FileKind,
    FileSystemKey, KeyCreationSrc, Path, RemoteDataSrc, Resource,
    ResourceAddress, ResourceArchetype, ResourceAssign, ResourceCreate,
    ResourceCreateStrategy, ResourceCreationChamber, ResourceKind, ResourceStub,
    ResourceType
};
use crate::resource::ResourceKey;
use crate::resource::state_store::{
    StateStore,
};
use crate::star::StarSkel;
use crate::util;
use std::convert::TryInto;
use crate::data::{DataSet, BinSrc};

pub struct FileStoreHost {
    skel: StarSkel,
    file_access: FileAccess,
    store: StateStore,
    mutex: Arc<Mutex<u8>>,
}

impl Debug for FileStoreHost {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(format!("FileStoreHost: {:?}", self.skel).as_str() )
    }
}

impl FileStoreHost {
    pub async fn new(skel: StarSkel, file_access: FileAccess) -> Result<Self, Error> {
        let file_access = file_access.with_path("filesystems".to_string())?;
        let rtn = FileStoreHost {
            skel: skel.clone(),
            file_access: file_access,
            store: StateStore::new(skel).await,
            mutex: Arc::new(Mutex::new(0)),
        };

        rtn.walk().await?;

        //rtn.watch().await?;

        Ok(rtn)
    }

    async fn walk(&self) -> Result<(), Error> {
        unimplemented!()
        /*

        let mut event_rx = self.file_access.walk().await?;
        let dir = PathBuf::from(self.file_access.path());
        let root_path = fs::canonicalize(&dir)?
            .to_str()
            .ok_or("turning path to string")?
            .to_string();
        let store = self.store.clone();
        let skel = self.skel.clone();
        tokio::spawn(async move {
            while let Option::Some(event) = event_rx.recv().await {
                match Self::handle_event(
                    root_path.clone(),
                    event.clone(),
                    store.clone(),
                    skel.clone(),
                )
                .await
                {
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!(
                            "WALK: error when handling path: {} error: {} ",
                            event.path,
                            error.to_string()
                        );
                    }
                }
            }
        });
        Ok(())
         */
    }

    #[instrument]
    async fn watch(&self) -> Result<(), Error> {
        unimplemented!()
        /*
        let mut event_rx = self.file_access.watch().await?;
        let dir = PathBuf::from(self.file_access.path());
        let root_path = fs::canonicalize(&dir)?
            .to_str()
            .ok_or("turning path to string")?
            .to_string();
        let store = self.store.clone();
        let skel = self.skel.clone();
        let mutex = self.mutex.clone();
        tokio::spawn(async move {
            while let Option::Some(event) = event_rx.recv().await {
                let _lock = mutex.lock().await;
                match Self::handle_event(
                    root_path.clone(),
                    event.clone(),
                    store.clone(),
                    skel.clone(),
                )
                .await
                {
                    Ok(_) => {}
                    Err(error) => {
                        error!(
                            "WATCH: error when handling path: {} error: {} ",
                            event.path,
                            error.to_string()
                        );
                    }
                }
            }
        });
        Ok(())

         */
    }

    /*
    async fn handle_event(
        root_path: String,
        event: FileEvent,
        store: StateStore,
        skel: StarSkel,
    ) -> Result<(), Error> {
        let mut path = event.path.clone();
        for _ in 0..root_path.len() {
            path.remove(0);
        }

        if path.len() == 0 {
            return Ok(());
        }
        // remove leading / for filesystem
        path.remove(0);
        let mut split = path.split("/");
        let filesystem = split
            .next()
            .ok_or("expected at least one directory for the filesystem")?;
        let mut file_path = String::new();
        for part in split {
            file_path.push_str("/");
            file_path.push_str(part);
        }

        if event.file_kind == FileKind::Directory {
            file_path.push_str("/");
        }

        let filesystem_key = ResourceKey::FileSystem(FileSystemKey::from_str(filesystem)?);

        FileStoreHost::ensure_file(filesystem_key, file_path, event.file_kind, store, skel).await?;

        Ok(())
        // first get filesystem
    }

    async fn ensure_file(
        filesystem_key: ResourceKey,
        file_path: String,
        kind: FileKind,
        store: StateStore,
        skel: StarSkel,
    ) -> Result<(), Error> {
        let filesystem = store
            .get(filesystem_key.clone().into())
            .await?;
        let filesystem: ResourceStub = filesystem.into();

        let archetype = ResourceArchetype {
            kind: ResourceKind::File(kind),
            specific: None,
            config: None,
        };

        let create = ResourceCreate {
            key: KeyCreationSrc::None,
            parent: filesystem.key.clone().into(),
            archetype: archetype,
            address: AddressCreationSrc::Append(file_path),
            state_src: AssignResourceStateSrc::AlreadyHosted,
            registry_info: Option::None,
            owner: Option::None,
            strategy: ResourceCreateStrategy::Ensure,
        };

        let rx = ResourceCreationChamber::new(filesystem, create, skel.clone()).await;

        let _x = util::wait_for_it_whatever(rx).await??;
        Ok(())
    }
     */
}


#[async_trait]
impl Host for FileStoreHost {
    async fn assign(
        &mut self,
        assign: ResourceAssign<AssignResourceStateSrc<DataSet<BinSrc>>>,
    ) -> Result<Resource, Fail> {
        // if there is Initialization to do for assignment THIS is where we do it

        match assign.stub.key.resource_type() {
            ResourceType::FileSystem => {
                // here we just ensure that a directory exists for the filesystem
                if let ResourceKey::FileSystem(filesystem_key) = &assign.stub.key {
                    let path =
                        Path::from_str(format!("/{}", filesystem_key.to_string().as_str()).as_str())?;
                    self.file_access.mkdir(&path).await?;
                }
            }
            ResourceType::File => {
                match assign.state_src {
                    AssignResourceStateSrc::Direct(data) => {
                        let filesystem_key= assign
                            .stub
                            .key
                            .ancestor_of_type(ResourceType::FileSystem)?;
                        let filesystem_path = Path::from_str(
                            format!("/{}", filesystem_key.to_string().as_str()).as_str(),
                        )?;
                        let path = format!(
                            "{}{}",
                            filesystem_path.to_string(),
                            assign.stub.address.last_to_string()
                        );

                        let _lock = self.mutex.lock().await;
                        let content = data.get("content").cloned().ok_or("expected file content")?;
                        let content = content.to_bin(self.skel.machine.bin_context())?;
                        self.file_access
                            .write(&Path::from_str(path.as_str())?, content)
                            .await?;
                    }
                    AssignResourceStateSrc::AlreadyHosted => {
                        // do nothing, the file should already be present in the filesystem detected by the watcher and
                        // this call to assign is just making sure the database registry is updated
                    }
                    AssignResourceStateSrc::None => {
                        // do nothing, there is no data (this should never happen of course in a file)
                    }
                    AssignResourceStateSrc::CreateArgs(_) => {
                        return Err("File cannot be created with CreateArgs".into());
                        // cannot create with init_args
                    }

                }
            }
            rt => {
                return Err(Fail::WrongResourceType {
                    expected: HashSet::from_iter(vec![
                        ResourceType::FileSystem,
                        ResourceType::File,
                    ]),
                    received: rt,
                });
            }
        }

        let state = DataSet::new();

        let assign = ResourceAssign {
            stub: assign.stub,
            state_src: state,
        };

        Ok(self.store.put(assign).await?)
    }

    async fn get(&self, key: ResourceKey ) -> Result<DataSet<BinSrc>, Fail> {
        self.store.get(key).await
    }

    /*
    async fn state(&self, key: ResourceKey ) -> Result<DataSet<BinSrc>, Fail> {
        if let Ok(Option::Some(resource)) = self.store.get(key.clone()).await {
            match key.resource_type() {
                ResourceType::File => {
                    let filesystem_key = resource
                        .key()
                        .ancestor_of_type(ResourceType::FileSystem)?;
                    let filesystem_path =
                        Path::from_str(format!("/{}", filesystem_key.to_string().as_str()).as_str())?;
                    let path = format!(
                        "{}{}",
                        filesystem_path.to_string(),
                        resource.address().last_to_string()
                    );
                    let data = self
                        .file_access
                        .read(&Path::from_str(path.as_str())?)
                        .await?;
                    let mut state = DataSet::new();
                    state.insert("content".to_string(), BinSrc::Memory(data));
                    Ok(state)
                }
                _ => Ok(DataSet::new()),
            }
        } else {
            Err(Fail::ResourceNotFound(key.into()))
        }
    }

     */

    async fn delete(&self, _identifier: ResourceKey ) -> Result<(), Fail> {
        unimplemented!()
    }
}
