use crate::Kind;

#[derive(Clone)]
pub enum MountKind {
    Control,
    Portal,
}

impl MountKind {
    pub fn kind(&self) -> Kind {
        match self {
            MountKind::Control => Kind::Control,
            MountKind::Portal => Kind::Portal,
        }
    }
}
