use crate::hyper::space::Cosmos;
use itertools::Itertools;
use starlane_space::point::{Point, PointSeg};
use starlane_space::substance::Substance;
use std::marker::PhantomData;
use std::path::PathBuf;

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

    fn child<F, S>(&self, seg: S) -> Result<F, P::Err>
    where
        F: FileStore<P, K>,
        S: ToString;
}

#[derive(Clone)]
pub struct TruncParentFileStore<P, C>
where
    P: Cosmos,
    C: FileStore<P, PathBuf>,
{
    parent: Point,
    store: C,
    phantom: PhantomData<P>,
}

impl<P, C> TruncParentFileStore<P, C>
where
    P: Cosmos,
    C: FileStore<P, PathBuf>,
{
    pub fn new(parent: Point, store: C) -> Self {
        Self {
            parent,
            store,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<P, C> FileStore<P, Point> for TruncParentFileStore<P, C>
where
    P: Cosmos,
    C: FileStore<P, PathBuf>,
{
    async fn get(&self, point: &Point) -> Result<Substance, P::Err> {
        let path = point.truncate_filepath(&self.parent)?.try_into()?;
        self.store.get(&path).await
    }

    async fn insert(&self, point: &Point, substance: Substance) -> Result<(), P::Err> {
        let path = point.truncate_filepath(&self.parent)?.try_into()?;
        self.store.insert(&path, substance).await
    }

    async fn mkdir(&self, point: &Point) -> Result<(), P::Err> {
        let path = point.truncate_filepath(&self.parent)?.try_into()?;
        self.store.mkdir(&path).await
    }

    async fn remove(&self, point: &Point) -> Result<(), P::Err> {
        let path = point.truncate_filepath(&self.parent)?.try_into()?;
        self.store.remove(&path).await
    }

    async fn list(&self, point: &Point) -> Result<Vec<Point>, P::Err> {
        let path = point.truncate_filepath(&self.parent).try_into()?;
        let rtn = self
            .store
            .list(&path)
            .await?
            .into_iter()
            .map(|p| self.parent.push(p.to_str()).unwrap())
            .collect();

        Ok(rtn)
    }

    fn child<F, S>(&self, seg: S) -> Result<F, P::Err>
    where
        F: FileStore<P, Point>,
        S: ToString,
    {
        let parent = self.parent.push(seg.to_string())?;
        let store = self.store.child(seg)?;

        Ok(Self::new(parent, store))
    }
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

    fn child<F, S>(&self, seg: S) -> Result<F, P::Err>
    where
        F: FileStore<P, PathBuf>,
        S: ToString,
    {
        let root = self.root.join(seg);
        Ok(Self::new(root))
    }
}
