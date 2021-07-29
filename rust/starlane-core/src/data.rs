use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;

use crate::error::Error;
use starlane_resources::Path;

use std::future::Future;
use crate::star::StarKey;
use crate::file_access::FileAccess;
use std::alloc::Global;


pub type Binary = Arc<Vec<u8>>;
pub type DataSet<B> = HashMap<String,B>;

#[Debug,Clone,Serialize,Deserialize]
pub enum BinSize{
    Unknown,
    Size(i32)
}

pub enum BinSizeCategory{
    Small,
    Large
}

pub enum FileSpace{
    Perm,
    Temp
}

#[Debug,Clone,Serialize,Deserialize]
pub struct BinNetworkAddress {
    pub star: StarKey,
    pub filepath: String,
    pub filespace: FileSpace
}

#[Debug,Clone,Serialize,Deserialize]
pub enum BinSrc {
  Memory(Binary),
  Network{address:BinNetworkAddress, size: BinSize}
}

impl BinSrc{
    pub fn new(bin: Binary) {
        Self::Memory(bin)
    }
}

pub trait BinContext {
  fn file_access(&self) -> FileAccess;
  fn spawn<T>(&self, task: T) -> JoinHandle<T::Output>
        where
            T: Future + Send + 'static,
            T::Output: Send + 'static;

  fn is_local_star( star: StarKey ) -> bool;
}

pub struct BinTransfer{
    pub ctx: Arc<dyn BinContext>,
    pub index: i32,
    pub complete: bool
}

impl BinTransfer{
    pub fn new(ctx: Arc<dyn BinContext>) -> Self {
        Self {
            ctx,
            index: 0,
            complete: false
        }
    }
}

impl BinSrc{
    pub fn size(&self) -> BinSize{
        match self {
            BinSrc::Memory(binary) => {
                BinSize::Size(binary.len() as _)
            }
            BinSrc::Network{ address:_, size } => {
                size.clone()
            }
        }
    }

    pub fn to_bin(&self, ctx: Arc<dyn BinContext>) -> Result<Binary,Error> {
        match self {
            BinSrc::Memory(bin) => {
                bin.clone()
            }
            BinSrc::Network { .. } => {
                unimplemented!()
            }
        }
    }

    fn transfer_block(&self, transfer: &mut BinTransfer ) -> Result<Option<Vec<u8>>,Error> {
        match self {
            BinSrc::Memory(bin) => {
                if transfer.index > 0 {
                    return Ok(Option::None)
                }
                transfer.index = bin.len() as _;
                transfer.complete = true;
                Ok(Option::Some((*bin).clone()))
            }
            BinSrc::Network { .. } => {
                unimplemented!()
            }
        }
    }

    /// if the file is local (or bin is in memory) it's better to issue a move command
    pub async fn mv(&self, ctx: Arc<dyn BinContext>, path: Path, tx: tokio::sync::oneshot::Sender<Result<(),Error>> ) {
        match self {
            BinSrc::Memory(bin) => {
                tx.send(ctx.file_access().write( &path, bin ).await).unwrap_or_default();
            }
            BinSrc::Network { address,size: _ } => {
                if address.filespace == FileSpace::Temp && ctx.is_local_star(address.star.clone())
                {
                    // find some way to move a file from TMP FileAccess to Perm FileAccess
                    unimplemented!()
                }
                else
                {
                    let clone = self.clone();
                    ctx.spawn_bin_transfer(async move {
                        let mut transfer = BinTransfer::new(ctx.clone());
                        // output stream does not exist yet in filesysstem
                        // let output = ctx.file_access().output( path );
                        while !transfer.complete {
                            let result = clone.transfer_block(&mut transfer);
                            match result {
                                Ok(block) => {
                                    match block {
                                        None => { break; }
                                        Some(data) => {
                                            unimplemented!()
                                            /*
                                           match output.append(data).await{
                                              Ok(_) => {},
                                              Err(err) => {
                                                 tx.send(Result::Err(err));
                                                 return;
                                              }
                                           }
                                         */
                                        }
                                    }
                                }
                                Err(err) => {
                                    tx.send(Result::Err(err));
                                    return;
                                }
                            }
                        }
                        tx.send(Result::Ok(()));
                    })
                }
            }
        }
    }
}

