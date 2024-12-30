use crate::config::Document;
use crate::loc::ToSurface;
use crate::point::Point;
use crate::substance::Bin;
use core::borrow::Borrow;
use serde_derive::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::Arc;

pub mod asynch;
pub mod builtin;

#[derive(Debug)]
pub struct ArtRef<A> {
    artifact: Arc<A>,
    pub point: Point,
    tx: tokio::sync::mpsc::Sender<()>,
}

impl ArtRef<Document> {}

unsafe impl<A> Send for ArtRef<A> {}

unsafe impl<A> Sync for ArtRef<A> {}

impl<A> Clone for ArtRef<A> {
    fn clone(&self) -> Self {
        /// cloning indicates a usage event...
        self.tx.try_send(()).unwrap_or_default();
        Self {
            artifact: self.artifact.clone(),
            point: self.point.clone(),
            tx: self.tx.clone(),
        }
    }
}

impl<A> ArtRef<A> {
    fn new(artifact: A, point: Point, tx: tokio::sync::mpsc::Sender<()>) -> Self {
        let artifact = Arc::new(artifact);
        Self {
            artifact,
            point,
            tx,
        }
    }
}

impl<A> ArtRef<A> {
    pub fn contents(&self) -> Arc<A> {
        self.artifact.clone()
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
