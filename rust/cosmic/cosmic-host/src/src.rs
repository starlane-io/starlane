use crate::err::Err;
use async_trait::async_trait;
use std::path::Path;
use tokio::fs;


#[async_trait]
pub trait WasmSource: Send + Sync {
    async fn get(&self, key: &str) -> Result<Vec<u8>, Err>;
}


pub struct FileSystemSrc {
    path: String
}




#[async_trait]
impl WasmSource for FileSystemSrc {

    async fn get(&self, path: &str) -> Result<Vec<u8>, Err> {
       let parent = Path::new(self.path.as_str());
        let path = parent.join(Path::new(path));
        fs::read(path).await.map_err(|e| e.into())
    }
}

impl  FileSystemSrc {
   pub fn new(path: String) -> Self {
      Self { path }
   }
}
