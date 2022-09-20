use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use std::sync::Arc;

use dashmap::DashMap;
use mysql::uuid::Uuid;
use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::oneshot::Sender;
use tokio::time::Duration;
use tokio::time::error::Elapsed;
use tokio::time::Instant;

use cosmic_universe::hyper::ParticleRecord;
use cosmic_universe::loc::{ToPoint, ToSurface};
use cosmic_universe::loc::StarKey;
use mesh_portal::version::latest::id::Point;
use mesh_portal::version::latest::messaging::{Message, ReqShell, RespShell};
use mesh_portal::version::latest::parse::Res;
use mesh_portal::version::latest::util::uuid;

use crate::error::Error;
use crate::fail::{Fail, StarlaneFailure};
use crate::frame::{SimpleReply, StarMessage, StarMessagePayload};
use crate::message::{
    MessageExpect, MessageId, ProtoStarMessage, ProtoStarMessageTo, Reply, ReplyKind,
};
use crate::star::StarSkel;
use crate::util::{AsyncProcessor, AsyncRunner, Call};

#[derive(Clone)]
pub struct MessagingApi {
    pub tx: mpsc::Sender<MessagingCall>,
}

impl MessagingApi {
    pub fn new(tx: mpsc::Sender<MessagingCall>) -> Self {
        Self { tx }
    }

    pub fn message(&self, message: Message) {
        let mut proto = ProtoStarMessage::new();
        match message {
            Message::Req(request) => {
                proto.to = ProtoStarMessageTo::Point(request.to.clone().to_point());
                proto.payload = StarMessagePayload::Request(request);
            }
            Message::Resp(response) => {
                proto.to = ProtoStarMessageTo::Point(response.to.clone().to_point());
                proto.payload = StarMessagePayload::Response(response);
            }
        }
        self.star_notify(proto);
    }

    pub fn star_notify(&self, message: ProtoStarMessage) {
        self.tx
            .try_send(MessagingCall::Send(message))
            .unwrap_or_default();
    }

    pub async fn star_exchange(
        &self,
        proto: ProtoStarMessage,
        expect: ReplyKind,
        description: &str,
    ) -> Result<Reply, Error> {
        let (tx, rx) = oneshot::channel();
        let call = MessagingCall::Exchange {
            proto,
            expect,
            description: description.to_string(),
            tx,
        };
        self.tx.send(call).await?;
        rx.await?
    }

    pub async fn notify(&self, request: ReqShell) -> Result<(), Error> {
        let mut proto = ProtoStarMessage::new();
        proto.to = ProtoStarMessageTo::Point(request.to.clone().to_point());
        proto.payload = StarMessagePayload::Request(request);
        self.star_notify(proto);
        Ok(())
    }

    pub async fn request(&self, request: ReqShell) -> RespShell {
        let (tx, rx) = oneshot::channel();
        let call = MessagingCall::ExchangeRequest {
            request: request.clone(),
            tx,
        };
        self.tx.send(call).await;
        match tokio::time::timeout(Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => response,
            _ => {
                let response = request.fail("timeout".to_string().as_str());
                response
            }
        }
    }

    pub fn on_reply(&self, message: StarMessage) {
        if message.reply_to.is_none() {
            error!("received an on_reply message which has no reply_to");
        } else {
            self.tx
                .try_send(MessagingCall::Reply(message))
                .unwrap_or_default();
        }
    }

    pub fn on_response(&self, response: RespShell) {
        let call = MessagingCall::Response(response);
        self.tx.try_send(call).unwrap_or_default();
    }

    pub fn fail_exchange(&self, id: MessageId, proto: ProtoStarMessage, fail: Error) {
        let call = MessagingCall::FailExchange { id, proto, fail };
        self.tx.try_send(call).unwrap_or_default();
    }
}

pub enum MessagingCall {
    Send(ProtoStarMessage),
    Exchange {
        proto: ProtoStarMessage,
        expect: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Error>>,
        description: String,
    },
    TimeoutExchange(MessageId),
    FailExchange {
        id: MessageId,
        proto: ProtoStarMessage,
        fail: Error,
    },
    Reply(StarMessage),
    ExchangeRequest {
        request: ReqShell,
        tx: oneshot::Sender<RespShell>,
    },
    Response(RespShell),
}

impl Call for MessagingCall {}

pub struct MessagingComponent {
    inner: Arc<MessagingComponentInner>,
}

pub struct MessagingComponentInner {
    pub skel: StarSkel,
    pub exchanges: DashMap<MessageId, MessageExchanger>,
    pub resource_exchange: DashMap<String, oneshot::Sender<RespShell>>,
    pub point: Point,
}

impl MessagingComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<MessagingCall>) {
        let point =
            Point::from_str(format!("<<{}>>::messaging", skel.info.key.to_string()).as_str())
                .expect("expected messaging address to parse");
        let inner = Arc::new(MessagingComponentInner {
            skel: skel.clone(),
            exchanges: DashMap::new(),
            resource_exchange: DashMap::default(),
            point,
        });
        AsyncRunner::new(Box::new(Self { inner }), skel.messaging_api.tx.clone(), rx);
    }
}

#[async_trait]
impl AsyncProcessor<MessagingCall> for MessagingComponent {
    async fn process(&mut self, call: MessagingCall) {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            match call {
                MessagingCall::Send(proto) => {
                    inner.send(proto).await;
                }
                MessagingCall::Exchange {
                    proto,
                    expect,
                    tx,
                    description,
                } => {
                    inner.exchange(proto, expect, tx, description).await;
                }
                MessagingCall::TimeoutExchange(id) => {
                    inner.timeout_exchange(id);
                }
                MessagingCall::Reply(message) => {
                    inner.on_reply(message);
                }
                MessagingCall::FailExchange { id, proto, fail } => {
                    inner.fail_exchange(id, proto, fail);
                }

                MessagingCall::ExchangeRequest { request, tx } => {
                    inner.resource_exchange.insert(request.id.clone(), tx);
                    let mut proto = ProtoStarMessage::new();
                    proto.to = ProtoStarMessageTo::Point(request.to.clone().to_point());
                    proto.payload = StarMessagePayload::Request(request);
                    inner.send(proto).await;
                }
                MessagingCall::Response(response) => {
                    match inner.resource_exchange.remove(&response.reflection_of) {
                        None => {}
                        Some((_, tx)) => {
                            tx.send(response);
                        }
                    }
                }
            }
        });
    }
}

impl MessagingComponentInner {
    fn on_reply(&self, message: StarMessage) {
        if let Option::Some(reply_to) = message.reply_to {
            if let StarMessagePayload::Reply(SimpleReply::Ack(_)) = &message.payload {
                if let Option::Some(exchanger) = self.exchanges.get(&reply_to) {
                    exchanger
                        .timeout_tx
                        .try_send(TimeoutCall::Extend)
                        .unwrap_or_default();
                }
            } else if let Option::Some((_, exchanger)) = self.exchanges.remove(&reply_to) {
                exchanger
                    .timeout_tx
                    .try_send(TimeoutCall::Done)
                    .unwrap_or_default();
                let result = match message.payload {
                    StarMessagePayload::Reply(SimpleReply::Ok(reply)) => {
                        match exchanger.expect == reply.kind() {
                            true => Ok(reply),
                            false => {
                                Err(format!("expected: {}", exchanger.expect.to_string()).into())
                            }
                        }
                    }
                    StarMessagePayload::Reply(SimpleReply::Fail(fail)) => Err("fail".into()),

                    _ => {
                        error!(
                            "unexpected response. expected: {} found: {} for message exchange with description: {}",
                            message.payload.to_string(),
                            exchanger.expect.to_string(),
                            exchanger.description
                        );
                        Err(format!(
                            "StarMessagePayload::Reply(Reply::Ok(Reply::{}))",
                            exchanger.expect.to_string()
                        )
                        .into())
                    }
                };
                exchanger.tx.send(result).unwrap_or_default();
            }
        } else {
            error!("received an on_reply message which has no reply_to");
        }
    }

    fn timeout_exchange(&self, id: MessageId) {
        if let Option::Some((_, exchanger)) = self.exchanges.remove(&id) {
            exchanger.tx.send(Err("Fail::Timeout.into())".into()));
        }
    }

    fn fail_exchange(&self, id: MessageId, proto: ProtoStarMessage, fail: Error) {
        if let Option::Some((_, exchanger)) = self.exchanges.remove(&id) {
            exchanger.tx.send(Err(fail.into()));
        }
        if let StarMessagePayload::Request(request) = &proto.payload {
            if let Option::Some((_, exchanger)) = self.resource_exchange.remove(&request.id) {
                let response = RespShell {
                    id: uuid(),
                    to: request.from.clone(),
                    from: self.skel.info.point.clone().to_port(),
                    core: request.core.not_found(),
                    reflection_of: request.id.clone(),
                };
                exchanger.send(response);
            }
        }
    }

    async fn send(&self, proto: ProtoStarMessage) {
        let id = Uuid::new_v4().to_string();
        self.send_with_id(proto, id).await;
    }

    async fn exchange(
        &self,
        mut proto: ProtoStarMessage,
        expect: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Error>>,
        description: String,
    ) {
        let id = Uuid::new_v4().to_string();
        let (timeout_tx, mut timeout_rx) = mpsc::channel(1);
        self.exchanges.insert(
            id.clone(),
            MessageExchanger::new(expect, tx, timeout_tx, description),
        );
        let messaging_tx = self.skel.messaging_api.tx.clone();
        let cancel_id = id.clone();
        tokio::spawn(async move {
            loop {
                match tokio::time::timeout(Duration::from_secs(10), timeout_rx.recv()).await {
                    Ok(Option::Some(call)) => {
                        match call {
                            TimeoutCall::Extend => {
                                // in this case we extend the waiting time
                                info!("Extending wait time for message.");
                            }
                            TimeoutCall::Done => {
                                return;
                            }
                        }
                    }
                    Ok(Option::None) => {
                        return;
                    }
                    Err(_) => {
                        messaging_tx
                            .try_send(MessagingCall::TimeoutExchange(cancel_id.clone()))
                            .unwrap_or_default();
                    }
                }
            }
        });
        self.send_with_id(proto, id).await;
    }

    async fn send_with_id(&self, mut proto: ProtoStarMessage, id: MessageId) {
        let skel = self.skel.clone();

        tokio::spawn(async move {
            let star = match &proto.to {
                ProtoStarMessageTo::None => {
                    error!("ProtoStarMessage to address cannot be None");
                    return;
                }
                ProtoStarMessageTo::Point(point) => {
                    let record = match skel.registry_api.locate(&point).await {
                        Ok(record) => record,
                        Err(fail) => {
                            eprintln!("{}", fail.to_string());
                            error!(
                                "locator could not find particle record for: '{}'",
                                point.to_string()
                            );
                            skel.messaging_api
                                .fail_exchange(id.clone(), proto, fail.into());
                            return;
                        }
                    };
                    match record.location.ok_or() {
                        Ok(point) => match StarKey::try_from(point) {
                            Ok(star) => star,
                            Err(err) => {
                                error!("{}", err.to_string());
                                return;
                            }
                        },
                        Err(_) => {
                            error!("ProtoStarMessage to address cannot be None");
                            return;
                        }
                    }
                }
                ProtoStarMessageTo::Star(star) => star.clone(),
            };

            match proto.validate() {
                Err(error) => {
                    skel.messaging_api
                        .fail_exchange(id, proto, "invalid proto message".into());
                    return;
                }
                _ => {}
            }

            let message = StarMessage {
                id: id,
                from: skel.info.key.clone(),
                to: star,
                payload: proto.payload,
                reply_to: proto.reply_to,
                trace: false,
                log: proto.log,
            };

            skel.router_api.route(message).unwrap_or_default();
        });
    }
}

pub struct MessageExchanger {
    pub expect: ReplyKind,
    pub tx: oneshot::Sender<Result<Reply, Error>>,
    pub timeout_tx: mpsc::Sender<TimeoutCall>,
    pub description: String,
}

pub enum TimeoutCall {
    Extend,
    Done,
}

impl MessageExchanger {
    pub fn new(
        expected: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Error>>,
        timeout_tx: mpsc::Sender<TimeoutCall>,
        description: String,
    ) -> Self {
        Self {
            expect: expected,
            tx,
            timeout_tx,
            description,
        }
    }
}
