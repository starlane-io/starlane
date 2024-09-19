use crate::hyper::space::err::HyperErr;
use crate::hyper::space::Cosmos;
use itertools::Itertools;
use starlane_space::point::{Point, PointSeg};
use starlane_space::substance::Substance;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

#[async_trait]
pub trait FileStore<P, K>
where
    P: Cosmos,
    Self: Clone,
{
    async fn get(&self, point: &K) -> Result<Substance, P::Err>;
    async fn insert(&self, point: &K, substance: Substance) -> Result<(), P::Err>;

    async fn mkdir(&self, point: &K) -> Result<(), P::Err>;
    async fn remove(&self, point: &K) -> Result<(), P::Err>;

    async fn list(&self, point: &K) -> Result<Vec<K>, P::Err>;

    async fn child<F, S>(&self, seg: S) -> Result<F, P::Err>
    where
        F: FileStore<P, K>,
        S: ToString;
}

#[derive(Clone)]
pub struct LocalFileStore<P>
where
    P: Cosmos,
{
    pub root: PathBuf,
}

impl<P> LocalFileStore<P>
where
    P: Cosmos,
{
    pub fn new<B>(root: B) -> Self
    where
        B: Into<PathBuf>,
    {
        let root = root.into();
        Self { root }
    }
}

impl<P> FileStore<P, PathBuf> for LocalFileStore<P>
where
    P: Cosmos,
{
    async fn get(&self, path: &PathBuf) -> Result<Substance, P::Err> {
        let path = self.root.join(path);
        let data = tokio::fs::read(path).await?;
        Ok(Substance::from_vec(data))
    }

    async fn insert(&self, path: &PathBuf, substance: Substance) -> Result<(), P::Err> {
        let path = self.root.join(path);
        let data = substance.to_bin()?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }

    async fn mkdir(&self, path: &PathBuf) -> Result<(), P::Err> {
        let path = self.root.join(path);
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn remove(&self, path: &PathBuf) -> Result<(), P::Err> {
        let path = self.root.join(path);
        tokio::fs::remove_dir_all(path).await?;
        Ok(())
    }

    async fn list(&self, path: &PathBuf) -> Result<Vec<PathBuf>, P::Err> {
        let path = self.root.join(path);
        let mut read = tokio::fs::read_dir(path).await?;
        let mut rtn = vec![];
        while let Some(entry) = read.next_entry().await? {
            rtn.push(entry.path());
        }
        Ok(rtn)
    }

    async fn child<F, S>(&self, seg: S) -> Result<F, P::Err>
    where
        F: FileStore<P, PathBuf>,
        S: ToString,
    {
        if Path::new(&seg).iter().count() != 1 {
            return Result::Err(P::Err::new(format!(
                "invalid child path segment: '{}' ... Child path can only be one path segment",
                seg.to_string()
            )));
        }
        self.mkdir(&seg).await?;
        let root = self.root.join(seg);
        Ok(Self::new(root))
    }
}
