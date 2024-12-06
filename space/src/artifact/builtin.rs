use crate::artifact::asynch::{ArtErr, ArtifactFetcher};
use crate::kind::BaseKind;
use crate::particle::Stub;
use crate::point::Point;
use crate::selector::Selector;
use crate::substance::Bin;
use crate::util::ValuePattern;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub static BUILTIN_FETCHER: Lazy<Arc<BuiltinArtifactFetcher>> = Lazy::new(|| {
    let mut builder = BuiltinArtifactFetcherBuilder::new();

    builder.insert(
        BaseKind::Star.bind(),
        Arc::<Vec<u8>>::new(include_bytes!("../../conf/star.bind").into()),
    );

    builder.insert(
        BaseKind::Driver.bind(),
        Arc::<Vec<u8>>::new(include_bytes!("../../conf/driver.bind").into()),
    );

    builder.insert(
        BaseKind::Global.bind(),
        Arc::<Vec<u8>>::new(include_bytes!("../../conf/global.bind").into()),
    );

    builder.insert(
        BaseKind::nothing_bind(),
        Arc::<Vec<u8>>::new(include_bytes!("../../conf/nothing.bind").into()),
    );

    Arc::new(builder.build())
});

impl BuiltinArtifactFetcherBuilder {
    pub fn add(&mut self, point: &Point, bin: Bin) {
        self.bins.insert(point.clone(), Arc::new(bin));
    }

    pub fn build(self) -> BuiltinArtifactFetcher {
        BuiltinArtifactFetcher { bins: self.bins }
    }
}

pub struct BuiltinArtifactFetcher {
    bins: HashMap<Point, Arc<Bin>>,
}

#[async_trait]
impl ArtifactFetcher for BuiltinArtifactFetcher {
    async fn stub(&self, point: &Point) -> Result<Stub, ArtErr> {
        Err(ArtErr::ArtifactServiceNotAvailable)
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, ArtErr> {
        Ok(self
            .bins
            .get(point)
            .cloned()
            .ok_or(ArtErr::not_found(point))?)
    }

    fn selector(&self) -> ValuePattern<Selector> {
        todo!()
    }
}

impl Deref for BuiltinArtifactFetcherBuilder {
    type Target = HashMap<Point, Arc<Bin>>;

    fn deref(&self) -> &Self::Target {
        &self.bins
    }
}

pub struct BuiltinArtifactFetcherBuilder {
    bins: HashMap<Point, Arc<Bin>>,
}

impl BuiltinArtifactFetcherBuilder {
    pub fn new() -> Self {
        Self {
            bins: Default::default(),
        }
    }
}

impl DerefMut for BuiltinArtifactFetcherBuilder {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bins
    }
}
