use crate::point::Point;
use crate::wave::Agent;
use async_trait::async_trait;
use derive_builder::Builder;
use futures::task::Spawn;
use serde_derive::{Deserialize, Serialize};
use starlane_space::err::SpaceErr;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;
use strum_macros::EnumDiscriminants;

/// [Entity] provides a utilization interface for `anything` that can be described by the [Status]
/// model be it `resource` or `service` ... anything!
///
/// `Examples:`
/// *  A `resource` such as a remote file archive that is [Status::Ready] after being downloaded
///    and cached to local storage
///
/// * A `service` such as [crate::particle::Particle]
///
/// * The [starlane_hyperspace] crate's [Provider] implements [Entity] to indicate that it is ready
///   to `provision`
///
/// [starlane_hyperspace]: ../../starlane_hyperspace
/// [Provider]: ../../starlane_hyperspace/src/provider.rs
pub trait Entity: Send + Sync + Sized {
    //fn kind(&self) -> &Self::Kind;
}

impl Entity for () {}

/// [StatusWatcher] is type bound to [tokio::sync::watch::Receiver<StatusResult>]) can get the realtime
/// [StatusDetail] of a [StatusProbe] by polling: [StatusWatcher::borrow] or by listening for
/// changes vi [StatusWatcher::changed]
pub type StatusWatcher = tokio::sync::watch::Receiver<StatusDetail>;
pub type StatusReporter = tokio::sync::watch::Sender<StatusDetail>;

/// get a [StatusWatcher] via [StatusReporter::subscribe]
pub fn status_reporter() -> StatusReporter {
    tokio::sync::watch::channel(StatusDetail::default()).0
}

/// [StatusProbe::probe] triggers the [StatusProbe::Entity] status model synchronization
/// to generate a [StatusDetail]
#[async_trait]
pub trait StatusProbe {
    /// Returns:
    /// * [StatusResult::Ready] if status is determined to be [Status::Ready]
    /// * [StatusResult::NotReady] which wraps a *hopefully* useful [StatusDetail]
    ///
    /// [StatusProbe::probe] should synchronize the internal [StatusDetail] model to
    /// describe the status of its target entity
    async fn probe(&self) -> StatusDetail {
        todo!()
    }
}

/*
pub enum ReadyFacilitator<C, U>
where
    C: provider::mode::create::ProviderConfig,
    U: provider::mode::utilize::ProviderConfig,
{
    Utilize(U),
    Control(C),
}


 */

/// trait that can bring a [StatusProbe] into a [Status::Ready] state


/// [Handle] contains [E]--which implements the [Entity] trait--and a private
/// `hold` reference which is a [tokio::sync::mpsc::Sender] created from the [StatusProbe]'s
/// internal `runner`.  The Runner should stay alive until it has no more hold references,
/// at which time it is up to the Runner to stop itself or ignore a reference count of 0
#[derive(Clone)]
pub struct Handle<E>
where
    E: Entity + Send + Sync + ?Sized,
{
    entity: Arc<E>,
    watcher: StatusWatcher,
    hold: tokio::sync::mpsc::Sender<()>,
}

impl<E> Deref for Handle<E>
where
    E: Entity + Send + Sync + ?Sized,
{
    type Target = Arc<E>;

    fn deref(&self) -> &Self::Target {
        &self.entity
    }
}

impl<E> Handle<E>
where
    E: Entity + Send + Sync,
{
    pub fn new(entity: E, watcher: StatusWatcher, hold: tokio::sync::mpsc::Sender<()>) -> Self {
        let entity = Arc::new(entity);
        Self {
            entity,
            watcher,
            hold,
        }
    }

    pub fn status(&self) -> StatusDetail {
        self.watcher.borrow().clone()
    }

    pub fn watcher(&self) -> StatusWatcher {
        self.watcher.clone()
    }

    pub fn entity(&self) -> &E {
        &(*self.entity)
    }

    ///  return a mocked version of `Handle` for testing
    #[cfg(feature = "test")]
    pub fn mock(entity: E) -> Handle<E> {
        let entity = Arc::new(entity);
        let (hold, mut hold_rx) = tokio::sync::mpsc::channel(1);
        let reporter = status_reporter();
        let watcher = reporter.subscribe();
        tokio::spawn(async move {
            /// idle =
            while let Some(_) = hold_rx.recv().await {
                reporter.send(StatusDetail::Ready).unwrap();
            }
        });

        Self {
            hold,
            watcher,
            entity,
        }
    }
}

#[async_trait]
impl<E> StatusProbe for Handle<E>
where
    E: StatusProbe + Entity + Send + Sync,
{
    async fn probe(&self) -> StatusDetail{
        self.entity.probe().await
    }
}

/// Indicate [Entity]'s internal state.
/// most importantly the desired variant of a [StatusProbe] is [Status::Ready]
/// and if that is the [Status] then there isn't a need to drill any deeper into
/// the [StatusDetail]
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumDiscriminants,
)]

#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Status))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize,strum_macros::Display))]
pub enum StatusDetail {
    /// [Status::Unknown] is the default status
    Unknown,
    /// [Status::Stopped] is a healthy state of [StatusProbe] that indicates not [Status::Ready]
    /// because the `start` action has not been requested by the host
    Stopped,
    /// the [StatusProbe] is waiting on a prerequisite condition to be true before it can
    /// return to [Status::Initializing] state and complete the [StatusProbe::start]
    Pending(PendingDetail),

    Panic(String),

    Fatal(String),
    /// the desired state
    Ready,
}

impl Default for Status {
    fn default() -> Self {
        Status::Unknown
    }
}



impl Default for StatusDetail {
    fn default() -> Self {
        Self::Unknown
    }
}



#[async_trait]
pub trait EntityReadier {
    type Entity: Entity;

    async fn ready(&self) -> EntityResult<Self::Entity> {
        todo!()
    }
}

pub type EntityResult<E> = Result<E,()>;

pub type StatusResult = Result<StatusDetail,()>;






/// [Actor] can be an [Agent], [Particle], etc.
#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ActorKind))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum Actor {
    /// referencing
    Agent(Agent),
    Particle(Point),
}

/// The results of a `Status Probe` which may contain a `Status` or a `StatusErr`
/// if for some reason the probe fails
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Probe<S> {
    Ok(S),
    Unreachable,
}



#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingDetail {
    conditions: Vec<String>,
}

#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(StateErr))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum StateErrDetail {
    /// The Panic signals an obstacle that the status [StatusProbe] doesn't know how to resolve.
    /// A Panic state indicates that the Entity has Not reached the desired
    /// [State::Ready] state and is now idle.
    ///
    /// An [StatusProbe] may recover from a Panic if the panic issue is externally resolved and then
    /// `Entity::synchronize()` is invoked trigger another try-again loop.
    Panic(String),
    /// [StateErr::Fatal] signals an error condition that cannot be recovered from.
    /// Depending upon the context of the status [StatusProbe] reporting [StateErr::Fatal] possible
    /// actions might be deleting and recreating the [StatusProbe] or shutting down the entire
    /// Starlane process
    Fatal(String),
}


#[cfg(feature = "test")]
#[cfg(test)]
pub mod test {
    use crate::status::{Entity, Handle};
    use std::ops::Deref;

    #[test]
    pub fn compiles() {}
}
