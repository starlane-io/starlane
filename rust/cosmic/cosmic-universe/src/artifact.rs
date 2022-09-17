use std::sync::Arc;
use tokio::sync::RwLock;
use lru::LruCache;
use std::ops::Deref;
use crate::{BindConfig, Point, Stub, UniErr};
use crate::substance2::Bin;
use serde::{Serialize,Deserialize};

#[derive(Clone)]
pub struct ArtifactApi {
    binds: Arc<RwLock<LruCache<Point, Arc<BindConfig>>>>,
    fetcher: Arc<dyn ArtifactFetcher>,
}

impl ArtifactApi {
    pub fn new(fetcher: Arc<dyn ArtifactFetcher>) -> Self {
        Self {
            binds: Arc::new(RwLock::new(LruCache::new(1024))),
            fetcher,
        }
    }

    pub async fn bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, UniErr> {
        {
            let read = self.binds.read().await;
            if read.contains(point) {
                let mut write = self.binds.write().await;
                let bind = write.get(point).unwrap().clone();
                return Ok(ArtRef::new(bind, point.clone()));
            }
        }

        let bind: Arc<BindConfig> = Arc::new(self.get(point).await?);
        {
            let mut write = self.binds.write().await;
            write.put(point.clone(), bind.clone());
        }
        return Ok(ArtRef::new(bind, point.clone()));
    }

    async fn get<A>(&self, point: &Point) -> Result<A, UniErr>
    where
        A: TryFrom<Vec<u8>, Error =UniErr>,
    {
        if !point.has_bundle() {
            return Err("point is not from a bundle".into());
        }
        let bin = self.fetcher.fetch(point).await?;
        Ok(A::try_from(bin)?)
    }
}

#[derive(Clone)]
pub struct ArtRef<A> {
    artifact: Arc<A>,
    point: Point,
}

impl<A> ArtRef<A> {
    pub fn new(artifact: Arc<A>, point: Point) -> Self {
        Self { artifact, point }
    }
}

impl<A> ArtRef<A> {
    pub fn bundle(&self) -> Point {
        self.point.clone().to_bundle().unwrap()
    }
    pub fn point(&self) -> &Point {
        &self.point
    }
}

impl<A> Deref for ArtRef<A> {
    type Target = Arc<A>;

    fn deref(&self) -> &Self::Target {
        &self.artifact
    }
}

impl<A> Drop for ArtRef<A> {
    fn drop(&mut self) {
        //
    }
}

impl NoDiceArtifactFetcher {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
pub trait ArtifactFetcher: Send + Sync {
    async fn stub(&self, point: &Point) -> Result<Stub, UniErr>;
    async fn fetch(&self, point: &Point) -> Result<Vec<u8>, UniErr>;
}

pub struct FetchErr {}

pub struct NoDiceArtifactFetcher {}

#[async_trait]
impl ArtifactFetcher for NoDiceArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, UniErr> {
        Err(UniErr::from_status(404u16))
    }

    async fn fetch(&self, point: &Point) -> Result<Vec<u8>, UniErr> {
        Err(UniErr::from_status(404u16))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub point: Point,
    pub bin: Bin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRequest {
    pub point: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactResponse {
    pub to: Point,
    pub payload: Bin,
}
