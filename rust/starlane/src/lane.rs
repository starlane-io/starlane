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

    pub async fn evolve(mut self, star: Option<Id>) ->Result<Lane,Error>
    {
        self.tx.send(StarGram::StarLaneProtocolVersion(STARLANE_PROTOCOL_VERSION)).await;

        if let Option::Some(star)=star
        {
            self.tx.send(StarGram::ReportStarId(star)).await;
        }

        // first we confirm that the version is as expected
        let recv = self.rx.recv().await;

        match recv
        {
            Some(StarGram::StarLaneProtocolVersion(version)) if version == STARLANE_PROTOCOL_VERSION => {
                // do nothing... we move onto the next step
            },
            Some(StarGram::StarLaneProtocolVersion(version)) => {
                return Err(format!("wrong version: {}",version).into());},
            Some(gram) => {
                return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)",gram).into());}
            None => {
                return Err("disconnected".into());},
        }

        match self.rx.recv().await
        {
            Some(StarGram::ReportStarId(remote_star_id))=>{
                return Ok( Lane {
                    remote_star: remote_star_id,
                    tx: self.tx,
                    rx: self.rx
                });
            },
            Some(gram) => {return Err(format!("unexpected star gram: {} (expected to receive StarLaneProtocolVersion first)",gram).into());}
            None => {return Err("disconnected".into());},
        };


    }
}

pub struct Lane
{
    pub remote_star: Id,
    pub tx: Sender<StarGram>,
    pub rx: Receiver<StarGram>
}

#[cfg(test)]
mod test
{
    use crate::lane::local_lane;
    use crate::gram::StarGram;
    use crate::id::Id;
    use tokio::runtime::Runtime;
    use crate::error::Error;
    use futures::FutureExt;



        #[test]
   pub fn test()
   {

       let rt = Runtime::new().unwrap();
       rt.block_on(async {
           let star1id  =     Id::new(0,0);
           let star2id  =     Id::new(0,2);
           let (p1,p2) = local_lane();
           let future1 = p1.evolve(Option::Some( star1id ));
           let future2 = p2.evolve(Option::Some( star2id ));
           let (result1,result2) = join!( future1, future2 );

           assert!(result1.is_ok());
           assert!(result2.is_ok());
       });



   }
}
