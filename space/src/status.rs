use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use thiserror::Error;
use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::sync::Arc;
use async_trait::async_trait;
use derive_builder::Builder;
use crate::point::Point;
use crate::wave::Agent;

/// [StatusWatcher] is type bound to [tokio::sync::watch::Receiver<StatusDetail>]) can get the realtime
/// [StatusDetail] of a [StatusEntity] by polling: [StatusWatcher::borrow] or by listening for
/// changes vi [StatusWatcher::changed]
pub type StatusWatcher = tokio::sync::watch::Receiver<Status>;

///  [StatusEntity] provides an interface for entities to report status
#[async_trait]
pub trait StatusEntity {
    fn status(&self) -> Status;

    fn status_detail(&self) -> StatusDetail;

    /// Returns a [StatusWatcher]
    fn status_watcher(&self) -> StatusWatcher;

    /// synchronize the [StatusEntity] [Status] with the real world properties that
    /// it models.
    async fn probe(&self) -> StatusWatcher;
}


/// [Handle] contains [E]--which implements the [StatusEntity] trait--and a private
/// `hold` reference which is a [tokio::sync::mpsc::Sender] created from the [StatusEntity]'s
/// internal `runner`.  The Runner should stay alive until it has no more hold references
/// at which time it is up to the Runner to stop itself or ignore a reference count of 0
#[derive(Clone)]
pub struct Handle<E> where E: StatusEntity {
    pub entity: Arc<E>,
    hold: tokio::sync::mpsc::Sender<()>,
}

impl<E> Handle<E> where E: StatusEntity {
    pub fn new(api: E, hold: tokio::sync::mpsc::Sender<()> ) -> Self {
        /// hopefully the [Arc::from] does what I hope it does which is only create a new
        /// [Arc<E>] if [E] is not already an instance of an [Arc<E>]
        let api = From::from(api);
        Self { entity: api, hold }
    }
}

impl <E> Deref for Handle<E> where E: StatusEntity {
    type Target = Arc<E>;

    fn deref(&self) -> &Self::Target {
        & self.entity
    }
}


/// the broad classification of a [StatusEntity]'s internal state.
/// most importantly the desired variant of a [StatusEntity] is [Status::Ready]
/// and if that is the [Status] then there isn't a need to drill any deeper into
/// the [StatusDetail]
#[derive(Clone, Hash,Eq,PartialEq, Debug, Serialize, Deserialize)]
pub enum Status {
    Unknown,
    /// [Status::Idle] is a healthy state of [StatusEntity] that indicates not [Status::Ready]
    /// because the [StatusEntity::start] action has not been requested by the host
    Idle,
    /// meaning the [StatusEntity] is healthy and is working towards reaching a
    /// [Status::Ready] state.
    Working,
    /// the [StatusEntity] is waiting on a prerequisite condition to be true before it can
    /// return to [Status::Working] state and complete the [StatusEntity::start]
    Pending,
    /// [StatusEntity::start] procedure has been halted by a problem that the [StatusEntity]
    /// understands and perhaps can supply an [ActionRequest] so an external [Actor] can
    /// remedy the situation.  An example: a Database depends on a DatabaseConnectionPool
    /// to be [Status::Ready] state (in this case the actual Database is managed externally
    /// and is stopped) so the status is set to [Status::Blocked] prompting the host to
    /// see if there are any [ActionRequest]s from [StatusDetail]
    Blocked,
    /// A non-fatal error occurred that [StatusEntity] does not compre
    Panic,
    Fatal,
    Ready
}

impl Default for Status {
    fn default() -> Self {
        Status::Unknown
    }
}

/// The verbose details of a [StatusEntity]'s [StatusDetail]
#[derive( Clone, Debug, Serialize, Deserialize)]
pub struct StatusDetail {
    pub stage: StageDetail,
    pub action: ActionDetail,
}
impl Default for StatusDetail {
    fn default() -> Self {
        Self {
            stage: StageDetail::default(),
            action: ActionDetail::default()
        }
    }
}


impl StatusDetail {
    pub fn new(stage: StageDetail, action: ActionDetail) -> Self {
        Self { stage, action }
    }
}



#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(State))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum StateDetail{
    /// before the [Provider] [State] is queried and reported by the [Provider::synchronize]
    /// the state is not known
    Unknown,
    /// stage progression is halted until the depdendency conditions of [StateDetail::Pending]
    /// are rectified
    Pending(PendingDetail),
    /// [StatusEntity] is halted described by [StateErrDetail]
    Error(StateErrDetail),
    /// [StatusEntity] is `provisioning` meaning it is progressing through it's [StageDetail] variants
    ///
    Provisioning,
    /// [StatusEntity] is ready to be used
    Ready
}





/// [StageDetail] describes the [StatusEntity]'s presently reached life cycle stage.
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
    /// the meaning of [StageDetail::Cached] differs by implementation. It's most common meaning
    /// signifies that all fetching/downloading stages have completed... and of course
    /// some providers don't have a cached stage at all
    Cached,
    /// [StatusEntity] is installed
    Installed,
    /// [StatusEntity] has completed its Initialize
    Initialized,
    Started,
    /// [StatusEntity] is ready to be used
    Ready(),
}

impl StageDetail {
    pub fn stage(&self) -> StageDetail {
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
  Particle(Point)
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AwaitCondition {
    ActionRequest(ActionRequest)
}

#[derive(Clone, Debug, EnumDiscriminants, Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Action))]
#[strum_discriminants(derive(Hash, Serialize, Deserialize))]
pub enum ActionDetail {
    /// Idle is the nominal action state in two cases:
    /// 1. the Status Entity
    Idle,
    /// the ntity is attempting to make the [StatusEntity] model match the external resource
    /// that the [StatusEntity] represent (this can mean changing the [StatusEntity] to match
    /// the external model or changing the external model to match the [StatusEntity]...
    /// the synchronizing strategy differs by implementation use case
    Synchronizing,
    Fetching,

    /// a vector of [PendingDetail] a set of [AwaitCondition] and [ActionRequest] describing
    /// why the actions are halted
    Pending(Vec<PendingDetail>),
    /// performing initial setup (in the case of a Database the sql schema (tables, indices, etc.)
    /// are being created
    Initializing,
    /// Attempting to start the [StatusEntity] AFTER it has been initialized... If the status
    /// [StatusEntity] is, for example a Postgres cluster instance managed by the Starlane foundation
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
#[strum_discriminants(
    derive(Hash, Serialize, Deserialize)
)] // information explaining why the StatusItem is in a [ActionDetail::Pending] state
pub enum Report {
    Pending(PendingDetail)
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

#[derive(Clone, Debug,  Serialize, Deserialize)]
pub struct PendingDetail {
  request: Vec<ActionRequest>,
  conditions: Vec<AwaitCondition>,
}






/// a remedy action request for an [Actor] external to the [StatusEntity] (usually a flesh and
/// blood human being)...  The [StatusEntity] cannot perform this remedy on its own and
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

    pub fn print(&self) { }
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

#[derive(Clone, Debug, Serialize, Deserialize,Builder)]
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
  /// The Panic signals an obstacle that the status [StatusEntity] doesn't know how to resolve.
  /// A Panic state indicates that the Entity has Not reached the desired
  /// [State::Ready] state and is now idle.
  ///
  /// An [StatusEntity] may recover from a Panic if the panic issue is externally resolved and then
  /// `Entity::synchronize()` is invoked trigger another try-again loop.
  Panic(String),
  /// [StateErr::Fatal] signals an error condition that cannot be recovered from.
  /// Depending upon the context of the status [StatusEntity] reporting [StateErr::Fatal`] possible
  /// actions might be deleting and recreating the [StatusEntity] or shutting down the entire
  /// Starlane process
  Fatal(String)
}