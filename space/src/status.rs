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
pub trait Entity: Send + Sync {
    type Kind: Eq+PartialEq+Hash+ToString;

    fn kind(&self) -> &Self::Kind;
}

/// [StatusWatcher] is type bound to [tokio::sync::watch::Receiver<StatusResult>]) can get the realtime
/// [StatusDetail] of a [StatusProbe] by polling: [StatusWatcher::borrow] or by listening for
/// changes vi [StatusWatcher::changed]
pub type StatusWatcher = tokio::sync::watch::Receiver<StatusResult>;
pub type StatusReporter = tokio::sync::watch::Sender<StatusResult>;

/// get a [StatusWatcher] via [StatusReporter::subscribe]
pub fn status_reporter() -> StatusReporter {
    tokio::sync::watch::channel(StatusResult::default()).0
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
    async fn probe(&self) -> StatusResult;
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
#[async_trait]
pub trait EntityReadier: StatusProbe {
    type Entity: Entity + Send + Sync + ?Sized;

    /// takes steps to bring [Self::Entity] to a [Status::Ready] state. For a resource [Entity] such
    /// as a network artifact (a remote file), readying steps might be downloading and storing
    /// a copy to the local filesystem. For a fully managed service [Entity] such as a Postgres
    /// service managed by [DockerDaemonFoundation] the [EntityReadier] may take many
    /// steps such as: triggering and awaiting [ProviderKind::DockerDaemon] to reach [Status::Ready],
    /// pulling a postgres docker image, starting the postgres docker image... .
    async fn ready(&self) -> EntityResult<Self::Entity>;
}

/// [Handle] contains [E]--which implements the [Entity] trait--and a private
/// `hold` reference which is a [tokio::sync::mpsc::Sender] created from the [StatusProbe]'s
/// internal `runner`.  The Runner should stay alive until it has no more hold references
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

    pub fn status(&self) -> StatusResult {
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
                reporter.send(StatusResult::Ready).unwrap();
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
    async fn probe(&self) -> StatusResult {
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
    Hash,
    Eq,
    PartialEq,
    Serialize,
    Deserialize,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum Status {
    /// [Status::Unknown] is the default status
    Unknown,
    /// [Status::Offline] is a healthy state of [StatusProbe] that indicates not [Status::Ready]
    /// because the [StatusProbe::start] action has not been requested by the host
    Offline,
    /// the [StatusProbe] is waiting on a prerequisite condition to be true before it can
    /// return to [Status::Initializing] state and complete the [StatusProbe::start]
    Pending,
    /// meaning the [StatusProbe] is healthy and is working towards reaching a
    /// [Status::Ready] state... i.e. `readying` itself
    Initializing,
    /// [StatusProbe::start] procedure has been halted by a problem that the [StatusProbe]
    /// understands and perhaps can supply an [ActionRequest] so an external [Actor] can
    /// remedy the situation.  An example: a Database depends on a DatabaseConnectionPool
    /// to be [Status::Ready] state (in this case the actual Database is managed externally
    /// and is stopped) so the status is set to [Status::Blocked] prompting the host to
    /// see if there are any [ActionRequest]s from [StatusDetail]
    Blocked,
    /// the [Entity]s actually [Status] cannot be determined because it cannot
    /// be reached over the network.
    Unreachable,
    /// A non-fatal error occurred that [StatusProbe] does not comprehend.  Panic signals that 
    /// no more attempts will be made to remedy the situation.  The entity must be `unpanicked` 
    /// in order for trying to resume.
    Panic,
    /// the [StatusProbe] reports that it cannot go on... [Status::Fatal] is a suggestion
    /// leaving the [StatusProbe]'s [EntityReadier] with the choice to: delete and recreate the
    /// [StatusProbe], abort its [EntityReadier::ready] attempt or kill the entire process
    /// with an error code
    Fatal,
    /// the desired state 
    Ready,
}

impl Default for Status {
    fn default() -> Self {
        Status::Unknown
    }
}

/// The verbose details of a [StatusProbe]'s [StatusDetail]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusDetail {
    pub status: Status,
    pub stage: StageDetail,
    pub action: ActionDetail,
}

impl Into<Status> for StatusDetail {
    fn into(self) -> Status {
        self.status
    }
}

impl Into<StatusResult> for StatusDetail {
    fn into(self) -> StatusResult {
        let stage: Stage = self.stage.stage();
        match stage {
            Stage::Ready => StatusResult::Ready,
            _ => StatusResult::NotReady(self),
        }
    }
}

impl Default for StatusDetail {
    fn default() -> Self {
        Self {
            status: Status::default(),
            stage: StageDetail::default(),
            action: ActionDetail::default(),
        }
    }
}

impl StatusDetail {
    pub fn new(status: Status, stage: StageDetail, action: ActionDetail) -> Self {
        Self {
            status,
            stage,
            action,
        }
    }
}

#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(State))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum StateDetail {
    /// before the [Provider] [State] is queried and reported by the [Provider::synchronize]
    /// the state is not known
    Unknown,
    /// stage progression is halted until the depdendency conditions of [StateDetail::Pending]
    /// are rectified
    Pending(PendingDetail),
    /// [StatusProbe] is halted described by [StateErrDetail]
    Error(StateErrDetail),
    /// [StatusProbe] is `provisioning` meaning it is progressing through it's [StageDetail] variants
    ///
    Provisioning,
    /// [StatusProbe] is ready to be used
    Ready,
}

/// [StageDetail] describes the [StatusProbe]'s presently reached life cycle stage.
/// The [StageDetail] should progress through the variants in order although
/// a [Provider] implementation may skip states that are not relative... such as
/// the fetching and caching of an external config which would skip over the
/// [StageDetail::Installed] stage.
#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Stage))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum StageDetail {
    /// [StatusDetail]
    Unknown,
    ///
    None,
    /// the meaning of [StageDetail::Cached] differs by implementation. It's most base meaning
    /// signifies that all fetching/downloading stages have completed... and of course
    /// some providers don't have a cached stage at all
    Cached,
    /// [StatusProbe] is installed
    Installed,
    /// [StatusProbe] has completed its Initialize
    Initialized,
    Started,
    /// [StatusProbe] is ready to be used
    Ready,
}

impl StageDetail {
    pub fn stage(&self) -> Stage {
        self.clone().into()
    }
}

impl Default for StageDetail {
    fn default() -> Self {
        StageDetail::Unknown
    }
}

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AwaitCondition {
    ActionRequest(ActionRequest),
}

#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Action))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum ActionDetail {
    /// Idle is the nominal action state in two cases:
    /// 1. the Status Entity
    Idle,
    /// the ntity is attempting to make the [StatusProbe] model match the external resource
    /// that the [StatusProbe] represent (this can mean changing the [StatusProbe] to match
    /// the external model or changing the external model to match the [StatusProbe]...
    /// the synchronizing strategy differs by implementation use case
    Synchronizing,
    Fetching,

    /// a vector of [PendingDetail] a set of [AwaitCondition] and [ActionRequest] describing
    /// why the actions are halted
    Pending(Vec<PendingDetail>),
    /// performing initial setup (in the case of a Database the sql schema (tables, indices, etc.)
    /// are being created
    Initializing,
    /// Attempting to start the [StatusProbe] AFTER it has been initialized... If the status
    /// [StatusProbe] is, for example a Postgres cluster instance managed by the Starlane foundation
    /// this action is being performed after the postgres start command has been issued until
    /// it is ready to accept requests
    Starting,
}

impl Default for ActionDetail {
    fn default() -> Self {
        Self::Idle
    }
}

/// The results of a `Status Probe` which may contain a `Status` or a `StatusErr`
/// if for some reason the probe fails
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Probe<S> {
    Ok(S),
    Unreachable,
}

/// stores a variety of Report each of which should be able to generate a colorful terminal
/// message (and maybe in the future as HTML)
#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ReportType))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))] // information explaining why the StatusItem is in a [ActionDetail::Pending] state
pub enum Report {
    Pending(PendingDetail),
}

/*
/// information explaining why the StatusItem is in a [ActionDetail::Pending] state
#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Pending))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum PendingDetail {
    /// a vector of [AwaitCondition] that must be satisfied before [StatusDetail] can move
    /// from [Stage::Pending]
    Awaiting(AwaitCondition),
    ActionRequest(ActionRequest),
}

 */

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingDetail {
    request: Vec<ActionRequest>,
    conditions: Vec<AwaitCondition>,
}

/// a remedy action request for an [Actor] external to the [StatusProbe] (usually a flesh and
/// blood human being)...  The [StatusProbe] cannot perform this remedy on its own and
/// making it therefore reliant on an external actor
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionRequest {
    pub actor: Actor,
    pub title: String,
    pub description: String,
    pub items: Vec<ActionItem>,
}

impl ActionRequest {
    pub fn new(actor: Actor, title: String, description: String) -> Self {
        Self {
            actor,
            title,
            description,
            items: vec![],
        }
    }

    pub fn add(&mut self, item: ActionItem) {
        self.items.push(item);
    }

    pub fn print(&self) {}
}

impl Display for ActionRequest {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ACTION REQUEST: ")?;
        f.write_str(&self.title)?;
        f.write_str("\n")?;
        f.write_str(&self.description)?;
        f.write_str("\n")?;
        f.write_str(format!("ITEMS: {} required action items...", self.items.len()).as_str())?;
        f.write_str("\n")?;
        for (index, item) in self.items.iter().enumerate() {
            f.write_str(format!("{} -> {}", index.to_string(), item.title).as_str())?;

            if let Some(ref web) = item.website {
                f.write_str("\n")?;
                f.write_str(format!(" more info: {}", web).as_str())?;
            }
            f.write_str("\n")?;
            f.write_str(item.details.as_str())?;
            if self.items.len() != index {
                f.write_str("\n")?;
            }
        }

        f.write_str("\n")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
pub struct ActionItem {
    pub title: String,
    #[builder(setter(into, strip_option), default)]
    pub website: Option<String>,
    pub details: String,
}

impl ActionItem {
    pub fn new(title: String, details: String) -> Self {
        Self {
            title,
            details,
            website: None,
        }
    }

    pub fn with_website(&mut self, website: String) {
        self.website = Some(website);
    }

    pub fn print(vec: &Vec<Self>) {}
}

impl Display for ActionItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.title)?;
        f.write_str("\n")?;
        if let Some(website) = &self.website {
            f.write_str("more info: ")?;
            f.write_str(website)?;
            f.write_str("\n")?;
        };

        f.write_str(&self.details)?;
        f.write_str("\n")
    }
}

///
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

/// a convenience result in cases where teh [Status::Entity] host
/// wants to know if it is [Status::Ready] and only cares about the
/// [StatusDetail] if it is not ready.
#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(StatusResultKind))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum StatusResult {
    Ready,
    NotReady(StatusDetail),
}

impl Default for StatusResult {
    fn default() -> Self {
        StatusResult::NotReady(StatusDetail::default())
    }
}

impl StatusResult {
    pub fn to_res(self) -> Result<(), SpaceErr> {
        match self {
            StatusResult::Ready => Ok(()),
            StatusResult::NotReady(detail) => {
                let err: SpaceErr = detail.into();
                Err(err)
            }
        }
    }
}

impl Into<Result<(), SpaceErr>> for StatusResult {
    fn into(self) -> Result<(), SpaceErr> {
        self.to_res()
    }
}

/// Similar to [Result] [EntityResult] is a convenience enum for [StatusProbe] hosts which
/// may be responsible for keeping the [StatusProbe] in a [Status::Ready] state...
///
/// example:
/// ```
/// use starlane::status::{EntityReadier, StatusProbe};
/// use starlane::status::StatusResult;
/// use starlane::status::EntityResult;
///
/// # pub mod util {
/// #  use starlane::status::StatusDetail;
/// #  pub fn check_ready() -> bool { true }
/// #  pub fn create() -> super::super::Connection { todo!() }
/// #  pub fn generate_status_detail() -> StatusDetail { todo!() }
/// # }
/// #
/// # trait StatusEntity { }
///
/// struct Connection;
///
/// impl StatusEntity for Connection {
///    // various concrete functions  defined...
/// }
///
/// struct ConnectionFacilitator;
///
/// impl ConnectionFacilitator {
///    fn probe(&self) -> StatusResult {
///        match util::check_ready() {
///           true => StatusResult::Ready,
///           false => StatusResult::NotReady(util::generate_status_detail())
///        }
///     }
/// }
///
/// impl StatusProbe for ConnectionFacilitator {
///   async fn probe(&self) -> StatusResult {
///      // do some `probing` here
///         # todo!()
///    }
/// }
///
/// impl EntityReadier for ConnectionFacilitator {
///    type Entity = Connection;
///    async fn ready(&self) -> EntityResult<Self::Entity> {
///       if util::check_ready() {
///          EntityResult::Ready(util::create())
///       } else {
///          EntityResult::StatusErr(util::generate_status_detail())
///       }
///    }
/// }
///
/// ```

#[derive(Clone, Debug, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(EntityResultKind))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum EntityResult<E>
where
    E: Entity + Send + Sync + ?Sized,
{
    Ready(Arc<E>),
    StatusErr(StatusDetail),
}

impl<E> EntityResult<E>
where
    E: Entity + Send + Sync + ?Sized,
{
    pub fn to_res(self) -> Result<Arc<E>, SpaceErr> {
        match self {
            EntityResult::Ready(entity) => Ok(entity),
            EntityResult::StatusErr(detail) => {
                let err = detail.into();
                Err(err)
            }
        }
    }
}

impl<E> Into<Result<Arc<E>, SpaceErr>> for EntityResult<E>
where
    E: Entity + Send + Sync + ?Sized,
{
    fn into(self) -> Result<Arc<E>, SpaceErr> {
        self.to_res()
    }
}
impl<E> Into<StatusDetail> for EntityResult<E>
where
    E: Entity + Clone + Send + Sync + ?Sized,
{
    fn into(self) -> StatusDetail {
        match self {
            EntityResult::Ready(_) => StatusDetail::default(),
            EntityResult::StatusErr(status) => status,
        }
    }
}

#[cfg(feature = "test")]
#[cfg(test)]
pub mod test {
    use crate::status::{Entity, Handle};
    use std::ops::Deref;


    #[test]
    pub fn compiles() { }

}
