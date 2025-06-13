use crate::err::StarErr;
use starlane_hyperspace::err::HyperErr;
use starlane_hyperspace::platform::Platform;
use starlane_space::substance::Substance;
use itertools::Itertools;
use std::path::PathBuf;

#[async_trait]
pub trait FileStore<K>
where
    K: Clone + Sized,
    Self: Clone + Sized,
{
    async fn get<'a>(&'a self, point: &'a K) -> Result<Substance, StarErr>;
    async fn insert<'a>(&'a self, point: &'a K, substance: Substance) -> Result<(), StarErr>;

    async fn mkdir<'a>(&'a self, point: &'a K) -> Result<(), StarErr>;
    async fn remove<'a>(&'a self, point: &'a K) -> Result<(), StarErr>;

    async fn list<'a>(&'a self, point: &'a K) -> Result<Vec<K>, StarErr>;

    async fn child<F, S>(&self, seg: S) -> Result<F, StarErr>
    where
        F: FileStore<K>,
        S: ToString;
}

#[derive(Clone)]
pub struct LocalFileStore
{
    pub root: PathBuf,
}

impl LocalFileStore
{
    pub fn new<B>(root: B) -> Self
    where
        B: Into<PathBuf>,
    {
        let root = root.into();
        Self {
            root,
        }
    }
}

#[async_trait]
impl FileStore<PathBuf> for LocalFileStore

{
    async fn get<'a>(&'a self, path: &'a PathBuf) -> Result<Substance, StarErr> {
        let path = self.root.join(path);
        let data = tokio::fs::read(path).await?;
        Ok(Substance::from_vec(data))
    }

    async fn insert<'a>(&'a self, path: &'a PathBuf, substance: Substance) -> Result<(), StarErr> {
        let path = self.root.join(path);
        let data = substance.to_bin()?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }

    async fn mkdir<'a>(&'a self, path: &'a PathBuf) -> Result<(), StarErr> {
        let path = self.root.join(path);
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn remove<'a>(&'a self, path: &'a PathBuf) -> Result<(), StarErr> {
        let path = self.root.join(path);
        tokio::fs::remove_dir_all(path).await?;
        Ok(())
    }

    async fn list<'a>(&'a self, path: &'a PathBuf) -> Result<Vec<PathBuf>, StarErr> {
        let path = self.root.join(path);
        let mut read = tokio::fs::read_dir(path).await?;
        let mut rtn = vec![];
        while let Some(entry) = read.next_entry().await? {
            rtn.push(entry.path());
        }
        Ok(rtn)
    }

    async fn child<F, S>(&self, seg: S) -> Result<F, StarErr>
    where
        F: FileStore<PathBuf>,
        S: ToString,
    {
        if Path::new(&seg).iter().count() != 1 {
            return Result::Err(StarErr::new(format!(
                "invalid child path segment: '{}' ... Child path can only be one path segment",
                seg.to_string()
            )));
        }
        self.mkdir(&seg).await?;
        let root = self.root.join(seg);
        Ok(Self::new(root))
    }
}
