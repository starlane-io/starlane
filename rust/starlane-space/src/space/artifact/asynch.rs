use crate::space::artifact::builtin::BUILTIN_FETCHER;
use crate::space::artifact::{builtin, ArtRef};
use crate::space::config::mechtron::MechtronConfig;
use crate::space::loc::ToSurface;
use crate::space::parse::doc;
use crate::space::parse::util::new_span;
use crate::space::point::Point;
use crate::space::settings::Timeouts;
use crate::space::wave::core::cmd::CmdMethod;
use crate::space::wave::exchange::asynch::ProtoTransmitter;
use crate::space::wave::{DirectedProto, WaitTime};
use crate::{Bin, BindConfig, SpaceErr, Stub, Substance};
use alloc::string::FromUtf8Error;
use core::str::FromStr;
use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::watch::Ref;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::error::Elapsed;
use tokio::time::Timeout;

#[derive(Clone, Error, Debug)]
pub enum ArtErr {
    #[error("artifact is in an unknown status")]
    UnknownStatus,
    #[error("artifact has not completed fetching ")]
    FetchingStatus,
    #[error("artifact raw data has completed fetching")]
    BinStatus,
    #[error("artifact is being parsing")]
    ParsingStatus,
    #[error("artifact not found.")]
    NotFound,
    #[error("repo for artifact is is not currently reachable")]
    Unreachable,
    #[error(transparent)]
    SpaceErr(#[from] SpaceErr),
    #[error("timeout")]
    Timeout,
    #[error("watch was unexpectedly cancelled")]
    WatchReceiveErr,
    #[error("watch broadcast unexpectedly failed")]
    BroadcastSendErr,
    #[error("Utf8 error")]
    Utf8Error,
}

impl From<FromUtf8Error> for ArtErr {
    fn from(_: FromUtf8Error) -> Self {
        Self::Utf8Error
    }
}

impl From<Elapsed> for ArtErr {
    fn from(_: Elapsed) -> Self {
        Self::Timeout
    }
}

impl From<watch::error::RecvError> for ArtErr {
    fn from(_: watch::error::RecvError) -> Self {
        Self::WatchReceiveErr
    }
}

impl<T> From<broadcast::error::SendError<T>> for ArtErr {
    fn from(_: broadcast::error::SendError<T>) -> Self {
        Self::BroadcastSendErr
    }
}

#[derive(Clone)]
pub struct Attempt {
    pub err: ArtErr,
    pub time: SystemTime,
    pub retries: u16,
}

impl Attempt {
    pub fn new(err: ArtErr) -> Self {
        Self {
            err,
            time: SystemTime::now(),
            retries: 1u16,
        }
    }
}

impl ArtErr {}

#[derive(Clone)]
pub enum ArtStatus<A> {
    Unknown,
    Fetching,
    Raw(Arc<Bin>),
    Parsing,
    Cached(ArtRef<A>),
    Fail(Attempt),
}

impl<A> Into<Result<ArtRef<A>, ArtErr>> for ArtStatus<A> {
    fn into(self) -> Result<ArtRef<A>, ArtErr> {
        match self {
            ArtStatus::Unknown => Err(ArtErr::UnknownStatus)?,
            ArtStatus::Fetching => Err(ArtErr::FetchingStatus)?,
            ArtStatus::Raw(raw) => Err(ArtErr::BinStatus)?,
            ArtStatus::Parsing => Err(ArtErr::ParsingStatus)?,
            ArtStatus::Cached(art) => Ok(art),
            ArtStatus::Fail(err) => Err(err.err)?,
        }
    }
}

pub struct ArtifactPipeline<A> {
    watch: watch::Receiver<ArtStatus<A>>,
}

#[derive(Clone)]
pub struct ArtifactsSkel {
    pub timeouts: Timeouts,
    pub wait_time: WaitTime,
}

impl Default for ArtifactsSkel {
    fn default() -> Self {
        Self {
            timeouts: Default::default(),
            wait_time: WaitTime::default(),
        }
    }
}

impl<A> ArtifactPipeline<A> {
    pub fn new(point: &Point, fetcher: Arc<dyn ArtifactFetcher>) -> ArtifactPipeline<A> {
        let runner = ArtifactPipelineRunner::new(point.clone(), fetcher);
        let watch = runner.watch();
        runner.start();
        Self { watch }
    }

    pub fn status(&self) -> ArtStatus<A> {
        self.watch.borrow().clone()
    }

    pub fn watch(&self) -> watch::Receiver<ArtStatus<A>> {
        self.watch.clone()
    }
}

struct ArtifactPipelineRunner<A> {
    point: Point,
    fetcher: Arc<dyn ArtifactFetcher>,
    broadcast_tx: broadcast::Sender<ArtStatus<A>>,
    watch_rx: watch::Receiver<ArtStatus<A>>,
}

impl<A> ArtifactPipelineRunner<A>
where
    A: FromStr<Err = SpaceErr>,
{
    pub fn new<B>(point: Point, fetcher: Arc<dyn ArtifactFetcher>) -> Self {
        let (watch_tx, watch_rx) = watch::channel(ArtStatus::Unknown);
        let (broadcast_tx, mut broadcast_rx) = broadcast::channel(10);

        tokio::spawn(async move {
            while let Ok(status) = broadcast_rx.recv().await {
                watch_tx.send(status).unwrap_or_default();
            }
        });

        let runner = Self {
            point,
            fetcher,
            broadcast_tx: broadcast_tx,
            watch_rx,
        };

        runner
    }

    pub fn watch(&self) -> watch::Receiver<ArtStatus<A>> {
        self.watch_rx.clone()
    }

    pub fn start(mut self) {
        tokio::spawn(async move {
            match self.run().await {
                Ok(_) => {}
                Err(err) => {
                    let attempt = Attempt::new(err);
                    self.broadcast_tx
                        .send(ArtStatus::Fail(attempt))
                        .unwrap_or_default();
                }
            }
        });
    }

    async fn run(&mut self) -> Result<(), ArtErr> {
        self.broadcast_tx.send(ArtStatus::Fetching)?;
        let bin = self.fetcher.fetch(&self.point).await?;
        self.broadcast_tx.send(ArtStatus::Raw(bin.clone()))?;
        let string = String::from_utf8((*bin).clone())?;

        self.broadcast_tx.send(ArtStatus::Parsing)?;
        let artifact = A::from_str(string.as_str())?;
        let (tx, rx) = mpsc::channel(10);
        let art = ArtRef::new(artifact, self.point.clone(), tx);
        self.broadcast_tx.send(ArtStatus::Cached(art))?;
        Ok(())
    }
}

struct ArtifactCache<A> {
    skel: ArtifactsSkel,
    artifacts: DashMap<Point, ArtRef<A>>,
    bins: DashMap<Point, Arc<Bin>>,
    pipelines: DashMap<Point, ArtifactPipeline<A>>,
    fetcher: Arc<dyn ArtifactFetcher>,
}

impl<A> ArtifactCache<A> {
    pub fn new(fetcher: Arc<dyn ArtifactFetcher>, skel: ArtifactsSkel) -> ArtifactCache<A> {
        ArtifactCache {
            skel,
            artifacts: DashMap::new(),
            bins: DashMap::new(),
            pipelines: DashMap::new(),
            fetcher,
        }
    }

    pub async fn get(&self, point: &Point) -> Result<ArtRef<A>, ArtErr> {
        self.get_with_wait(point, self.skel.wait_time.clone())
    }
    pub async fn get_with_wait(&self, point: &Point, wait: WaitTime) -> Result<ArtRef<A>, ArtErr> {
        if let Some(art) = self.artifacts.get(point) {
            return Ok(art.value().clone());
        }

        let timeout = Duration::from_secs(self.skel.timeouts.from(wait));
        let fetcher = self.fetcher.clone();
        let pipeline = self
            .pipelines
            .entry(point.clone())
            .or_insert_with(move || ArtifactPipeline::new(point, fetcher))
            .value();
        let mut watch = pipeline.watch();
        let bins = &self.bins;
        let artifacts = &self.artifacts;
        let status = tokio::time::timeout(
            timeout,
            watch.wait_for(move |status| match status {
                ArtStatus::Raw(bin) => {
                    bins.insert(point.clone(), bin.clone());
                    false
                }
                ArtStatus::Cached(art) => {
                    artifacts.insert(point.clone(), art.clone());
                    true
                }
                ArtStatus::Fail(_) => true,
                _ => false,
            }),
        )
        .await??;

        match status.clone() {
            ArtStatus::Unknown => Err(ArtErr::UnknownStatus),
            ArtStatus::Fetching => Err(ArtErr::FetchingStatus),
            ArtStatus::Raw(_) => Err(ArtErr::BinStatus),
            ArtStatus::Parsing => Err(ArtErr::ParsingStatus),
            ArtStatus::Cached(art) => Ok(art),
            ArtStatus::Fail(err) => Err(err.err),
        }
    }
}

impl ArtifactCache<Bin> {}

pub struct ArtifactHub {
    skel: ArtifactsSkel,
    pub bind: ArtifactCache<BindConfig>,
    pub mechtron: ArtifactCache<MechtronConfig>,
}

impl ArtifactHub {
    pub fn new(fetcher: Arc<dyn ArtifactFetcher>, skel: ArtifactsSkel) -> ArtifactHub {
        ArtifactHub {
            bind: ArtifactCache::new(fetcher.clone(), skel.clone()),
            mechtron: ArtifactCache::new(fetcher.clone(), skel.clone()),
            skel,
        }
    }

    pub async fn bind_conf(&self, point: &Point) -> Result<ArtRef<BindConfig>, ArtErr> {
        self.bind.get(point).await
    }

    pub async fn mechtron_conf(&self, point: &Point) -> Result<ArtRef<MechtronConfig>, ArtErr> {
        self.mechtron.get(point).await
    }
}

pub struct ArtifactsBuilder {
    fetchers: Vec<Arc<dyn ArtifactFetcher>>,
}

impl ArtifactsBuilder {
    pub fn new() -> Self {
        Self { fetchers: vec![] }
    }
}

impl Deref for ArtifactsBuilder {
    type Target = Vec<Arc<dyn ArtifactFetcher>>;

    fn deref(&self) -> &Self::Target {
        &self.fetchers
    }
}

impl DerefMut for ArtifactsBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.fetchers
    }
}

#[derive(Clone)]
pub struct Artifacts {
    hubs: Vec<Arc<ArtifactHub>>,
}

impl Artifacts {
    pub fn just_builtins() -> Self {
        let fetcher = BUILTIN_FETCHER.clone();
        let hub = Arc::new(ArtifactHub::new(fetcher, ArtifactsSkel::default()));
        Self { hubs: vec![hub] }
    }

    pub async fn get<A>(&self, point: &Point) -> Result<ArtRef<A>, ArtErr>
    where
        A: FromStr<Err = SpaceErr>,
    {
        for hub in &self.hubs {
            hub.get
        }
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
pub trait ArtifactFetcher: Send + Sync {
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr>;
    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, SpaceErr>;
}

pub struct NoDiceArtifactFetcher;

#[async_trait]
impl ArtifactFetcher for NoDiceArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr> {
        Err("cannot pull artifacts right now".into())
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, SpaceErr> {
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
