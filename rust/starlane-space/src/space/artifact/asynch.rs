use crate::space::artifact::builtin::BUILTIN_FETCHER;
use crate::space::artifact::ArtRef;
use crate::space::config::mechtron::MechtronConfig;
use crate::space::err::{ParseErrs, PrintErr};
use crate::space::loc::ToSurface;
use crate::space::point::Point;
use crate::space::selector::{PointSelector, Selector};
use crate::space::settings::Timeouts;
use crate::space::util::{ValueMatcher, ValuePattern};
use crate::space::wave::core::cmd::CmdMethod;
use crate::space::wave::exchange::asynch::ProtoTransmitter;
use crate::space::wave::{DirectedProto, WaitTime};
use crate::{Bin, BindConfig, Stub, Substance};
use alloc::string::FromUtf8Error;
use anyhow::anyhow;
use core::fmt::Display;
use core::str::FromStr;
use dashmap::DashMap;
use serde::Serialize;
use std::collections::HashMap;
use std::error::Error;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::error::Elapsed;

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
    #[error("artifact '{0}' not found.")]
    NotFound(Point),
    #[error("repo for artifact is is not currently reachable")]
    Unreachable,
    #[error("timeout")]
    Timeout,
    #[error("watch was unexpectedly cancelled")]
    WatchReceiveErr,
    #[error("watch broadcast unexpectedly failed")]
    BroadcastSendErr,
    #[error("Utf8 error")]
    Utf8Error,
    #[error("Artifacts cannot be fetched because no Artifacts service not available")]
    ArtifactServiceNotAvailable,
    #[error("expecting {thing}: {expecting}, found: {found}")]
    Expecting {
        thing: String,
        expecting: String,
        found: String,
    },
    #[error(transparent)]
    ParseErrs(#[from] ParseErrs),
    #[error("Err({0})")]
    Source(#[source] Arc<anyhow::Error>),
}

impl ArtErr {
    pub fn expecting<A, B, C>(thing: A, expecting: B, found: C) -> Self
    where
        A: ToString,
        B: ToString,
        C: ToString,
    {
        Self::Expecting {
            thing: thing.to_string(),
            expecting: expecting.to_string(),
            found: found.to_string(),
        }
    }

    pub fn not_found(point: &Point) -> Self {
        ArtErr::NotFound(point.clone())
    }


    pub fn err<E>(err: E) -> Self
    where
        E: Display,
    {
        ArtErr::Source(Arc::new(anyhow!("Err: {err}")))
    }

    pub fn result<R, E>(result: Result<R, E>) -> Result<R, ArtErr>
    where
        E: Display,
    {
        match result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(ArtErr::err(err)),
        }
    }
}

impl From<anyhow::Error> for ArtErr {
    fn from(err: anyhow::Error) -> Self {
        ArtErr::Source(Arc::new(err))
    }
}

impl PrintErr for ArtErr {
    fn print(&self) {
        println!("ArtErr: {}", self);
    }
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

pub enum ArtStatus<A> {
    Unknown,
    Fetching,
    Raw(Arc<Bin>),
    Parsing,
    Cached(ArtRef<A>),
    Fail(Attempt),
}

impl<A> Clone for ArtStatus<A> {
    fn clone(&self) -> Self {
        match self {
            ArtStatus::Unknown => ArtStatus::Unknown,
            ArtStatus::Fetching => ArtStatus::Fetching,
            ArtStatus::Raw(raw) => ArtStatus::Raw(raw.clone()),
            ArtStatus::Parsing => ArtStatus::Parsing,
            ArtStatus::Cached(art) => ArtStatus::Cached(art.clone()),
            ArtStatus::Fail(err) => ArtStatus::Fail(err.clone()),
        }
    }
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

pub struct ArtifactPipeline<A>
where
    A: FromStr<Err =ParseErrs>,
{
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

impl<A> ArtifactPipeline<A>
where
    A: FromStr<Err =ParseErrs> + 'static,
{
    pub fn new(point: &Point, fetcher: Arc<dyn ArtifactFetcher>) -> ArtifactPipeline<A> {
        let runner = ArtifactPipelineRunner::new(point.clone(), fetcher);
        let watch = runner.watch();
        runner.start();
        Self { watch }
    }

    pub fn status(&mut self) -> ArtStatus<A> {
        let x = self.watch.borrow_and_update();
        let x = (*x).clone();
        x
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
    A: FromStr<Err =ParseErrs> + 'static,
{
    pub fn new(point: Point, fetcher: Arc<dyn ArtifactFetcher>) -> Self {
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

struct ArtifactCache<A>
where
    A: FromStr<Err =ParseErrs>,
{
    skel: ArtifactsSkel,
    artifacts: DashMap<Point, ArtRef<A>>,
    bins: DashMap<Point, Arc<Bin>>,
    pipelines: DashMap<Point, ArtifactPipeline<A>>,
    fetcher: Arc<dyn ArtifactFetcher>,
}

impl<A> ArtifactCache<A>
where
    A: FromStr<Err =ParseErrs> + 'static,
{
    pub fn new(fetcher: Arc<dyn ArtifactFetcher>, skel: ArtifactsSkel) -> ArtifactCache<A> {
        ArtifactCache {
            skel,
            artifacts: DashMap::new(),
            bins: DashMap::new(),
            pipelines: DashMap::new(),
            fetcher,
        }
    }

    pub fn selector(&self) -> ValuePattern<Selector> {
        self.fetcher.selector()
    }

    pub async fn get(&self, point: &Point) -> Result<ArtRef<A>, ArtErr> {
        self.get_with_wait(point, &self.skel.wait_time).await
    }

    pub async fn get_with_wait(&self, point: &Point, wait: &WaitTime) -> Result<ArtRef<A>, ArtErr> {
        if let Some(art) = self.artifacts.get(point) {
            let art2 = &*art;
            //return Ok((*art).clone());
            return Ok(art2.clone());
        }

        let timeout = Duration::from_secs(self.skel.timeouts.from_wait(wait));
        let fetcher = self.fetcher.clone();
        let pipeline = self
            .pipelines
            .entry(point.clone())
            .or_insert_with(move || ArtifactPipeline::new(point, fetcher));
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

        match &*status {
            ArtStatus::Unknown => Err(ArtErr::UnknownStatus),
            ArtStatus::Fetching => Err(ArtErr::FetchingStatus),
            ArtStatus::Raw(_) => Err(ArtErr::BinStatus),
            ArtStatus::Parsing => Err(ArtErr::ParsingStatus),
            ArtStatus::Cached(art) => Ok(art.clone()),
            ArtStatus::Fail(err) => Err(err.err.clone()),
        }
    }
}

pub struct ArtifactHub {
    skel: ArtifactsSkel,
    pub bind: ArtifactCache<BindConfig>,
    pub mechtron: ArtifactCache<MechtronConfig>,
    pub selector: PointSelector,
}

impl ArtifactHub {
    pub fn new(fetcher: Arc<dyn ArtifactFetcher>, skel: ArtifactsSkel) -> ArtifactHub {
        ArtifactHub {
            bind: ArtifactCache::new(fetcher.clone(), skel.clone()),
            mechtron: ArtifactCache::new(fetcher.clone(), skel.clone()),
            skel,
            selector: PointSelector::always()
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

    pub async fn get_bind(&self, point: &Point) -> Result<ArtRef<BindConfig>, ArtErr> {
        for hub in &self.hubs {
            if hub.selector.is_match(point).is_ok() {
                match hub.bind.get(point).await {
                    Ok(art) => return Ok(art),
                    Err(err) => {
                        return Err(err);
                    }
                }
            }
        }
        Err(ArtErr::NotFound(point.clone()))
    }

    pub async fn get_mechtron(
        &self,
        point: &Point,
    ) -> Option<Result<ArtRef<MechtronConfig>, ArtErr>> {
        for hub in &self.hubs {
            match hub.mechtron.get(point).await {
                Ok(art) => return Some(Ok(art)),
                Err(ArtErr::NotFound(_)) => return None,
                Err(err) => {
                    return Some(Err(err));
                }
            }
        }
        None
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
    async fn stub(&self, point: &Point) -> Result<Stub, ArtErr>;
    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, ArtErr>;
    fn selector(&self) -> ValuePattern<Selector>;
}

pub struct NoDiceArtifactFetcher;

#[async_trait]
impl ArtifactFetcher for NoDiceArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, ArtErr> {
        Err(ArtErr::ArtifactServiceNotAvailable)
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, ArtErr> {
        Err(ArtErr::ArtifactServiceNotAvailable)
    }

    fn selector(&self) -> ValuePattern<Selector> {
        ValuePattern::Never
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
    async fn stub(&self, point: &Point) -> Result<Stub, ArtErr> {
        Err(ArtErr::not_found(point))
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, ArtErr> {
        let mut directed = DirectedProto::ping();
        directed.to(point.clone().to_surface());
        directed.method(CmdMethod::Read);
        let pong = ArtErr::result(self.transmitter.ping(directed).await)?;
        ArtErr::result(pong.core.ok_or())?;
        match pong.variant.core.body {
            Substance::Bin(bin) => Ok(Arc::new(bin)),
            ref other => Err(ArtErr::expecting("Substance", "Bin", other.kind()))?,
        }
    }

    fn selector(&self) -> ValuePattern<Selector> {
        todo!()
    }
}

pub struct MapFetcher {
    pub map: HashMap<Point, Arc<Bin>>,
}

#[async_trait]
impl ArtifactFetcher for MapFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, ArtErr> {
        todo!()
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, ArtErr> {
        let rtn = self.map.get(point).ok_or(ArtErr::not_found(point))?;
        Ok(rtn.clone())
    }

    fn selector(&self) -> ValuePattern<Selector> {
        todo!()
    }
}

impl MapFetcher {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
    pub fn ser<S: Serialize>(&mut self, point: &Point, bin: S) {
        let bin = Arc::new(bincode::serialize(&bin).unwrap());
        self.map.insert(point.clone(), bin);
    }

    pub fn str<S: ToString>(&mut self, point: &Point, string: S) {
        let bin = Arc::new(string.to_string().into_bytes());
        self.map.insert(point.clone(), bin);
    }
}

#[cfg(test)]
#[test]
fn test() {
    struct Blah;

    let arc = Arc::new(Blah);

    arc.clone();

    #[derive(Clone)]
    struct BeBo<A> {
        arc: Arc<A>,
    }
}
