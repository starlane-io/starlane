use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};
use tokio::sync::mpsc::error::TrySendError;
use tokio::time::Duration;
use tokio::time::error::Elapsed;
use tokio::time::Instant;

use crate::error::Error;
use crate::frame::{ SimpleReply, StarMessage, StarMessagePayload};
use crate::message::{MessageExpect, ProtoStarMessage, ProtoStarMessageTo, MessageId, ReplyKind, Reply};
use crate::resource::ResourceRecord;
use crate::star::{StarSkel, StarKey};
use crate::util::{AsyncProcessor, AsyncRunner, Call};
use crate::fail::{Fail, StarlaneFailure};
use crate::resources::message::{ProtoRequest, MessageFrom};
use crate::mesh::Response;
use mesh_portal_serde::version::v0_0_1::messaging::ExchangeType;
use crate::mesh::serde::messaging::Exchange;
use mysql::uuid::Uuid;
use std::convert::TryInto;
use crate::mesh::serde::id::Address;
use std::str::FromStr;

#[derive(Clone)]
pub struct MessagingApi {
    pub tx: mpsc::Sender<MessagingCall>,
}

impl MessagingApi {
    pub fn new(tx: mpsc::Sender<MessagingCall>) -> Self {
        Self { tx }
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

    pub async fn notify(&self, mut proto: ProtoRequest)->Result<(),Error>{
        proto.exchange = ExchangeType::Notification;

        let (tx,rx) = oneshot::channel();
        let call = MessagingCall::Request {proto ,tx };
        self.tx.send(call).await?;

        Ok(())
    }


    pub async fn exchange(&self, mut proto: ProtoRequest)->Result<Response,Error>{
        proto.exchange = ExchangeType::RequestResponse;

        let (tx,rx) = oneshot::channel();
        let call = MessagingCall::Request {proto ,tx };
        self.tx.send(call).await?;
        Ok(rx.await??.ok_or("expectect response")?)
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

    pub fn fail_exchange(&self, id: MessageId, fail: Error) {
        let call = MessagingCall::FailExchange { id, fail };
        self.tx.try_send(call).unwrap_or_default();
    }
}

pub enum MessagingCall {
    Send(ProtoStarMessage),
    Request {
        proto: ProtoRequest,
        tx: oneshot::Sender<Result<Option<Response>, Error>>,
    },
    Exchange {
        proto: ProtoStarMessage,
        expect: ReplyKind,
        tx: oneshot::Sender<Result<Reply, Error>>,
        description: String,
    },
    TimeoutExchange(MessageId),
    FailExchange {
        id: MessageId,
        fail: Error,
    },
    Reply(StarMessage),
}

impl Call for MessagingCall {}

pub struct MessagingComponent {
    skel: StarSkel,
    exchanges: HashMap<MessageId, MessageExchanger>,
    address: Address
}

impl MessagingComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<MessagingCall>) {
        let address = Address::from_str(format!("<<{}>>::messaging",skel.info.key.to_string()).as_str() ).expect("expected messaging address to parse");
        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                exchanges: HashMap::new(),
                address
            }),
            skel.messaging_api.tx.clone(),
            rx,
        );
    }
}

#[async_trait]
impl AsyncProcessor<MessagingCall> for MessagingComponent {
    async fn process(&mut self, call: MessagingCall) {
        match call {
            MessagingCall::Send(proto) => {
                self.send(proto).await;
            }
            MessagingCall::Exchange {
                proto,
                expect,
                tx,
                description,
            } => {
                self.exchange(proto, expect, tx, description).await;
            }
            MessagingCall::TimeoutExchange(id) => {
                self.timeout_exchange(id);
            }
            MessagingCall::Reply(message) => {
                self.on_reply(message);
            }
            MessagingCall::FailExchange { id, fail } => {
                self.fail_exchange(id, fail);
            }
            MessagingCall::Request { proto, tx } => {
                self.request( proto, tx ).await;
            }
        }
    }
}

impl MessagingComponent {

    async fn request( &mut self, proto: ProtoRequest, tx: oneshot::Sender<Result<Option<Response>,Error>>) {
        async fn process( messaging: &mut MessagingComponent, proto: ProtoRequest ) ->Result<Option<Response>,Error> {
            let mut proto = proto;
            let exchange_type = proto.exchange.clone();
            if let Option::Some(MessageFrom::Inject) = proto.from {
                proto.from = Option::Some(MessageFrom::Address(messaging.address.clone()));
            }
            let request = proto.create()?;
            let mut proto = ProtoStarMessage::new();
            proto.to = ProtoStarMessageTo::Resource(request.to.clone());
            proto.payload = StarMessagePayload::Request(request.clone());
            let (tx, rx) = oneshot::channel();
            messaging.exchange(proto, ReplyKind::Response, tx, "resource request exchange".to_string() );
            match exchange_type {
                ExchangeType::Notification => {
                    Ok(Option::None)
                }
                ExchangeType::RequestResponse => {
                    let response = rx.await??;
                    if let Reply::Response(response) = response {
                        Ok(Option::Some(response))
                    } else {
                        Err("unexpected reply".into())
                    }
                }
            }
        }
        tx.send(process(self, proto).await );
    }



    fn on_reply(&mut self, message: StarMessage) {
        if let Option::Some(reply_to) = message.reply_to {
            if let StarMessagePayload::Reply(SimpleReply::Ack(_)) = &message.payload {
                if let Option::Some(exchanger) = self.exchanges.get(&reply_to) {
                    exchanger
                        .timeout_tx
                        .try_send(TimeoutCall::Extend)
                        .unwrap_or_default();
                }
            } else if let Option::Some(exchanger) = self.exchanges.remove(&reply_to) {
                exchanger
                    .timeout_tx
                    .try_send(TimeoutCall::Done)
                    .unwrap_or_default();
                let result = match message.payload {
                    StarMessagePayload::Reply(SimpleReply::Ok(reply)) => {
                        match exchanger.expect == reply.kind() {
                            true => Ok(reply),
                            false => Err(format!("expected: {}",exchanger.expect.to_string()).into()),
                        }
                    }
                    StarMessagePayload::Reply(SimpleReply::Fail(fail)) => Err("fail".into()),

                    _ => {
                        error!(
                            "unexpected response for message exchange with description: {}",
                            exchanger.description
                        );
                        Err(
                            format!(
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

    fn timeout_exchange(&mut self, id: MessageId) {
        if let Option::Some(exchanger) = self.exchanges.remove(&id) {
            exchanger.tx.send(Err("Fail::Timeout.into())".into()));
        }
    }

    fn fail_exchange(&mut self, id: MessageId, fail: Error) {
        if let Option::Some(exchanger) = self.exchanges.remove(&id) {
            exchanger.tx.send(Err(fail.into()));
        }
    }

    async fn send(&self, proto: ProtoStarMessage) {
        let id = Uuid::new_v4().to_string();
        self.send_with_id(proto, id).await;
    }

    async fn exchange(
        &mut self,
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
                ProtoStarMessageTo::Resource(ident) => {
                    let record = match skel.resource_locator_api.locate(ident.clone()).await {
                        Ok(record) => record,
                        Err(fail) => {
                            error!(
                                "locator could not find resource record: {}",
                                ident.to_string()
                            );
                            skel.messaging_api.fail_exchange(id, fail.into());
                            return;
                        }
                    };
                    match record.location.ok_or() {
                        Ok(star) => {star}
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
                        .fail_exchange(id, "invalid proto message".into() );
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

struct MessageExchanger {
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
