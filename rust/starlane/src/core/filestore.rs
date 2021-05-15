use crate::star::{StarComm, StarSkel};

pub struct FileStoreStarCore {
    pub skel: StarSkel,
    pub comm: StarComm
}

pub trait FileStoreBacking {

}