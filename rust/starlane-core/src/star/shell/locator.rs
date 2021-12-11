use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};
use core::time::Duration;

use async_trait::async_trait;
use lru::LruCache;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use crate::frame::{ResourceRegistryRequest, Reply, ReplyKind, SimpleReply, StarMessagePayload};
use crate::message::ProtoStarMessage;
use crate::resource::{Kind, ResourceRecord, ResourceType};
use crate::star::{
    LogId, Request, ResourceRegistryBacking, Set, Star, StarCommand, StarKey, StarKind, StarSkel,
};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::error::Error;
use crate::mesh::serde::id::Address;
use crate::mesh::serde::generic::resource::ResourceStub;
use crate::resource::selector::ConfigSrc;
use crate::fail::Fail;

#[derive(Clone)]
pub struct ResourceLocatorApi {
    pub tx: mpsc::Sender<ResourceLocateCall>,
}

impl ResourceLocatorApi {
    pub fn new(tx: mpsc::Sender<ResourceLocateCall>) -> Self {
        Self { tx }
    }


    pub async fn locate(&self, address: Address ) -> Result<ResourceRecord, Fail> {
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
        address: Address,
        star: StarKey,
    ) -> Result<ResourceRecord, Error> {
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

    pub fn found(&self, record: ResourceRecord) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            tx.send(ResourceLocateCall::Found(record))
                .await
                .unwrap_or_default();
        });
    }

    pub fn filter(&self, result: Result<ResourceRecord, Fail>) -> Result<ResourceRecord, Fail> {

        if let Result::Ok(record) = &result {
            self.found(record.clone());
        }
        result
    }
}

pub enum ResourceLocateCall {
    Locate {
        address: Address,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    },
    ExternalLocate {
        address: Address,
        star: StarKey,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    },
    Found(ResourceRecord),
}

impl Call for ResourceLocateCall {}

pub struct ResourceLocatorComponent {
    skel: StarSkel,
    resource_record_cache: LruCache<Address, ResourceRecord>,
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
            ResourceLocateCall::Locate { address: identifier, tx } => {
                self.locate(identifier, tx);
            }
            ResourceLocateCall::ExternalLocate {
                address: identifier,
                star,
                tx,
            } => {
                self.external_locate(identifier, star, tx).await;
            }
            ResourceLocateCall::Found(record) => {
                self.resource_address_to_key
                    .put(record.stub.address.clone(), record.stub.key.clone());
                self.resource_record_cache
                    .put(record.stub.key.clone(), record);
            }
        }
    }
}

impl ResourceLocatorComponent {
    fn locate(
        &mut self,
        identifier: ResourceIdentifier,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    ) {
        if self.has_cached_record(&identifier) {
            let result = match self
                .get_cached_record(&identifier)
                .ok_or("expected resource record")
            {
                Ok(record) => Ok(record),
                Err(s) => Err(Fail::Error(s.to_string()).into()),
            };

            tx.send(result).unwrap_or_default();
        } else if identifier.parent().is_some() {
            let locator_api = self.skel.resource_locator_api.clone();
            tokio::spawn(async move {
                async fn locate(
                    locator_api: ResourceLocatorApi,
                    identifier: ResourceIdentifier,
                ) -> Result<ResourceRecord, Error> {
                    let parent_record = locator_api.filter(
                        locator_api
                            .locate(
                                identifier
                                    .parent()
                                    .expect("expected this identifier to have a parent"),
                            )
                            .await,
                    )?;
                    let rtn = locator_api
                        .external_locate(identifier, parent_record.location.host)
                        .await?;

                    Ok(rtn)
                }

                tx.send(locate(locator_api, identifier).await)
                    .unwrap_or_default();
            });
        } else {
            let record = ResourceRecord::new(
                ResourceStub {
                    address: ResourcePath::root(),
                    archetype: ResourceArchetype {
                        kind: Kind::Root,
                        specific: None,
                        config: ConfigSrc::None,
                    },
                },
                StarKey::central(),
            );
            tx.send(Ok(record)).unwrap_or_default();
        }
    }

    async fn external_locate(
        &mut self,
        identifier: ResourceIdentifier,
        star: StarKey,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    ) {
        let (request, rx) = Request::new((identifier, star));
        self.request_resource_record_from_star(request).await;
        tokio::spawn(async move {
            async fn timeout(
                rx: oneshot::Receiver<Result<ResourceRecord, Fail>>,
            ) -> Result<ResourceRecord, Fail> {
                Ok(tokio::time::timeout(Duration::from_secs(15), rx).await???)
            }
            tx.send(timeout(rx).await).unwrap_or_default();
        });
    }

    fn has_cached_record(&mut self, address: &Address) -> bool {
      self.resource_record_cache.contains(address)
    }

    fn get_cached_record(&mut self, address: &Address) -> Option<ResourceRecord> {
        self.resource_record_cache.get(address).cloned()
    }

    async fn request_resource_record_from_star(
        &mut self,
        locate: Request<(Address, StarKey), ResourceRecord>,
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
                .exchange(
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
