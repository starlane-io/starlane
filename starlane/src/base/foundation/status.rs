use crate::base::err::ActionRequest;
use crate::base::foundation::kind::FoundationKind;
use crate::space::parse::CamelCase;
use serde::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use crate::base::kind::Kind;

#[derive(Default,Clone, Debug, Serialize, Deserialize)]
pub struct Status {
    pub phase: PhaseDetail,
    pub action: ActionDetail,
}

impl Status {

    pub fn action(&self) -> Action {
        self.action.clone().into()
    }

}


impl Status {
    pub fn new(phase: PhaseDetail, action: ActionDetail) -> Self {
        Self { phase, action}
    }
}




impl Default for Phase {
    fn default() -> Self {
        Phase::Unknown
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
    /// Item has received a cycle request but cannot continue to next lifecycle phase until
    /// some prerequisite [`PendingConditions`] are met
    Pending,
    ActionRequest(ActionRequest),
    Ready,
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


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Condition<X> where X: ToString {
    x: X
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
  Synchronizing,
  Awaiting{ conditions: Vec<AwaitCondition> },
  Initializing,
  Done
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
