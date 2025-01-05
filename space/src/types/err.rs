use thiserror::Error;
use crate::kind::Specific;
use crate::types::TypeKind;

#[derive(Clone, Debug, Error)]
pub enum TypeErr {
    #[error("type unknown: '{0}'")]
    Unknown(String),
    #[error("empty Meta Layers for TypeKind '{0}' (Meta requires at least 1 Layer to be defined")]
    EmptyMeta(TypeKind),
    #[error("{kind} Meta::by_layer({tried}) index out of bounds because exceeds layers length {len}")]
    MetaLayerIndexOutOfBounds{ kind: TypeKind, tried: usize, len: usize,  },
    #[error("specific '{specific} not found in '{search_location}'")]
    SpecificNotFound{ search_location: String ,specific: Specific  },
}

impl TypeErr {
    pub fn unknown(src: impl ToString) -> Self {
        Self::Unknown(src.to_string())
    }

    pub fn empty_meta(k: TypeKind) -> Self {
        Self::EmptyMeta(k)
    }

    pub fn meta_layer_index_out_of_bounds(kind: &TypeKind, tried: &usize, len: usize) -> Self {

        let kind = kind.clone();
        let tried = tried.clone();
        Self::MetaLayerIndexOutOfBounds {kind, tried, len}
    }

    pub fn specific_not_found(specific: Specific, search_location: String) -> Self {
        Self::SpecificNotFound {search_location, specific}
    }
}