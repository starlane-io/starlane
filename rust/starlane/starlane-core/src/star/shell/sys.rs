use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::{StarMessage, StarMessagePayload};
use crate::message::delivery::Delivery;
use crate::registry::match_kind;
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use cosmic_universe::id::ToPoint;
use cosmic_universe::particle2::particle::Details;
use cosmic_universe::hyper::{Location, ParticleRecord};
use mesh_portal::version::latest::entity::request::create::{PointSegFactory, Template};
use mesh_portal::version::latest::fail;
use mesh_portal::version::latest::id::{Point, RouteSegment};
use mesh_portal::version::latest::messaging::{Message, ReqShell};
use mesh_portal::version::latest::particle::{Status, Stub};
use mesh_portal::version::latest::security::Permissions;
use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct SysApi {
    pub tx: mpsc::Sender<SysCall>,
}

impl SysApi {
    pub fn new(tx: mpsc::Sender<SysCall>) -> Self {
        Self { tx }
    }

    pub async fn create(
        &self,
        template: Template,
        messenger: mpsc::Sender<Message>,
    ) -> Result<Stub, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(SysCall::Create {
                template,
                messenger,
                tx,
            })
            .await?;
        rx.await?
    }

    pub fn delete(&self, point: Point) {
        self.tx.try_send(SysCall::Delete(point)).unwrap_or_default();
    }

    pub fn deliver(&self, message: StarMessage) {
        self.tx
            .try_send(SysCall::Delivery(message))
            .unwrap_or_default();
    }

    pub async fn get_record(&self, point: Point) -> Result<ParticleRecord, Error> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(SysCall::GetRecord { point, tx }).await;
        rx.await?
    }
}

pub enum SysCall {
    Create {
        template: Template,
        messenger: mpsc::Sender<Message>,
        tx: oneshot::Sender<Result<Stub, Error>>,
    },
    Delete(Point),
    Delivery(StarMessage),
    GetRecord {
        point: Point,
        tx: oneshot::Sender<Result<ParticleRecord, Error>>,
    },
}

impl Call for SysCall {}

pub struct SysComponent {
    counter: AtomicU64,
    skel: StarSkel,
    map: HashMap<Point, SysResource>,
}

impl SysComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<SysCall>) {
        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                map: HashMap::new(),
                counter: AtomicU64::new(0),
            }),
            skel.sys_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<SysCall> for SysComponent {
    async fn process(&mut self, call: SysCall) {
        match call {
            SysCall::Create {
                mut template,
                messenger,
                tx,
            } => {
                template.point.parent.route = RouteSegment::Fabric(self.skel.info.key.to_string());

                tx.send(handle(self, template, messenger));

                fn handle(
                    sys: &mut SysComponent,
                    template: Template,
                    messenger: mpsc::Sender<Message>,
                ) -> Result<Stub, Error> {
                    match template.point.child_segment_template {
                        PointSegFactory::Exact(exact) => {
                            let point: Point = template.point.parent.clone();
                            let point = point.push(exact)?;
                            if sys.map.contains_key(&point) {
                                return Err("sys particle already exists with that point".into());
                            }

                            let stub = Stub {
                                point: point.clone(),
                                kind: match_kind(&template.kind)?,
                                status: Status::Unknown,
                            };

                            let resource = SysResource {
                                stub: stub.clone(),
                                tx: messenger,
                            };

                            sys.map.insert(point.clone(), resource);
                            return Ok(stub);
                        }
                        PointSegFactory::Pattern(pattern) => {
                            let pattern: String = pattern;
                            if !pattern.contains("%") {
                                return Err("pattern must contain one '%' char".into());
                            }
                            let point: Point = template.point.parent.clone();
                            loop {
                                let index = sys.counter.fetch_add(1, Ordering::Relaxed);
                                let exact = pattern.replace("%", index.to_string().as_str());
                                let point = point.push(exact)?;
                                if !sys.map.contains_key(&point) {
                                    let stub = Stub {
                                        point: point.clone(),
                                        kind: match_kind(&template.kind)?,
                                        status: Status::Unknown,
                                    };

                                    let resource = SysResource {
                                        stub: stub.clone(),
                                        tx: messenger,
                                    };

                                    sys.map.insert(point.clone(), resource);
                                    return Ok(stub);
                                }
                            }
                        }
                    }
                }
            }
            SysCall::Delete(point) => {
                self.map.remove(&point);
            }
            SysCall::Delivery(message) => {
                if let StarMessagePayload::Request(request) = &message.payload {
                    match self.map.get(&request.to) {
                        Some(resource) => {
                            resource.tx.send(Message::Req(request.clone())).await;
                        }
                        None => {}
                    }
                } else if let StarMessagePayload::Response(response) = &message.payload {
                    match self.map.get(&response.to) {
                        Some(resource) => {
                            resource.tx.send(Message::Resp(response.clone())).await;
                        }
                        None => {}
                    }
                }
            }
            SysCall::GetRecord { point, tx } => match self.map.get(&point) {
                None => {
                    tx.send(Err("not found".into()));
                }
                Some(resource) => {
                    let record = ParticleRecord {
                        details: Details {
                            stub: resource.stub.clone(),
                            properties: Default::default(),
                        },
                        location: Location::Somewhere(self.skel.info.key.clone().to_point()),
                    };
                    tx.send(Ok(record));
                }
            },
        }
    }
}

impl SysComponent {}

pub struct SysResource {
    pub stub: Stub,
    pub tx: mpsc::Sender<Message>,
}
