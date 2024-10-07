use core::borrow::Borrow;
use std::ops::Deref;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use crate::space::artifact::asynch::{ArtifactPipeline, ArtifactHub};
use crate::space::err::SpaceErr;
use crate::space::loc::ToSurface;
use crate::space::point::Point;
use crate::space::substance::Bin;

pub mod asynch;
pub mod builtin;

pub struct ArtRef<A> {
    artifact: Arc<A>,
    pub point: Point,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl <A> Clone for ArtRef<A> {
    fn clone(&self) -> Self {
        /// cloning indicates a usage event...
        self.tx.try_send(()).unwrap_or_default();
        Self {
            artifact: self.artifact.clone(),
            point: self.point.clone(),
            tx: self.tx.clone()
        }
    }
}

impl<A> ArtRef<A> {
    fn new(artifact: A, point: Point, tx: tokio::sync::mpsc::Sender<()>) -> Self {
        Self { artifact, point, tx }
    }
}





impl<A> ArtRef<A>
where
    A: Clone,
{
    pub fn contents(&self) -> A {
        (*self.artifact).clone()
    }
}

impl<A> ArtRef<A> {
    pub fn bundle(&self) -> Point {
        self.point.clone().to_bundle().unwrap()
    }
    pub fn point(&self) -> &Point {
        &self.point
    }
}

impl<A> Deref for ArtRef<A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        self.tx.try_send(()).unwrap_or_default();
        &self.artifact
    }
}

impl<A> Drop for ArtRef<A> {
    fn drop(&mut self) {
        //
    }
}

pub struct FetchErr {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub point: Point,
    pub bin: Bin,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRequest {
    pub point: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactResponse {
    pub to: Point,
    pub payload: Bin,
}
