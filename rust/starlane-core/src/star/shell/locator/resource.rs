use async_trait::async_trait;
use core::option::Option;
use core::option::Option::{None, Some};
use core::result::Result;
use core::result::Result::{Err, Ok};
use core::time::Duration;

use lru::LruCache;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use starlane_resources::ResourceIdentifier;

use crate::frame::{RegistryAction, Reply, ReplyKind, SimpleReply, StarMessagePayload};
use crate::message::{Fail, ProtoStarMessage};
use crate::resource::{
    ResourceAddress, ResourceArchetype, ResourceKey, ResourceKind, ResourceRecord, ResourceStub,
    ResourceType,
};
use crate::star::{
    LogId, Request, ResourceRegistryBacking, Set, Star, StarCommand, StarKey, StarKind, StarSkel,
};
use crate::util::{AsyncProcessor, AsyncRunner, Call};

#[derive(Clone)]
pub struct ResourceLocatorApi {
    pub tx: mpsc::Sender<ResourceLocateCall>,
}

impl ResourceLocatorApi {
    pub fn new(tx: mpsc::Sender<ResourceLocateCall>) -> Self {
        Self { tx }
    }

    pub async fn fetch_resource_key(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<ResourceKey, Fail> {
        match self.locate(identifier).await {
            Ok(record) => Ok(record.stub.key),
            Err(fail) => Err(fail),
        }
    }

    pub async fn fetch_resource_address(
        &self,
        identifier: ResourceIdentifier,
    ) -> Result<ResourceAddress, Fail> {
        match self.locate(identifier).await {
            Ok(record) => Ok(record.stub.address),
            Err(fail) => Err(fail),
        }
    }

    pub async fn locate(&self, identifier: ResourceIdentifier) -> Result<ResourceRecord, Fail> {
        let (tx, mut rx) = oneshot::channel();
        self.tx
            .send(ResourceLocateCall::Locate { identifier, tx })
            .await
            .unwrap_or_default();

        //Ok(tokio::time::timeout( Duration::from_secs(15), rx).await???)
        let rtn = tokio::time::timeout(Duration::from_secs(15), rx).await???;
        Ok(rtn)
    }

    pub async fn external_locate(
        &self,
        identifier: ResourceIdentifier,
        star: StarKey,
    ) -> Result<ResourceRecord, Fail> {
        let (tx, mut rx) = oneshot::channel();
        self.tx
            .send(ResourceLocateCall::ExternalLocate {
                star,
                identifier,
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
        identifier: ResourceIdentifier,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    },
    ExternalLocate {
        identifier: ResourceIdentifier,
        star: StarKey,
        tx: oneshot::Sender<Result<ResourceRecord, Fail>>,
    },
    Found(ResourceRecord),
}

impl Call for ResourceLocateCall {}

pub struct ResourceLocatorComponent {
    skel: StarSkel,
    resource_record_cache: LruCache<ResourceKey, ResourceRecord>,
    resource_address_to_key: LruCache<ResourceAddress, ResourceKey>,
}

impl ResourceLocatorComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<ResourceLocateCall>) {
        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                resource_address_to_key: LruCache::new(1024),
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
            ResourceLocateCall::Locate { identifier, tx } => {
                self.locate(identifier, tx);
            }
            ResourceLocateCall::ExternalLocate {
                identifier,
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
                Err(s) => Err(Fail::Error(s.to_string())),
            };

            tx.send(result).unwrap_or_default();
        } else if identifier.parent().is_some() {
            let locator_api = self.skel.resource_locator_api.clone();
            tokio::spawn(async move {
                async fn locate(
                    locator_api: ResourceLocatorApi,
                    identifier: ResourceIdentifier,
                ) -> Result<ResourceRecord, Fail> {
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
                    key: ResourceKey::Root,
                    address: ResourceAddress::root(),
                    archetype: ResourceArchetype {
                        kind: ResourceKind::Root,
                        specific: None,
                        config: None,
                    },
                    owner: None,
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
                tokio::time::timeout(Duration::from_secs(15), rx).await??
            }
            tx.send(timeout(rx).await).unwrap_or_default();
        });
    }

    fn has_cached_record(&mut self, identifier: &ResourceIdentifier) -> bool {
        match identifier {
            ResourceIdentifier::Key(key) => self.resource_record_cache.contains(key),
            ResourceIdentifier::Address(address) => {
                let key = self.resource_address_to_key.get(address);
                match key {
                    None => false,
                    Some(key) => self.resource_record_cache.contains(key),
                }
            }
        }
    }

    fn get_cached_record(&mut self, identifier: &ResourceIdentifier) -> Option<ResourceRecord> {
        match identifier {
            ResourceIdentifier::Key(key) => self.resource_record_cache.get(key).cloned(),
            ResourceIdentifier::Address(address) => {
                let key = self.resource_address_to_key.get(address);
                match key {
                    None => Option::None,
                    Some(key) => self.resource_record_cache.get(key).cloned(),
                }
            }
        }
    }

    async fn request_resource_record_from_star(
        &mut self,
        locate: Request<(ResourceIdentifier, StarKey), ResourceRecord>,
    ) {
        let (identifier, star) = locate.payload.clone();
        let mut proto = ProtoStarMessage::new();
        proto.to = star.clone().into();
        proto.payload = StarMessagePayload::ResourceManager(RegistryAction::Find(identifier));
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
