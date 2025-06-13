/// the [std] module and defines and sometimes provides a limited enumeration of standard
/// starlane services that are so essential to the operation of starlane itself it made sense
/// to embed them rather instead of installing extensions.
use crate::service::{FileStoreService, ServiceErr};
use std::path::PathBuf;

impl FileStoreService {
    pub async fn sub_root(&self, sub_root: PathBuf) -> Result<FileStoreService, ServiceErr> {
        let runner = self.runner.sub_root(sub_root).await?;
        Ok(FileStoreService {
            template: self.template.clone(),
            runner,
        })
    }
}
