use crate::hyper::space::err::HyperErr;
use crate::hyper::space::platform::Platform;
use itertools::Itertools;
use starlane_space::substance::Substance;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};



#[async_trait]
pub trait FileStore<K>
where
    Self::Err: HyperErr,
    K: Clone + Sized,
    Self: Clone + Sized,
{
    type Err;
    async fn get<'a>(&'a self, point: &'a K) -> Result<Substance, Self::Err>;
    async fn insert<'a>(&'a self, point: &'a K, substance: Substance) -> Result<(), Self::Err>;

    async fn mkdir<'a>(&'a self, point: &'a K) -> Result<(), Self::Err>;
    async fn remove<'a>(&'a self, point: &'a K) -> Result<(), Self::Err>;

    async fn list<'a>(&'a self, point: &'a K) -> Result<Vec<K>, Self::Err>;

    async fn child<F, S>(&self, seg: S) -> Result<F, Self::Err>
    where
        F: FileStore<K,FileStore::Err=Self::Err>,
        S: ToString;
}

#[derive(Clone)]
pub struct LocalFileStore<E>
{
    pub root: PathBuf,
    phantom: PhantomData<E>,
}

impl<P> LocalFileStore<P>
where
    P: Platform,
{
    pub fn new<B>(root: B) -> Self
    where
        B: Into<PathBuf>
    {
        let root = root.into();
        Self {
            root,
            phantom: Default::default(),
        }
    }
}

#[async_trait]
impl<E> FileStore<PathBuf> for LocalFileStore<E>
where E: HyperErr,
    Self::Err: HyperErr,

{
    type Err = E;
    async fn get<'a>(&'a self, path: &'a PathBuf) -> Result<Substance, Self::Err> {
        let path = self.root.join(path);
        let data = tokio::fs::read(path).await?;
        Ok(Substance::from_vec(data))
    }

    async fn insert<'a>(&'a self, path: &'a PathBuf, substance: Substance) -> Result<(), Self::Err> {
        let path = self.root.join(path);
        let data = substance.to_bin()?;
        tokio::fs::write(path, data).await?;
        Ok(())
    }

    async fn mkdir<'a>(&'a self, path: &'a PathBuf) -> Result<(), Self::Err> {
        let path = self.root.join(path);
        tokio::fs::create_dir_all(path).await?;
        Ok(())
    }

    async fn remove<'a>(&'a self, path: &'a PathBuf) -> Result<(), Self::Err> {
        let path = self.root.join(path);
        tokio::fs::remove_dir_all(path).await?;
        Ok(())
    }

    async fn list<'a>(&'a self, path: &'a PathBuf) -> Result<Vec<PathBuf>, Self::Err> {
        let path = self.root.join(path);
        let mut read = tokio::fs::read_dir(path).await?;
        let mut rtn = vec![];
        while let Some(entry) = read.next_entry().await? {
            rtn.push(entry.path());
        }
        Ok(rtn)
    }

    async fn child<F, S>(&self, seg: S) -> Result<F, Self::Err>
    where
        F: FileStore<PathBuf>,
        S: ToString,
    {
        if Path::new(&seg).iter().count() != 1 {
            return Result::Err(Self::Err::new(format!(
                "invalid child path segment: '{}' ... Child path can only be one path segment",
                seg.to_string()
            )));
        }
        self.mkdir(&seg).await?;
        let root = self.root.join(seg);
        Ok(Self::new(root))
    }
}
