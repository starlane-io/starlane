use crate::gram::{StarGram, Command};
use crate::error::Error;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};
use crate::id::Id;

static STARLANE_PROTOCOL_VERSION :i32 = 1;

pub fn local_lane()->(ProtoLane,ProtoLane)
{
    let (atx,arx) = mpsc::channel::<StarGram>(32);
    let (btx,brx) = mpsc::channel::<StarGram>(32);

    (ProtoLane{
        tx: atx,
        rx: brx
    },
    ProtoLane
    {
        tx: btx,
        rx: arx
    })
}

pub struct ProtoLane
{
    pub tx: Sender<StarGram>,
    pub rx: Receiver<StarGram>
}

impl ProtoLane
{
    pub async fn evolve(mut self) ->Result<Lane,Error>
    {
        self.tx.send(StarGram::StarLaneProtocolVersion(STARLANE_PROTOCOL_VERSION));
        match self.rx.recv().await
        {
            Some(StarGram::StarLaneProtocolVersion(version)) if version == STARLANE_PROTOCOL_VERSION => {
                // do nothing... we move onto the next step
            },
            Some(StarGram::StarLaneProtocolVersion(version)) => {return Err(format!("wrong version: {}",version).into());},
            Some(gram) => {return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)",gram).into());}
            None => {return Err("disconnected".into());},
        }

        // now wait for remote ReportStarId
        match self.rx.recv().await
        {
            Some(StarGram::ReportStarId(id))=>{

                let lane = Lane{
                    remote_star: id,
                    tx: self.tx,
                    rx: self.rx
                };

                Ok(lane)
            },
            Some(gram) => {Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)",gram).into())}
            None => {Err("disconnected".into())},
        }
    }
}

pub struct Lane
{
    pub remote_star: Id,
    pub tx: Sender<StarGram>,
    pub rx: Receiver<StarGram>
}








pub trait StarGramReceiver: Sync+Send
{
    fn receive( gram: StarGram )->Result<(),Error>;
}