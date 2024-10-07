use crate::space::artifact::asynch::ArtifactFetcher;
use crate::space::artifact::ArtRef;
use crate::space::config::bind::BindConfig;
use crate::space::err::SpaceErr;
use crate::space::parse::bind_config;
use crate::space::particle::Stub;
use crate::space::point::Point;
use crate::space::substance::Bin;
use crate::space::util::log;
use core::str::FromStr;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;

pub static BUILTIN_FETCHER: Lazy<Arc<BuiltinArtifactFetcher>> = Lazy::new(|| {
    let mut builder = BuiltinArtifactFetcherBuilder::new();
    builder.insert(
        Point::from_str("GLOBAL::repo:1.0.0:/bind/star.bind").unwrap(),
        Arc::new(include_str!("../../../conf/star.bind").into()).unwrap(),
    );

    builder.insert(
        Point::from_str("GLOBAL::repo:1.0.0:/bind/driver.bind").unwrap(),
        Arc::new(include_str!("../../../conf/driver.bind").into()).unwrap(),
    );
    builder.insert(
        Point::from_str("GLOBAL::repo:1.0.0:/bind/nothing.bind").unwrap(),
        Arc::new(include_str!("../../../conf/nothing.bind").into()).unwrap(),
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
    async fn stub(&self, point: &Point) -> Result<Stub, SpaceErr> {
        Err("cannot pull artifacts right now".into())
    }

    async fn fetch(&self, point: &Point) -> Result<Arc<Bin>, SpaceErr> {
        Ok(self
            .bins
            .get(point)
            .cloned()
            .ok_or(SpaceErr::not_found(point))?)
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
