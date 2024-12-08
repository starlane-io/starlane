use thiserror::Error;
use crate::types::TypeKind;

#[derive(Clone, Debug, Error)]
pub enum TypeErr {
    #[error("type unknown: '{0}'")]
    Unknown(String),
    #[error("empty Meta Layers for TypeKind '{0}' (Meta requires at least 1 Layer to be defined")]
    EmptyMeta(TypeKind),
}

impl TypeErr {
    pub fn unknown(src: impl ToString) -> Self {
        Self::Unknown(src.to_string())
    }

    pub fn empty_meta(k: TypeKind) -> Self {
        Self::EmptyMeta(k)
    }
}