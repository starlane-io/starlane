use crate::err::ParseErrs0;
use crate::fetch::FetchErr;
use async_trait::async_trait;
use std::fmt::Display;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
use strum_macros::EnumDiscriminants;

/// represents a cache for a given `type` i.e. [Cache<Id=Full,Entity=BindConfig>] ...
pub trait Cache {
  type Id;
  type Entity;

  fn get(&self, id: &Self::Id ) -> Option<&Self::Entity>;
}

pub trait ArtifactLoc: Eq + Hash + Send + Sync + Clone { }

#[derive(EnumDiscriminants, strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(StageDisc))]
#[strum_discriminants(derive(
  Hash,
  strum_macros::EnumString,
  strum_macros::ToString,
  strum_macros::IntoStaticStr
))]
pub enum Stage<Id,Entity> where Id: ArtifactLoc,
{
  /// default starting state
  Unknown,
  /// The fetch mechanism is `fetching (downloading?)` the artifact as data
  Fetching,
  /// post fetch steps are being performed.
  /// In the case of the [Package] [Cache] [Stage::Processing] would involve
  /// unzipping and organizing of the contents of the [Package] in the local
  /// filesystem
  Processing,
  /// the raw data of the [Artifact] is available as a file in the file storage
  /// and is presently being loaded into memory
  Loading,
  /// at this point the artifact's raw data is loaded into memory but may require
  /// a transformation step before the [Entity] is ready.  The most common example
  /// is an [Artifact] that requires parsing.  i.e. `Vec<u8>` -> `BindConf`
  Raw,
  /// The [Cache] [Artifact]'s end
  Ready(Artifact<Id,Entity>),
}


impl <Id,Entity> Clone for Stage<Id,Entity> where Id: ArtifactLoc
{
  fn clone(&self) -> Self {
     match self {
       Stage::Unknown => Stage::Unknown,
       Stage::Fetching => Stage::Fetching,
       Stage::Processing => Stage::Processing,
       Stage::Loading => Stage::Loading,
       Stage::Raw => Stage::Raw,
       Stage::Ready(artifact) => Stage::Ready(artifact.clone())
     }
  }
}


impl <Id,Entity> Default for Stage<Id, Entity> where Id: ArtifactLoc
{
  fn default() -> Self {
    Stage::Unknown
  }
}

#[derive(Debug)]
pub struct Artifact<Id,Entity> where Id: ArtifactLoc
{
  id: Id,
  entity: Arc<Entity>,
}

impl<Id,Entity> Clone for Artifact<Id,Entity> where Id: ArtifactLoc
{
  fn clone(&self) -> Self {
     Self {
       id: self.id.clone(),
       entity: self.entity.clone()
     }
  }
}

impl <Id,Entity> Artifact<Id,Entity>  where Id: ArtifactLoc
{
  fn new(id: Id, entity: Entity) -> Self {
     Self { id, entity: Arc::new(entity), }
  }
}

impl <Id,Entity> Deref for Artifact<Id,Entity> where Id: ArtifactLoc
{
  type Target = Arc<Entity>;

  fn deref(&self) -> &Self::Target {
    & self.entity
  }
}

#[derive(thiserror::Error, Debug)]
pub enum CacheErr {
  /// an error occurred while trying to Fetch
  #[error("Fetch Error: {0}")]
  Fetch(FetchErr),
  #[error("Parse Errors: {0}")]
  Parse(ParseErrs0),
  #[error("File Store Error: {0}")]
  FileStore(String)
}

#[async_trait]
pub trait Watcher where Self::Id: ArtifactLoc
{
  type Id;
  type Entity;

  fn stage(&self) -> Result<Stage<Self::Id,Self::Entity>,CacheErr>;

  async fn get(&self, id: &Self::Id ) -> Result<Artifact<Self::Id,Self::Entity>, CacheErr>;

  /// return if [Stage::Ready]... else return [Option::None]
  fn try_ready(&self, id: Self::Id) -> Result<Option<Artifact<Self::Id,Self::Entity>>, CacheErr> {
     if let Stage::Ready(artifact) = self.stage()? {
        Ok(Some(artifact))
     } else {
        Ok(None)
     }
  }
}


/*
pub struct ArtifactWatcher<Id,Entity> where Id: ArtifactId {
  pub id: Id,
  pub entity: Entity,
  stage: watch::Receiver<Stage<Id,Entity>>,
}

impl <Id,Entity> ArtifactWatcher<Id,Entity> where Id: ArtifactId {
  pub fn stage(&mut self) -> Stage<Id,Entity> {
    self.stage.borrow().clone()
  }

  /// sure, it's weird to have a `mut self` here, but remember the [ArtifactWatcher]
  /// is created anew for each request
  pub async fn get(&mut self) -> Result<Artifact<Id,Entity>,SpaceErr> {
    if let Stage::Ready(artifact) = self.stage() {
      Ok(artifact)
    } else {
     loop {
       self.stage.changed().await?;
       if let Stage::Ready(artifact) = self.stage() {
         return Ok(artifact);
       }
     }
    }
  }

}

 */





