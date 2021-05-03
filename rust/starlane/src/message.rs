use crate::star::{StarKey, StarSearchTransaction, Transaction, TransactionResult, StarCommand};
use crate::frame::{StarMessagePayload, Frame};
use crate::error::Error;
use crate::lane::LaneMeta;
use std::sync::mpsc;

pub struct ProtoMessage
{
    pub to: Option<StarKey>,
    pub payload: Option<StarMessagePayload>,
    pub transaction: Option< Box< dyn ProtoTransaction >>
}

impl ProtoMessage
{
    pub fn new()->Self
    {
        ProtoMessage{
            to: Option::None,
            payload: Option::None,
            transaction: Option::None
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

pub trait ProtoTransaction
{
    fn evolve(&self)->(Box<dyn Transaction>,oneshot::Receiver<()>);
}

pub struct MessageResultTransaction
{
    pub tx: oneshot::Sender<Result<(),Error>>
}

impl Transaction for MessageResultTransaction
{

    async fn on_frame(&mut self, frame: &Frame, lane: Option<&mut LaneMeta>, command_tx: &mut mpsc::Sender<StarCommand>) -> TransactionResult {
        todo!()
    }
}