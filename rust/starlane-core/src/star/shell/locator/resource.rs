use alloc::boxed::Box;
use async_trait::async_trait;
use core::option::Option::{None, Some};
use core::option::Option;
use core::result::Result;
use core::result::Result::{Err, Ok};
use core::time::Duration;
use starlane_core::frame::{RegistryAction, Reply, SimpleReply, StarMessagePayload};
use starlane_core::message::{Fail, ProtoStarMessage};
use starlane_core::resource::{ResourceAddress, ResourceKey, ResourceRecord};
use starlane_core::star::{Request, Set, StarCommand, StarKey, StarSkel};
use starlane_core::util::{AsyncProcessor, AsyncRunner, Call};

use lru::LruCache;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use starlane_resources::ResourceIdentifier;

use crate::frame::{RegistryAction, Reply, SimpleReply, StarMessagePayload};
use crate::message::{Fail, ProtoStarMessage};
use crate::resource::{ResourceAddress, ResourceKey, ResourceRecord, ResourceType};
use crate::star::{LogId, Request, ResourceRegistryBacking, Set, Star, StarCommand, StarKey, StarKind, StarSkel};
use crate::util::{AsyncProcessor, AsyncRunner, Call};

#[derive(Clone)]
pub struct ResourceLocatorApi {
    pub tx: bounded::Sender<ResourceLocateCall>
}

impl ResourceLocatorApi {

    pub fn new(tx: bounded::Sender<ResourceLocateCall> ) -> Self {
        Self {
            tx
        }
    }

    pub async fn fetch_resource_key( &self, identifier: ResourceIdentifier ) -> Result<ResourceKey,Fail> {
        match self.locate(identifier).await {
            Ok(record) => {
                Ok(record.stub.key)
            }
            Err(fail) => {
                Err(fail)
            }
        }
    }

    pub async fn fetch_resource_address( &self, identifier: ResourceIdentifier ) -> Result<ResourceAddress,Fail> {
        match self.locate(identifier).await {
            Ok(record) => {
                Ok(record.stub.address)
            }
            Err(fail) => {
                Err(fail)
            }
        }
    }


    pub async fn locate( &self, identifier: ResourceIdentifier ) -> Result<ResourceRecord,Fail> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( ResourceLocateCall::Locate {
            identifier,
            tx
        }).await.unwrap_or_default();

        Ok(tokio::time::timeout( Duration::from_secs(15), rx).await???)
    }

    pub async fn external_locate( &self, identifier: ResourceIdentifier, star: StarKey ) -> Result<ResourceRecord,Fail> {
        let (tx,mut rx) = oneshot::channel();
        self.tx.send( ResourceLocateCall::ExternalLocate {
            star,
            identifier,
            tx
        }).await.unwrap_or_default();

        Ok(tokio::time::timeout( Duration::from_secs(15), rx).await???)
    }


    pub fn found( &self, record: ResourceRecord ) {
        tokio::spawn( async move {
            self.tx.send( ResourceLocateCall::Found(record) ).await.unwrap_or_default();
        });
    }

    pub fn filter(&self, result: Result<ResourceRecord,Fail> ) -> Result<ResourceRecord,Fail> {
        if let Result::Ok(record) = &result {
            self.found( record.clone() );
        }
        result
    }

}

pub enum ResourceLocateCall {
    Locate{ identifier: ResourceIdentifier, tx: oneshot::Sender<Result<ResourceRecord,Fail>> },
    ExternalLocate { identifier: ResourceIdentifier, star: StarKey, tx: oneshot::Sender<Result<ResourceRecord,Fail>> },
    Found(ResourceRecord)
}

impl Call for ResourceLocateCall {}

pub struct ResourceLocatorComponent {
    skel: StarSkel,
    resource_record_cache: LruCache<ResourceKey, ResourceRecord>,
    resource_address_to_key: LruCache<ResourceAddress, ResourceKey>,
}

impl ResourceLocatorComponent {
    pub fn start(skel: StarSkel, rx: bounded::Receiver<ResourceLocateCall>) {
        AsyncRunner::new(Box::new(Self { skel:skel.clone(), resource_address_to_key: LruCache::new(1024), resource_record_cache: LruCache::new(1024) }), skel.resource_locator_api.tx.clone(), rx);
    }
}

#[async_trait]
impl AsyncProcessor<ResourceLocateCall> for ResourceLocatorComponent {
    async fn process(&mut self, call: ResourceLocateCall) {
        match call {
            ResourceLocateCall::Locate { identifier, tx } => {
                self.locate(identifier,tx);
            }
            ResourceLocateCall::ExternalLocate { identifier, star, tx } => {
                self.external_locate(identifier,star,tx);
            }
            ResourceLocateCall::Found(record) => {
                self.resource_address_to_key.insert( record.stub.address.clone(), record.stub.key.clone() );
                self.resource_record_cache.insert( record.stub.key.clone(), record );
            }
        }
    }
}

impl ResourceLocatorComponent {

    fn locate( &mut self, identifier: ResourceIdentifier, tx: oneshot::Sender<Result<ResourceRecord,Fail>>) {
        if self.has_cached_record(&identifier) {
            let result = self.get_cached_record(&identifier).ok_or("expected resource record")();
            tx.send(result.into() ).unwrap_or_default();
        } else if identifier.resource_type().parent().is_some() {
            let locator_api= self.skel.locator_api.clone();
            tokio::spawn( async move {

                async fn locate(locator_api: ResourceLocatorApi, identifier: ResourceIdentifier) -> Result<ResourceRecord,Fail> {
                    let parent_record = locator_api.filter(locator_api.locate(identifier.parent().expect("expected this identifier to have a parent")).await)?;
                    Ok(locator_api.external_locate(identifier,parent_record.location.host ).await?)
                }

                tx.send(locator_api.clone().filter(locate(locator_api, identifier).await)).unwrap_or_default();

            });
        } else {
            // This is for Root
            let locator_api= self.skel.locator_api.clone();
            tokio::spawn( async move {

                async fn locate(locator_api: ResourceLocatorApi, identifier: ResourceIdentifier) -> Result<ResourceRecord,Fail> {
                    Ok(locator_api.external_locate(identifier,StarKey::central() ).await?)
                }

                tx.send(locator_api.clone().filter(locate(locator_api, identifier).await)).unwrap_or_default();

            });
        }
    }


    async fn external_locate( &mut self, identifier: ResourceIdentifier, star: StarKey, tx: oneshot::Sender<Result<ResourceRecord,Fail>> ) {
        let (request,rx) = Request::new( (identifier,star) );
        self.request_resource_record_from_star(request).await;
        tokio::spawn( async move {
            async fn timeout( rx: oneshot::Receiver<Result<ResourceRecord,Fail>> )-> Result<ResourceRecord,Fail> {
                tokio::time::timeout( Duration::from_secs(15), rx).await??
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

    fn get_cached_record(
        &mut self,
        identifier: &ResourceIdentifier,
    ) -> Option<ResourceRecord> {
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
        proto.to = star.into();
        proto.payload =
            StarMessagePayload::ResourceManager(RegistryAction::Find(identifier));
        proto.log = locate.log;
        let reply = proto.get_ok_result().await;
        self.send_proto_message(proto).await;
        let star_tx = self.skel.star_tx.clone();
        tokio::spawn(async move {
            let result = reply.await;

            if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Record(record)))) =
            result
            {
                let (set, rx) = Set::new(record);
                star_tx.send(StarCommand::ResourceRecordSet(set)).await;
                tokio::spawn(async move {
                    if let Result::Ok(record) = rx.await {
                        locate.tx.send(Ok(record));
                    } else {
                        locate.tx.send(Err(Fail::expected("ResourceRecord")));
                    }
                });
            } else if let Result::Ok(StarMessagePayload::Reply(SimpleReply::Fail(fail))) = result {
                locate.tx.send(Err(fail));
            } else {
                match result {
                    Ok(StarMessagePayload::Reply(SimpleReply::Fail(Fail::ResourceNotFound(id)))) => {
                        error!("resource not found : {}", id.to_string());
                        locate.tx.send(Err(Fail::ResourceNotFound(id) ) );
                    }

                    Ok(result) => {
                        error!("payload: {}", result );
                        locate.tx.send(Err(Fail::unexpected("Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Resource(record))))", format!("{}",result.to_string()))));

                    }
                    Err(error) => {
                        error!("{}",error.to_string());
                        locate.tx.send(Err(Fail::expected("Result::Ok(StarMessagePayload::Reply(SimpleReply::Ok(Reply::Resource(record))))")));
                    }
                }
            }
        });
    }}
