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
use mysql::uuid::Uuid;
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use mesh_portal_serde::version::latest::id::Address;
use mesh_portal_serde::version::latest::messaging::{Message, ProtoResponse, Request, Response};
use mesh_portal_serde::version::latest::util::unique_id;
use mesh_portal_versions::version::v0_0_1::parse::Res;
use tokio::sync::oneshot::Sender;

#[derive(Clone)]
pub struct MessagingApi {
    pub tx: mpsc::Sender<MessagingCall>,
}

impl MessagingApi {
    pub fn new(tx: mpsc::Sender<MessagingCall>) -> Self {
        Self { tx }
    }

    pub fn message(&self, message: Message ) {
        let mut proto = ProtoStarMessage::new();
        match message {
            Message::Request(request) => {
                proto.to = ProtoStarMessageTo::Resource(request.to.clone());
                proto.payload = StarMessagePayload::Request(request);
            }
            Message::Response(response) => {
                proto.to = ProtoStarMessageTo::Resource(response.to.clone());
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

    pub async fn notify(&self, request: Request)->Result<(),Error>{
        let mut proto = ProtoStarMessage::new();
        proto.to = ProtoStarMessageTo::Resource(request.to.clone());
        proto.payload = StarMessagePayload::Request(request);
        self.star_notify(proto);
        Ok(())
    }

    pub async fn exchange(&self, request: Request)->Result<Response,Error>{
        let (tx,rx) = oneshot::channel();
        let call = MessagingCall::ExchangeRequest{ request, tx };
        self.tx.send(call).await;
        Ok(rx.await?)
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


    pub fn on_response(&self, response: Response ) {
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
    ExchangeRequest{request: Request, tx: oneshot::Sender<Response> },
    Response(Response)
}

impl Call for MessagingCall {}

pub struct MessagingComponent {
    skel: StarSkel,
    exchanges: HashMap<MessageId, MessageExchanger>,
    resource_exchange: HashMap<String, oneshot::Sender<Response>>,
    address: Address
}

impl MessagingComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<MessagingCall>) {
        let address = Address::from_str(format!("<<{}>>::messaging",skel.info.key.to_string()).as_str() ).expect("expected messaging address to parse");
        AsyncRunner::new(
            Box::new(Self {
                skel: skel.clone(),
                exchanges: HashMap::new(),
                resource_exchange: Default::default(),
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
            MessagingCall::FailExchange { id, proto, fail } => {
                self.fail_exchange(id, proto, fail);
            }

            MessagingCall::ExchangeRequest { request, tx } => {
               self.resource_exchange.insert( request.id.clone(), tx );
               let mut proto = ProtoStarMessage::new();
               proto.to=ProtoStarMessageTo::Resource(request.to.clone());
               proto.payload = StarMessagePayload::Request(request);
               self.send(proto).await;
            }
            MessagingCall::Response(response) => {
                match self.resource_exchange.remove(&response.response_to) {
                    None => {}
                    Some(tx) => {
                        tx.send(response);
                    }
                }
            }
        }
    }
}

impl MessagingComponent {

    /*
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

     */



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
                            "unexpected response. expected: {} found: {} for message exchange with description: {}",
                            message.payload.to_string(),
                            exchanger.expect.to_string(),
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

    fn fail_exchange(&mut self, id: MessageId, proto: ProtoStarMessage, fail: Error) {
        if let Option::Some(exchanger) = self.exchanges.remove(&id) {
            exchanger.tx.send(Err(fail.into()));
        }
        if let StarMessagePayload::Request(request) = &proto.payload {
            if let Option::Some(exchanger) = self.resource_exchange.remove(&request.id) {
                let response = Response {
                    id: unique_id(),
                    to: request.from.clone(),
                    from: self.skel.info.address.clone(),
                    core: request.core.not_found(),
                    response_to: request.id.clone()
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
                ProtoStarMessageTo::Resource(address) => {
                    let record = match skel.resource_locator_api.locate(address.clone()).await {
                        Ok(record) => record,
                        Err(fail) => {
                            eprintln!("{}", fail.to_string());
                            error!(
                                "locator could not find resource record for: '{}'",
                                address.to_string()
                            );
                            skel.messaging_api.fail_exchange(id.clone(), proto, fail.into());
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
                        .fail_exchange(id, proto, "invalid proto message".into() );
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
