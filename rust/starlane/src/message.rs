use crate::star::{StarKey, StarSearchTransaction, Transaction, TransactionResult, StarCommand};
use crate::frame::{StarMessagePayload, Frame, StarMessage, MessageAck};
use crate::error::Error;
use crate::lane::LaneMeta;
use std::cell::Cell;
use crate::id::Id;
use tokio::sync::{mpsc, oneshot, broadcast};
use tokio::sync::oneshot::Receiver;
use tokio::sync::mpsc::Sender;

pub struct ProtoMessage
{
    pub to: Option<StarKey>,
    pub payload: StarMessagePayload,
    pub transaction: Option< Id >,
    pub tx: mpsc::Sender<StarMessagePayload>,
    pub rx: Cell<mpsc::Receiver<StarMessagePayload>>,
    pub timeout_seconds: usize,
    pub retries: usize,
    pub expect: MessageExpect,
    pub reply_to: Option<Id>
}

impl ProtoMessage
{
    pub fn new()->Self
    {
        let (tx,rx) = mpsc::channel();
        ProtoMessage{
            to: Option::None,
            payload: StarMessagePayload::None,
            transaction: Option::None,
            timeout_seconds: 60,
            tx: tx,
            rx: Cell::new(rx),
            retries: 5,
            expect: MessageExpect::None,
            reply_to: Option::None
        }
    }

    pub fn validate(&self)->Result<(),Error>
    {
        let mut errors = vec!();
        if self.to.is_none()
        {
            errors.push("must specify 'to' field");
        }
        if self.payload.is_none()
        {
            errors.push("must specify a message payload");
        }

        if !errors.is_empty()
        {
            let mut rtn = String::new();
            for err in errors
            {
                rtn.push_str(err);
                rtn.push('\n');
            }
            Err(rtn.into())
        }

        Ok(())
    }
}


pub struct MessageReplyTracker
{
    pub reply_to: Id,
    pub tx: broadcast::Sender<MessageUpdate>
}

impl MessageReplyTracker
{
    pub fn on_message(&self, message: &StarMessage ) -> TrackerJob
    {
        match &message.payload {
            StarMessagePayload::Ack(ack) => {
              self.tx.send( MessageUpdate::Ack(ack.clone()) );
              TrackerJob::Continue
            }
            StarMessagePayload::Error(error) => {
              self.tx.send(MessageUpdate::Result(MessageResult::Err(error.clone())));
              TrackerJob::Done
            }
            payload => {
              self.tx.send(MessageUpdate::Result(MessageResult::Ok(payload.clone())));
              TrackerJob::Done
            }
       }
    }
}

pub enum TrackerJob
{
    Continue,
    Done
}

#[derive(Clone)]
pub enum MessageUpdate
{
    Ack(MessageAck),
    Result(MessageResult)
}

#[derive(Clone)]
pub enum MessageResult
{
    Ok(StarMessagePayload),
    Err(String),
    Timeout
}

#[derive(Clone)]
pub struct StarMessageDeliveryInsurance
{
    pub message: StarMessage,
    pub expect: MessageExpect,
    pub retries: usize,
    pub tx: broadcast::Sender<MessageUpdate>,
    pub rx: broadcast::Receiver<MessageUpdate>
}

impl StarMessageDeliveryInsurance
{
    pub fn new( message: StarMessage, expect: MessageExpect, retries: usize ) -> Self
    {
        let (tx,rx) = broadcast::channel(8);
        StarMessageDeliveryInsurance {
            message: message,
            expect: expect,
            retries: retries,
            tx: tx,
            rx: rx
        }
    }
}

#[derive(Clone)]
pub enum MessageExpect
{
    None,
    Reply
}