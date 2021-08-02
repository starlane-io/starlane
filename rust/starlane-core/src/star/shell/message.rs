use tokio::sync::{mpsc, oneshot};
use crate::message::resource::ProtoMessage;
use crate::message::{ProtoStarMessage, Fail, MessageId, ProtoStarMessageTo, MessageExpect};
use crate::util::{Call, AsyncRunner, AsyncProcessor};
use crate::star::StarSkel;
use crate::frame::{Reply, ReplyKind, StarMessage};
use tokio::time::Duration;
use std::collections::HashMap;
use tokio::time::Instant;


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

    pub fn exchange( &self, message: ProtoStarMessage, expect: ReplyKind ) -> Result<Reply,Fail> {
       let (tx, rx) = oneshot::channel();
       let call = MessagingCall::Exchange { message, expect, tx };
       self.tx.try_send(call)?;
       tokio::time::timeout(Duration::from_secs(15), rx).await??
    }
}

pub enum MessagingCall {
    Send(ProtoStarMessage),
    Exchange{ message: ProtoStarMessage, expect: ReplyKind, tx: oneshot::Sender<Result<Reply,Fail>> },
    TimeoutExchange(MessageId)
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
                self.send(message);
            }
            MessagingCall::Exchange { message, expect: ReplyKind, tx } => {
                self.exchange(message,expect,tx);
            }
            MessagingCall::TimeoutExchange(id) => {
                self.cancel_exchange(id);
            }
        }
    }
}

impl MessagingComponent {

    fn cancel_exchange( &mut self, id: MessageId ) {
        if let Option::Some(exchanger) = self.exchanges.remove(&id) {
            exchanger.tx.send( Fail::Timeout )
        }
    }

    fn send( &self, message: ProtoStarMessage) {
        let id = MessageId::new_v4();
        self.send_with_id(message,id);
    }

    fn exchange( &mut self, mut proto: ProtoStarMessage, expect: ReplyKin, tx: oneshot::Sender<Result<Reply,Fail>> ) {
        let id = MessageId::new_v4();
        if let MessageExpect::Reply(kind) = &proto.expect {
            self.exchanges.insert(id.clone(), MessageExchanger::new(kind,tx));
            let messaging_tx = self.skel.messaging_api.tx.clone();
            let cancel_id = id.clone();
            tokio::spawn( async move {
                tokio::time::sleep_until(Instant::new().checked_add(Duration::from_secs(15)).expect("expected to be able to add 15 seconds"));
                messaging_tx.try_send(MessagingCall::TimeoutExchange(cancel_id)).unwrap_or_default();
            });
        }
        self.send_with_id(message,id);
    }

    fn send_with_id( &self, mut proto: ProtoStarMessage, id: MessageId  ) {

        match &proto.to {
            ProtoStarMessageTo::None => {
                return Err("ProtoStarMessage to address cannot be None".into());
            }
            ProtoStarMessageTo::Resource(ident) => {
                let record = self.skel.resource_locator_api.locate(ident.clone() ).await?;
                proto.to = record.location.host.into();
            }
            _ => {}
        };

        proto.validate()?;

        let message = StarMessage {
            id: id,
            from: self.skel.info.key.clone(),
            to: star.clone(),
            payload: proto.payload,
            reply_to: proto.reply_to,
            trace: false,
            log: proto.log,
        };

        self.skel.router_api.route(message)?;
    }
}

struct MessageExchanger {
    pub expected: ReplyKind,
    pub tx: oneshot::Sender<Result<Reply,Fail>>
}

impl MessageExchanger {
    pub fn new( expected: ReplyKind, tx: oneshot::Sender<Result<Reply,Fail>>) -> Self {
        Self{
            expected,
            tx
        }
    }
}