use serde::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use thiserror::Error;
use std::fmt::{Display, Formatter};
use async_trait::async_trait;
use derive_builder::Builder;
use enum_ordinalize::Ordinalize;
use starlane_space::parse::CamelCase;
use crate::point::Point;
use crate::provider::err::StateErrDetail;
use crate::wave::Agent;


pub type Watcher = tokio::sync::watch::Receiver<State>;

///  [StatusEntity] provides an interfact to query and report it's internal Status
#[async_trait]
pub trait StatusEntity {
    fn status(&self) -> Status;

    fn status_detail(&self) -> StatusDetail;

    /// return a [Watcher] which facilitates asynchronous status updates
    async fn synchronize(&self) -> Watcher;
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub stage: Stage,
    pub action: ActionDetail,
}

impl Status {
    pub fn new(stage: Stage, action: ActionDetail) -> Self {
        Self { stage, action }
    }
}

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct StatusDetail {
    pub stage: Stage,
    ///
    pub action: ActionDetail,
}



impl StatusDetail {
    pub fn new(stage: Stage, action: ActionDetail) -> Self {
        Self { stage, action }
    }
}

impl Into<Status> for StatusDetail {
    fn into(self) -> Status {
        let stage = self.stage.into();
        let action = self.action.into();
        Status::new(stage, action)
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
    /// [StatusEntity] is `provisioning` meaning it is progressing through it's [Stage] variants
    ///
    Provisioning,
    /// Status entity is ready to be used
    Ready
}





/// [Stage] describes the entity's presently reached life cycle stage.
/// The [Stage] should progress through the variants in order although
/// a [Provider] implementation may skip states that are not relative... such as
/// the fetching and caching of an external config which would skip over the
/// [Stage::Installed] stage.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Ordinalize, Deserialize, EnumDiscriminants)]
pub enum Stage {
    /// [Status]
    Unknown,
    ///
    None,
    /// the meaning of [Stage::Cached] differs by implementation. It's most common meaning
    /// signifies that all fetching/downloading stages have completed... and of course
    /// some providers don't have a cached stage at all
    Cached,
    /// [StatusEntity] is installed
    Installed,
    /// [StatusEntity] has completed its Initialize
    Initialized,
    Started,
    Ready,
}

impl Stage {
    pub fn stage(&self) -> Stage {
        self.clone().into()
    }
}


impl Default for Stage {
    fn default() -> Self {
        Stage::Unknown
    }
}

impl Default for Stage {
    fn default() -> Self {
        Stage::default().into()
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
    /// the entity is attempting to make the entity model match the external resource
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
    /// entity is, for example a Postgres cluster instance managed by the Starlane foundation
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