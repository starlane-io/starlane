use crate::space::artifact::ArtRef;
use crate::space::config::mechtron::MechtronConfig;
use crate::space::loc::ToSurface;
use crate::space::point::Point;
use crate::space::wave::core::cmd::CmdMethod;
use crate::space::wave::exchange::asynch::ProtoTransmitter;
use crate::space::wave::DirectedProto;
use crate::{Bin, BindConfig, SpaceErr, Stub, Substance};
use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::SystemTime;
use dashmap::mapref::one::Ref;
use thiserror::Error;
use tokio::sync::watch;
use tokio::sync::watch::Receiver;
use crate::space::config::{DocKind, Document};

#[derive(Clone,Error,Debug)]
pub enum ArtErr {
    #[error("artifact '{0}' not found.")]
    NotFound(Point),
    #[error("repo for artifact '{0}' is is not currently reachable")]
    Unreachable(Point)
}

#[derive(Clone)]
pub struct Attempt{
    pub err: ArtErr,
    pub time: SystemTime
}

impl Attempt {
    pub fn new( err: ArtErr) -> Self {
        Self {
            err,
            time: SystemTime::now()
        }
    }
}

impl ArtErr {
}

pub enum ArtStatus<A> {
    Unknown,
    Fetching,
    Raw(String),
    Parsing,
    Cached(ArtRef<A>),
    Fail(Attempt)
}

pub struct ArtifactPipeline<A> {
    watch: watch::Receiver<ArtStatus<A>>
}

impl <A>  ArtifactPipeline<A> {
    pub fn new( ) -> (ArtifactPipeline<A>, watch::Sender<ArtStatus<A>>)  {
        let (tx, watch) = watch::channel(ArtStatus::Unknown);
        (ArtifactPipeline {
            watch
        }, tx)
    }
}

struct ArtifactCache<A,F> where F: ArtifactFetcher {
   artifacts: DashMap<Point, ArtRef<A>>,
   bins: DashMap<Point, Arc<Bin>>,
   pipeline: DashMap<Point, ArtifactPipeline<A>>,
   fetcher: F
}

impl <'a,A,F> ArtifactCache<A,&'a F> where F: ArtifactFetcher{
   pub fn new(fetcher: &'a F) -> ArtifactCache<A,&'a F> {
       ArtifactCache {
           artifacts: Default::default(),
           bins: Default::default(),
           pipeline: Default::default(),
           fetcher,
       }
   }
}



pub struct Artifacts<'a> {
    fetcher: Box<dyn ArtifactFetcher>,
    pub bind: ArtifactCache<BindConfig,&'a dyn ArtifactFetcher>,
    pub mechtron: ArtifactCache<MechtronConfig,&'a dyn ArtifactFetcher>
}


impl <'a> Artifacts<'a>  {
    pub fn new( fetcher: Box<dyn ArtifactFetcher>) -> Artifacts<'a>{
        Artifacts {
            bind: ArtifactCache::new(&*fetcher),
            mechtron: ArtifactCache::new(&*fetcher),
            fetcher,
        }
    }
}



impl <A,F>  ArtifactCache<A,F> where F: ArtifactFetcher{
    fn new(fetcher: F) -> ArtifactCache<A,F>{
        Self {
            artifacts: Default::default(),
            bins: Default::default(),
            pipeline: Default::default(),
            fetcher: fetcher,
        }
    }
}

#[derive(Clone)]
pub struct ArtifactApi {
    builtin: Arc<Artifacts>,
    cached: Arc<Artifacts>,
}

impl ArtifactApi {
    pub fn no_fetcher() -> Self {
        let fetcher = Arc::new(NoDiceArtifactFetcher{});
        Self::new(fetcher)
    }

    pub fn new(fetcher: Arc<dyn ArtifactFetcher>) -> Self {
        let builtin= Arc::new(ArtifactCache::default());
        let cached = Arc::new(ArtifactCache::default());
        Self {
            builtin,
            cached,
        }
    }

    pub fn bind<A>( &self, point: &Point ) -> Result<ArtRef<A>,SpaceErr> where A: TryFrom<Document>{

        match self.builtin.bind.get_mut(point) {
            Some(v) => {

            },
            None => {}
        }
    }

    pub async fn set_fetcher(&self, fetcher: Arc<dyn ArtifactFetcher>) {
        self.fetcher_tx.send(fetcher);
    }

    fn get_fetcher(&self) -> Arc<dyn ArtifactFetcher> {
        self.fetcher_rx.borrow().clone()
    }

    pub async fn mechtron(&self, point: &Point) -> Result<ArtRef<MechtronConfig>, SpaceErr> {
        {
            if self.mechtrons.contains_key(point) {
                let mechtron = self.mechtrons.get(point).unwrap().clone();
                return Ok(ArtRef::new(mechtron, point.clone()));
            }
        }

        let mechtron: Arc<MechtronConfig> = Arc::new(self.fetch(point).await?);
        self.mechtrons.insert(point.clone(), mechtron.clone());
        return Ok(ArtRef::new(mechtron, point.clone()));
    }

/*    pub async fn bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, SpaceErr> {
        {
            if self.binds.contains_key(point) {
                let bind = self.binds.get(point).unwrap().clone();
                return Ok(ArtRef::new(bind, point.clone()));
            }
        }

        let bind: Arc<BindConfig> = Arc::new(self.fetch(point).await?);
        {
            self.binds.insert(point.clone(), bind.clone());
        }
        return Ok(ArtRef::new(bind, point.clone()));
    }

 */

    pub async fn wasm(&self, point: &Point) -> Result<ArtRef<Bin>, SpaceErr> {
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

    async fn fetch<A>(&self, point: &Point) -> Result<A, SpaceErr>
    where
        A: TryFrom<Bin, Error = SpaceErr>,
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

#[async_trait]
pub trait ArtifactFetcher: Send + Sync + Clone {
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr>;
    async fn fetch(&self, point: &Point ) -> Result<ArtifactPipeline<Bin>, SpaceErr>;
}


pub struct BuiltinArtifactFetcherBuilder {
    bins: HashMap<Point,Arc<Bin>>
}

impl BuiltinArtifactFetcherBuilder {
    pub fn add(& mut self, point: &Point, bin: Bin ) {
        self.bins.insert(point.clone(),Arc::new(bin));
    }

    pub fn build(self) -> BuiltinArtifactFetcher {
        BuiltinArtifactFetcher {
            bins: self.bins
        }
    }
}

impl Deref for BuiltinArtifactFetcherBuilder {
    type Target = HashMap<Point,Arc<Bin>>;

    fn deref(&self) -> &Self::Target {
        &self.bins
    }
}

impl DerefMut for BuiltinArtifactFetcherBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        & mut self.bins
    }
}



pub struct BuiltinArtifactFetcher {
    bins: HashMap<Point,Arc<Bin>>
}
#[async_trait]
impl ArtifactFetcher for BuiltinArtifactFetcher{
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr> {
        Err("cannot pull artifacts right now".into())
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, SpaceErr> {
        Ok(self.bins.get(point).cloned().ok_or(SpaceErr::not_found(point))?)
    }
}

pub struct NoDiceArtifactFetcher;

#[async_trait]
impl ArtifactFetcher for NoDiceArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr> {
        Err("cannot pull artifacts right now".into())
    }

    async fn fetch(&self, point: &Point) -> Result<Bin, SpaceErr> {
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
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr> {
        Err(SpaceErr::from_status(404u16))
    }

    async fn fetch(&self, point: &Point) -> Result<Bin, SpaceErr> {
        let mut directed = DirectedProto::ping();
        directed.to(point.clone().to_surface());
        directed.method(CmdMethod::Read);
        let pong = self.transmitter.ping(directed).await?;
        pong.core.ok_or()?;
        match pong.variant.core.body {
            Substance::Bin(bin) => Ok(bin),
            other => Err(SpaceErr::server_error(format!(
                "expected Bin, encountered unexpected substance {} when fetching Artifact",
                other.kind().to_string()
            ))),
        }
    }
}

pub struct MapFetcher {
    pub map: HashMap<Point, Bin>,
}

#[async_trait]
impl ArtifactFetcher for MapFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr> {
        todo!()
    }

    async fn fetch(&self, point: &Point) -> Result<Bin, SpaceErr> {
        let rtn = self.map.get(point).ok_or(SpaceErr::not_found(format!(
            "could not find {}",
            point.to_string()
        )))?;
        Ok(rtn.clone())
    }
}

impl MapFetcher {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    pub fn ser<S: Serialize>(&mut self, point: &Point, bin: S) {
        let bin = bincode::serialize(&bin).unwrap();
        self.map.insert(point.clone(), bin);
    }

    pub fn str<S: ToString>(&mut self, point: &Point, string: S) {
        let bin = string.to_string().into_bytes();
        self.map.insert(point.clone(), bin);
    }
}
