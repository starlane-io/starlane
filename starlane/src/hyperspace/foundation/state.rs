use crate::hyperspace::foundation::kind::{DependencyKind, FoundationKind};
use crate::space::parse::CamelCase;

#[derive(Clone, Debug)]
pub enum State {
    None,
    Downloaded,
    Installed,
    Initialized,
    Started,
    Ready,
    Panic
}

#[derive(Clone)]
pub struct Panic {
    pub foundation: FoundationKind,
    pub dependency: Option<DependencyKind>,
    pub provider: Option<CamelCase>,
    pub message: String
}

impl Panic {
    pub fn new(foundation: FoundationKind, dependency: Option<DependencyKind>, provider: Option<CamelCase>, message: String) -> Self {
        Self {
            foundation,
            dependency,
            provider,
            message,
        }
    }
}
