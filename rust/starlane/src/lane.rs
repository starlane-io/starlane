use tokio::sync::mpsc;
use tokio::sync::mpsc::{Receiver, Sender};

use crate::error::Error;
use crate::id::Id;
use crate::message::{Command, ProtoGram};
use crate::proto::{ProtoLane, ProtoStar};
use crate::star::Star;

pub static STARLANE_PROTOCOL_VERSION :i32 = 1;

pub struct Lane
{
    pub remote_star: Id,
    pub tx: Sender<ProtoGram>,
    pub rx: Receiver<ProtoGram>
}

#[cfg(test)]
mod test
{
    use futures::FutureExt;
    use tokio::runtime::Runtime;

    use crate::error::Error;
    use crate::id::Id;
    use crate::message::ProtoGram;
    use crate::proto::local_lane;

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
