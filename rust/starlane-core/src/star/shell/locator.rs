use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};
use core::time::Duration;
use std::str::FromStr;

use async_trait::async_trait;
use lru::LruCache;
use mesh_portal::version::latest::id::{Point, RouteSegment};
use mesh_portal::version::latest::particle::{Stub, Status};
use mesh_portal::version::latest::security::Permissions;
use mesh_portal_versions::version::v0_0_1::parse::permissions;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::frame::{ResourceRegistryRequest,  SimpleReply, StarMessagePayload};
use crate::message::{ProtoStarMessage, ReplyKind, Reply};
use crate::particle::{Kind, ParticleRecord, KindBase};
use crate::star::{
    LogId, Request,  Set, Star, StarCommand, StarKey, StarKind, StarSkel,
};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::error::Error;

#[derive(Clone)]
pub struct ResourceLocatorApi {
    pub tx: mpsc::Sender<ResourceLocateCall>,
}

impl ResourceLocatorApi {
    pub fn new(tx: mpsc::Sender<ResourceLocateCall>) -> Self {
        Self { tx }
    }


    pub async fn locate(&self, address: Point) -> Result<ParticleRecord, Error> {
        let (tx, mut rx) = oneshot::channel();
        self.tx
            .send(ResourceLocateCall::Locate { address, tx })
            .await
            .unwrap_or_default();

        let rtn = tokio::time::timeout(Duration::from_secs(15), rx).await???;
        Ok(rtn)
    }


    pub async fn external_locate(
        &self,
        address: Point,
        star: StarKey,
    ) -> Result<ParticleRecord, Error> {
        let (tx, mut rx) = oneshot::channel();
        self.tx
            .send(ResourceLocateCall::ExternalLocate {
                star,
                address: address,
                tx,
            })
            .await
            .unwrap_or_default();

        Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
    }

    pub fn found(&self, record: ParticleRecord) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(ResourceLocateCall::Found(record))
                .await
                .unwrap_or_default();
        });
    }

    pub fn filter(&self, result: Result<ParticleRecord, Error>) -> Result<ParticleRecord, Error> {

        if let Result::Ok(record) = &result {
            self.found(record.clone());
        }
        result
    }
}

pub enum ResourceLocateCall {
    Locate {
        address: Point,
        tx: oneshot::Sender<Result<ParticleRecord, Error>>,
    },
    ExternalLocate {
        address: Point,
        star: StarKey,
        tx: oneshot::Sender<Result<ParticleRecord, Error>>,
    },
    Found(ParticleRecord),
}

impl Call for ResourceLocateCall {}

pub struct ResourceLocatorComponent {
    skel: StarSkel,
    resource_record_cache: LruCache<Point, ParticleRecord>,
}

impl ResourceLocatorComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<ResourceLocateCall>) {
        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                resource_record_cache: LruCache::new(1024),
            }),
            skel.resource_locator_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<ResourceLocateCall> for ResourceLocatorComponent {
    async fn process(&mut self, call: ResourceLocateCall) {
        match call {
            ResourceLocateCall::Locate { address: address, tx } => {
                self.locate(address, tx);
            }
            ResourceLocateCall::ExternalLocate {
                address: address,
                star,
                tx,
            } => {
                self.external_locate(address, star, tx).await;
            }
            ResourceLocateCall::Found(record) => {
                self.resource_record_cache
                    .put(record.stub.point.clone(), record);
            }
        }
    }
}

impl ResourceLocatorComponent {
    fn locate(
        &mut self,
        address: Point,
        tx: oneshot::Sender<Result<ParticleRecord, Error>>,
    ) {
        if self.has_cached_record(&address) {
            let result = match self
                .get_cached_record(&address)
                .ok_or("expected particle record")
            {
                Ok(record) => Ok(record),
                Err(s) => Err(s.to_string().into()),
            };

            tx.send(result).unwrap_or_default();
        } else if let RouteSegment::Mesh(star) = &address.route {
            match StarKey::from_str(star.as_str()) {
                Ok(star) => {
                    let skel = self.skel.clone();
                    tokio::spawn( async move {
                        let result = skel.resource_locator_api
                            .external_locate(address, star)
                            .await;
                        tx.send(result);
                    });
                }
                Err(error) => {
                    eprintln!("invalid StarKey string: {}",error.to_string());
                }
            }

        }
        else if address.parent().is_some() {
            let locator_api = self.skel.resource_locator_api.clone();
            tokio::spawn(async move {
                async fn locate(
                    locator_api: ResourceLocatorApi,
                    address: Point,
                ) -> Result<ParticleRecord, Error> {
                    let parent_record = locator_api.filter(
                        locator_api
                            .locate(
                                address
                                    .parent()
                                    .expect("expected this address to have a parent"),
                            )
                            .await,
                    )?;
                    let rtn = locator_api
                        .external_locate(address, parent_record.location.ok_or()?)
                        .await?;

                    Ok(rtn)
                }

                tx.send(locate(locator_api, address).await)
                    .unwrap_or_default();
            });
        } else {

            let record = ParticleRecord::new(
                Stub {
                    point: Point::root(),
                        kind: Kind::Root.to_resource_kind(),
                    properties: Default::default(),
                    status: Status::Ready
                },
                StarKey::central(),
                Permissions::none()
            );
            tx.send(Ok(record)).unwrap_or_default();
        }
    }

    async fn external_locate(
        &mut self,
        address: Point,
        star: StarKey,
        tx: oneshot::Sender<Result<ParticleRecord, Error>>,
    ) {
        let (request, rx) = Request::new((address, star));
        self.request_resource_record_from_star(request).await;
        tokio::spawn(async move {
            async fn timeout(
                rx: oneshot::Receiver<Result<ParticleRecord, Error>>,
            ) -> Result<ParticleRecord, Error> {
                Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
            }
            tx.send(timeout(rx).await).unwrap_or_default();
        });
    }

    fn has_cached_record(&mut self, address: &Point) -> bool {
      self.resource_record_cache.contains(address)
    }

    fn get_cached_record(&mut self, address: &Point) -> Option<ParticleRecord> {
        self.resource_record_cache.get(address).cloned()
    }

    async fn request_resource_record_from_star(
        &mut self,
        locate: Request<(Point, StarKey), ParticleRecord>,
    ) {
        let (address, star) = locate.payload.clone();
        let mut proto = ProtoStarMessage::new();
        proto.to = star.clone().into();
        proto.payload = StarMessagePayload::ResourceRegistry(ResourceRegistryRequest::Find(address));
        proto.log = locate.log;
        let skel = self.skel.clone();
        tokio::spawn(async move {
            let result = skel
                .messaging_api
                .star_exchange(
                    proto,
                    ReplyKind::Record,
                    "ResourceLocatorComponent.request_resource_record_from_star()",
                )
                .await;
            match result {
                Ok(Reply::Record(record)) => {
                    skel.resource_locator_api.found(record.clone());
                    locate.tx.send(Ok(record)).unwrap_or_default();
                }
                Err(fail) => {
                    locate.tx.send(Err(fail)).unwrap_or_default();
                }
                _ => unimplemented!(
                    "ResourceLocatorComponent.request_resource_record_from_star(): IMPOSSIBLE!"
                ),
            }
        });
    }
}
