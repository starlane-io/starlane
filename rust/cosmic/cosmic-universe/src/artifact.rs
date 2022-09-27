use core::borrow::Borrow;
use dashmap::DashMap;
use std::cell::Cell;
use std::ops::Deref;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, watch};

use crate::config::mechtron::MechtronConfig;
use crate::loc::{Point, ToSurface};
use crate::particle::Stub;
use crate::substance::Bin;
use crate::wave::core::cmd::CmdMethod;
use crate::wave::exchange::asynch::ProtoTransmitter;
use crate::wave::exchange::asynch::ProtoTransmitterBuilder;
use crate::wave::{DirectedProto, Pong, Wave};
use crate::{BindConfig, Substance, UniErr};

#[derive(Clone)]
pub struct ArtifactApi {
    binds: Arc<DashMap<Point, Arc<BindConfig>>>,
    mechtrons: Arc<DashMap<Point, Arc<MechtronConfig>>>,
    wasm: Arc<DashMap<Point, Bin>>,
    fetcher_tx: Arc<watch::Sender<Arc<dyn ArtifactFetcher>>>,
    fetcher_rx: watch::Receiver<Arc<dyn ArtifactFetcher>>
}

impl ArtifactApi {
    pub fn no_fetcher() -> Self {
        let fetcher = Arc::new(NoDiceArtifactFetcher);
        Self::new(fetcher)
    }

    pub fn new(fetcher: Arc<dyn ArtifactFetcher>) -> Self {
        let (fetcher_tx,fetcher_rx) = watch::channel(fetcher);
        let fetcher_tx = Arc::new(fetcher_tx);
        Self {
            binds: Arc::new(DashMap::new()),
            mechtrons: Arc::new(DashMap::new()),
            wasm: Arc::new(DashMap::new()),
            fetcher_tx,
            fetcher_rx
        }
    }

    pub async fn set_fetcher(&self, fetcher: Arc<dyn ArtifactFetcher>) {
        self.fetcher_tx.send(fetcher);
    }

    fn get_fetcher(&self) -> Arc<dyn ArtifactFetcher> {
        self.fetcher_rx.borrow().clone()
    }

    pub async fn mechtron(&self, point: &Point) -> Result<ArtRef<MechtronConfig>, UniErr> {
        {
            if self.mechtrons.contains_key(point) {
                let mechtron = self.mechtrons.get(point).unwrap().clone();
                return Ok(ArtRef::new(mechtron, point.clone()));
            }
        }

println!("fetching mechtron...");
        let mechtron: Arc<MechtronConfig> = Arc::new(self.get(point).await?);
        {
            self.mechtrons.insert(point.clone(), mechtron.clone());
        }
println!("got mechtron...");
        return Ok(ArtRef::new(mechtron, point.clone()));
    }

    pub async fn bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, UniErr> {
        {
            if self.binds.contains_key(point) {
                let bind = self.binds.get(point).unwrap().clone();
                return Ok(ArtRef::new(bind, point.clone()));
            }
        }

        let bind: Arc<BindConfig> = Arc::new(self.get(point).await?);
        {
            self.binds.insert(point.clone(), bind.clone());
        }
        return Ok(ArtRef::new(bind, point.clone()));
    }

    pub async fn wasm(&self, point: &Point) -> Result<ArtRef<Bin>, UniErr> {
        {
            if self.wasm.contains_key(point) {
                let wasm = self.wasm.get(point).unwrap().clone();
                return Ok(ArtRef::new(Arc::new(wasm), point.clone()));
            }
        }

        let wasm = self.get_fetcher().fetch(point).await?;
        {
            self.wasm.insert(point.clone(), wasm.clone());
        }
        return Ok(ArtRef::new(Arc::new(wasm), point.clone()));
    }

    async fn get<A>(&self, point: &Point) -> Result<A, UniErr>
    where
        A: TryFrom<Bin, Error = UniErr>,
    {
        if !point.has_bundle() {
            return Err("point is not from a bundle".into());
        }
        let bin = self.get_fetcher().fetch(point).await?;
        Ok(A::try_from(bin)?)
    }
}

pub struct FetchChamber {
    pub fetcher: Box<dyn ArtifactFetcher>,
}

impl FetchChamber {
    pub fn set(&mut self, fetcher: Box<dyn ArtifactFetcher>) {
        self.fetcher = fetcher;
    }
}

#[derive(Clone)]
pub struct ArtRef<A> {
    artifact: Arc<A>,
    pub point: Point,
}

impl<A> ArtRef<A> {
    pub fn new(artifact: Arc<A>, point: Point) -> Self {
        Self { artifact, point }
    }
}

impl<A> ArtRef<A>
where
    A: Clone,
{
    pub fn contents(&self) -> A {
        (*self.artifact).clone()
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

#[async_trait]
pub trait ArtifactFetcher: Send + Sync {
    async fn stub(&self, point: &Point) -> Result<Stub, UniErr>;
    async fn fetch(&self, point: &Point) -> Result<Bin, UniErr>;
}

pub struct FetchErr {}

pub struct NoDiceArtifactFetcher;

#[async_trait]
impl ArtifactFetcher for NoDiceArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, UniErr> {
        Err("cannot pull artifacts right now".into())
    }

    async fn fetch(&self, point: &Point) -> Result<Bin, UniErr> {
        Err("cannot pull artifacts right now".into())
    }
}

pub struct ReadArtifactFetcher {
    transmitter: ProtoTransmitter,
}

impl ReadArtifactFetcher {
    pub fn new(transmitter: ProtoTransmitter) -> Self {
        Self { transmitter }
    }
}

#[async_trait]
impl ArtifactFetcher for ReadArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, UniErr> {
        Err(UniErr::from_status(404u16))
    }

    async fn fetch(&self, point: &Point) -> Result<Bin, UniErr> {
        let mut directed = DirectedProto::ping();
        directed.to(point.clone().to_surface());
        directed.method(CmdMethod::Read);
println!("getting : {} ", point.to_surface().to_string());
        let pong = self.transmitter.ping(directed).await?;
println!("got: {} ", point.to_surface().to_string());
        pong.core.ok_or()?;
        match pong.variant.core.body {
            Substance::Bin(bin) => Ok(bin),
            _ => Err(UniErr::from_500(
                "encountered unexpected substance (expected Bin) when fetching Artifact",
            )),
        }
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
