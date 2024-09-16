use crate::host::err::Err;
use async_trait::async_trait;
use std::path::Path;
use std::process;
use tokio::fs;

#[async_trait]
pub trait Source: Send + Sync {
    async fn get(&self, key: &str) -> Result<Vec<u8>, Err>;
}

pub struct FileSystemSrc {
    root: String,
}

#[async_trait]
impl Source for FileSystemSrc {
    async fn get(&self, path: &str) -> Result<Vec<u8>, Err> {
        let parent = Path::new(self.root.as_str());
        let path = parent.join(Path::new(path));
        fs::read(path).await.map_err(|e| e.into())
    }
}

impl FileSystemSrc {
    pub fn new<S>(path: S) -> Self
    where
        S: ToString,
    {
        Self {
            root: path.to_string(),
        }
    }
}
