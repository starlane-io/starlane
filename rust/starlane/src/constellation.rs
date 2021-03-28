use crate::star::{Star, ProtoStar};
use std::sync::Arc;
use futures::prelude::*;
use futures::future::{join_all, ok, err};
use crate::lane::ProtoLane;
use crate::error::Error;

pub struct ProtoConstellation
{
    proto_stars: Vec<ProtoStar>
}

impl ProtoConstellation
{
    pub async fn evolve(&mut self)->Result<Constellation,Error>
    {
        let mut futures = vec![];
        for mut proto_star in self.proto_stars.drain(..)
        {
            let future = proto_star.evolve();
            futures.push(future);
        }
        let mut stars = vec![];
        for result in join_all(futures).await
        {
            let star = result?;
            stars.push(star);
        }

        Ok(Constellation{
            stars: stars
        })
    }
}

pub struct Constellation
{
    stars: Vec<Arc<Star>>
}

