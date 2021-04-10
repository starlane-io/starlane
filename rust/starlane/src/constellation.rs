use std::sync::Arc;

use futures::future::{err, join_all, ok};
use futures::prelude::*;

use crate::error::Error;
use crate::id::Id;
use crate::proto::{local_lanes, ProtoLane, ProtoStar};
use crate::star::{Star, StarKey};

pub struct Constellation
{
    pub stars: Vec<Arc<Star>>
}

#[cfg(test)]
mod test
{
    use tokio::runtime::Runtime;


    #[test]
    pub fn test()
    {




    }
}
