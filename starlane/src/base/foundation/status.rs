use crate::base::foundation::kind::FoundationKind;
use crate::space::parse::CamelCase;
use serde::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use thiserror::Error;
use crate::base::kind::Kind;
use std::fmt::{Display, Formatter};

#[derive(Default,Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub phase: Phase,
    pub action: Action,
}

#[derive(Default,Clone, Debug, Serialize, Deserialize)]
pub struct StatusDetail {
    pub phase: PhaseDetail,
    pub action: ActionDetail,
}

impl Status {
    pub fn action(&self) -> &Action {
        &self.action
    }
}





impl Status {
    pub fn new(phase: Phase, action: Action) -> Self {
        Self { phase, action}
    }
}

impl StatusDetail {
    pub fn new(phase: PhaseDetail, action: ActionDetail) -> Self {
        Self { phase, action}
    }
}

impl Into<Status> for StatusDetail {
    fn into(self) -> Status {
        let phase = self.phase.into();
        let action = self.action.into();
        Status::new(phase, action)
    }
}


/// [`PhaseDetail`] provides more detailed information than state.  Including ActionRequired which
/// should hopefully tell the user exactly what he needs to do to resolve the issue
#[derive(Clone, Debug, Serialize, Deserialize,EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Phase))]
#[strum_discriminants(derive(Hash,Serialize,Deserialize))]
pub enum PhaseDetail {
    Unknown,
    None,
    Downloaded,
    Installed,
    Initialize,
    Started,
    Ready,
}

impl PhaseDetail {
    pub fn state(&self) -> Phase {
        self.clone().into()
    }
}


impl Default for PhaseDetail {
    fn default() -> Self {
        PhaseDetail::Unknown
    }
}

impl Default for Phase {
    fn default() -> Self {
        PhaseDetail::default().into()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Panic {
    pub foundation: FoundationKind,
    pub kind: Kind,
    pub message: String,
}



#[derive(Clone,Debug, Serialize, Deserialize)]
pub struct AwaitCondition {
    /// in case it's waiting for an actual other kind (DependencyKind or ProviderKind)
    pub kind: Option<Kind>,
    pub description: String
}

#[derive(Clone,Debug, EnumDiscriminants,Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Action))]
#[strum_discriminants(derive(Hash,Serialize,Deserialize))]
pub enum ActionDetail {
  Unknown,
  None,
  Probing,
  Pending(Vec<PendingDetail>),
  Initializing,
  Done
}
impl ActionDetail {
    pub fn state(&self) -> Action {
        self.clone().into()
    }
}



impl Default for ActionDetail {
    fn default() -> Self {
        Self::Unknown
    }
}

impl Default for Action {
    fn default() -> Self {
        ActionDetail::default().into()
    }
}

impl Panic {
    pub fn new(
        foundation: FoundationKind,
        kind: impl Into<Kind>,
        provider: Option<CamelCase>,
        message: String,
    ) -> Self {
        let kind = kind.into();
        Self {
            kind,
            foundation,
            message,
        }
    }
}


/// The results of a `Status Probe` which may contain a `Status` or a `StatusErr`
/// if for some reason the probe fails
#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum Probe<S> {
    Ok(S),
    Unreachable,
}


/// stores a variety of Report each of which should be able to generate a colorful terminal
/// message (and maybe in the future as HTML)
#[derive(Clone,Debug, EnumDiscriminants,Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(ReportType))]
#[strum_discriminants(derive(Hash,Serialize,Deserialize))]// information explaining why the StatusItem is in a [ActionDetail::Pending] state
pub enum Report{
    Pending(PendingDetail)
}


/// information explaining why the StatusItem is in a [ActionDetail::Pending] state
#[derive(Clone,Debug, EnumDiscriminants,Serialize, Deserialize)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(Pending))]
#[strum_discriminants(derive(Hash,Serialize,Deserialize))]
pub enum PendingDetail{
    /// describe a `prerequisite` that must be satisfied before [Phase::Pending]
    PreReq{ },
    ActionRequest(ActionRequest)
}


#[derive(Error,Clone,Debug,Serialize,Deserialize)]
pub enum StatusErr {

}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionRequest {
    pub title: String,
    pub description: String,
    pub items: Vec<ActionItem>,
}

impl ActionRequest {
    pub fn new(title: String, description: String) -> Self {
        Self {
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionItem {
    pub title: String,
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