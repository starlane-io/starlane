use tokio::sync::{mpsc, oneshot};
use crate::message::resource::ProtoMessage;
use crate::message::{ProtoStarMessage, Fail, MessageId, ProtoStarMessageTo, MessageExpect};
use crate::util::{Call, AsyncRunner, AsyncProcessor};
use crate::star::StarSkel;
use crate::frame::{Reply, ReplyKind, StarMessage, StarMessagePayload, SimpleReply};
use tokio::time::Duration;
use std::collections::HashMap;
use tokio::time::Instant;
use tokio::sync::mpsc::error::TrySendError;
use crate::resource::ResourceRecord;
use crate::error::Error;


#[derive(Clone)]
pub struct MessagingApi {
    pub tx: mpsc::Sender<MessagingCall>,
}

impl MessagingApi {
    pub fn new(tx: mpsc::Sender<MessagingCall> ) -> Self {
        Self {
            tx,
        }
    }

    pub fn send( &self, message: ProtoStarMessage ) {
        self.tx.try_send(MessagingCall::Send(message)).unwrap_or_default();
    }

    pub async fn exchange(&self, proto: ProtoStarMessage, expect: ReplyKind, description: &str ) -> Result<Reply,Fail> {
       let (tx, rx) = oneshot::channel();
       let call = MessagingCall::Exchange { proto, expect, description: description.to_string(), tx };
       self.tx.try_send(call)?;
       rx.await?
    }

    pub fn on_reply( &self, message: StarMessage )  {
        if message.reply_to.is_none() {
            error!("received an on_reply message which has no reply_to");
        } else {
            self.tx.try_send(MessagingCall::Reply(message)).unwrap_or_default();
        }
    }

    pub fn fail_exchange( &self, id: MessageId, fail: Fail ) {
        let call = MessagingCall::FailExchange{id,fail};
        self.tx.try_send(call).unwrap_or_default();
    }
}

pub enum MessagingCall {
    Send(ProtoStarMessage),
    Exchange{ proto: ProtoStarMessage, expect: ReplyKind, tx: oneshot::Sender<Result<Reply,Fail>>, description: String  },
    TimeoutExchange(MessageId),
    FailExchange{id:MessageId,fail: Fail},
    Reply(StarMessage)
}

impl Call for MessagingCall {}

pub struct MessagingComponent {
    skel: StarSkel,
    exchanges: HashMap<MessageId,MessageExchanger>
}

impl MessagingComponent {
    pub fn start(skel: StarSkel, rx: mpsc::Receiver<MessagingCall>) {
        AsyncRunner::new(Box::new(Self { skel:skel.clone(), exchanges: HashMap::new() }), skel.messaging_api.tx.clone(), rx);

    }
}

#[async_trait]
impl AsyncProcessor<MessagingCall> for MessagingComponent {
    async fn process(&mut self, call: MessagingCall) {
        match call {
            MessagingCall::Send(proto) => {
                self.send(proto).await;
            }
            MessagingCall::Exchange { proto,expect, tx,description } => {
                self.exchange(proto,expect, tx, description).await;
            }
            MessagingCall::TimeoutExchange(id) => {
                self.timeout_exchange(id);
            }
            MessagingCall::Reply(message) => {
                self.on_reply(message);
            }
            MessagingCall::FailExchange { id, fail } => {
                self.fail_exchange(id,fail);
            }
        }
    }
}

impl MessagingComponent {

    fn on_reply( &mut self, message: StarMessage ) {
        if let Option::Some(reply_to) = message.reply_to {

            if let StarMessagePayload::Reply(SimpleReply::Ack(_)) = &message.payload {
                // do nothing, this is just an ack message
            }
            else if let Option::Some( exchanger ) = self.exchanges.remove( &reply_to ) {
                let result = match message.payload {
                    StarMessagePayload::Reply(SimpleReply::Ok(reply)) => {
                        match exchanger.expect.is_match(&reply)
                        {
                            true => Ok(reply),
                            false => Err(Fail::expected(exchanger.expect.to_string().as_str()))
                        }
                    }
                    StarMessagePayload::Reply(SimpleReply::Fail(fail)) => {
                        Err(fail)
                    }
                    _ => {
                        error!("unexpected response for message exchange with description: {}",exchanger.description);
                        Err(Fail::expected(format!("StarMessagePayload::Reply(Reply::Ok(Reply::{}))", exchanger.expect.to_string()).as_str() ))
                    }
                };
                exchanger.tx.send(result).unwrap_or_default();
            }
        } else {
            error!("received an on_reply message which has no reply_to");
        }
    }

    fn timeout_exchange(&mut self, id: MessageId ) {
        if let Option::Some(exchanger) = self.exchanges.remove(&id) {
            exchanger.tx.send( Err(Fail::Timeout) );
        }
    }

    fn fail_exchange(&mut self, id: MessageId, fail: Fail ) {
        if let Option::Some(exchanger) = self.exchanges.remove(&id) {
            exchanger.tx.send( Err(fail) );
        }
    }

    async fn send( &self, proto: ProtoStarMessage) {
        let id = MessageId::new_v4();
        self.send_with_id(proto,id).await;
    }

    async fn exchange(&mut self, mut proto: ProtoStarMessage, expect: ReplyKind, tx: oneshot::Sender<Result<Reply,Fail>>, description: String ) {
        let id = MessageId::new_v4();
        self.exchanges.insert(id.clone(), MessageExchanger::new(expect, tx, description ));
        let messaging_tx = self.skel.messaging_api.tx.clone();
        let cancel_id = id.clone();
        tokio::spawn( async move {
          tokio::time::sleep_until(Instant::now().checked_add(Duration::from_secs(15)).expect("expected to be able to add 15 seconds"));
          messaging_tx.try_send(MessagingCall::TimeoutExchange(cancel_id)).unwrap_or_default();
        });
        self.send_with_id(proto,id).await;
    }

    async fn send_with_id( &self, mut proto: ProtoStarMessage, id: MessageId  ) {

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
                            skel.messaging_api.fail_exchange(id, fail);
                            return;
                        }
                    };
                    record.location.host
                }
                ProtoStarMessageTo::Star(star) => {
                    star.clone()
                }
            };

            match proto.validate(){
                Err(error) => {
                    skel.messaging_api.fail_exchange(id, Fail::Error("invalid proto message".to_string()));
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
    pub tx: oneshot::Sender<Result<Reply,Fail>>,
    pub description: String
}

impl MessageExchanger {
    pub fn new( expected: ReplyKind, tx: oneshot::Sender<Result<Reply,Fail>>, description: String ) -> Self {
        Self{
            expect: expected,
            tx,
            description
        }
    }
}

