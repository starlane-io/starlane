use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::atomic::{AtomicU64, Ordering};
use mesh_portal::version::latest::entity::request::create::{PointSegFactory, Template};
use mesh_portal::version::latest::fail;
use mesh_portal::version::latest::id::{Point, RouteSegment};
use mesh_portal::version::latest::messaging::{Message, Request};
use mesh_portal::version::latest::particle::{Stub, Status};
use tokio::sync::{mpsc, oneshot};
use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::{StarMessage, StarMessagePayload};
use crate::message::delivery::Delivery;
use crate::particle::{ParticleLocation, ParticleRecord};
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};

#[derive(Clone)]
pub struct SysApi {
    pub tx: mpsc::Sender<SysCall>,
}

impl SysApi {
    pub fn new(tx: mpsc::Sender<SysCall>) -> Self {
        Self { tx }
    }

    pub async fn create(&self, template: Template, messenger: mpsc::Sender<Message> ) -> Result<Stub, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(SysCall::Create{template,messenger,tx}).await?;
        rx.await?
    }

    pub fn delete(&self, address: Point) {
        self.tx.try_send(SysCall::Delete(address)).unwrap_or_default();
    }

    pub fn deliver(&self, message: StarMessage ) {
        self.tx.try_send(SysCall::Delivery(message)).unwrap_or_default();
    }

    pub async fn get_record(&self, address: Point) -> Result<ParticleRecord,Error>{
        let (tx,rx) = oneshot::channel();
        self.tx.send(SysCall::GetRecord{address, tx}).await;
        rx.await?
    }
}

pub enum SysCall {
    Create{ template: Template, messenger: mpsc::Sender<Message>, tx: oneshot::Sender<Result<Stub,Error>> },
    Delete(Point),
    Delivery(StarMessage),
    GetRecord{ address: Point, tx: oneshot::Sender<Result<ParticleRecord,Error>>}
}

impl Call for SysCall {}

pub struct SysComponent {
    counter: AtomicU64,
    skel: StarSkel,
    map: HashMap<Point,SysResource>
}

impl SysComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<SysCall>) {
        AsyncRunner::new(
            Box::new(Self { skel: skel.clone(), map: HashMap::new(), counter: AtomicU64::new(0) }),
            skel.sys_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<SysCall> for SysComponent {
    async fn process(&mut self, call: SysCall) {
        match call {
            SysCall::Create{ mut template, messenger, tx }  => {

                    template.address.parent.route = RouteSegment::Mesh(self.skel.info.key.to_string());

                    tx.send(handle(self, template, messenger ));

                    fn handle(sys: &mut SysComponent, template: Template, messenger: mpsc::Sender<Message>) -> Result<Stub,Error>{



                        match template.address.child_segment_template {
                            PointSegFactory::Exact(exact) => {
                                let address: Point = template.address.parent.clone();
                                let address = address.push(exact)?;
                                if sys.map.contains_key(&address) {
                                    return Err("sys particle already exists with that address".into());
                                }

                                let stub = Stub {
                                    address: address.clone(),
                                    kind: template.kind.try_into()?,
                                    properties: Default::default(),
                                    status: Status::Unknown
                                };

                                let resource = SysResource {
                                    stub: stub.clone(),
                                    tx: messenger
                                };

                                sys.map.insert(address.clone(), resource);
                                return Ok(stub);
                            }
                            PointSegFactory::Pattern(pattern) => {
                                let pattern: String = pattern;
                                if !pattern.contains("%") {
                                    return Err("pattern must contain one '%' char".into());
                                }
                                let address: Point = template.address.parent.clone();
                                loop {
                                    let index = sys.counter.fetch_add(1, Ordering::Relaxed);
                                    let exact = pattern.replace("%", index.to_string().as_str());
                                    let address = address.push(exact)?;
                                    if !sys.map.contains_key(&address) {
                                        let stub = Stub {
                                            address: address.clone(),
                                            kind: template.kind.try_into()?,
                                            properties: Default::default(),
                                            status: Status::Unknown
                                        };

                                        let resource = SysResource {
                                            stub: stub.clone(),
                                            tx: messenger
                                        };

                                        sys.map.insert(address.clone(), resource);
                                        return Ok(stub);
                                    }
                                }
                            }
                        }
                    }
                }
            SysCall::Delete(address) => {
                self.map.remove(&address);
            }
            SysCall::Delivery(message) => {
                if let StarMessagePayload::Request(request) =  &message.payload {
                    match self.map.get( &request.to ) {
                        Some(resource) => {
                            resource.tx.send(Message::Request(request.clone())).await;
                        },
                        None => {
                        }
                    }
                }
                else if let StarMessagePayload::Response(response) =  &message.payload {
                    match self.map.get( &response.to ) {
                        Some(resource) => {
                            resource.tx.send(Message::Response(response.clone())).await;
                        },
                        None => {
                        }
                    }
                }
            }
            SysCall::GetRecord { address, tx } => {
                match self.map.get( &address ) {
                    None => {
                        tx.send( Err("not found".into() ));
                    }
                    Some(resource) => {
                        let record = ParticleRecord {
                            stub: resource.stub.clone(),
                            location: ParticleLocation::Star(self.skel.info.key.clone())
                        };
                        tx.send(Ok(record));
                    }
                }
            }
        }
    }

}

impl SysComponent {

}

pub struct SysResource {
    pub stub: Stub,
    pub tx: mpsc::Sender<Message>
}