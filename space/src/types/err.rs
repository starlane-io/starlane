use strum_macros::Display;
use thiserror::Error;
use crate::types::specific::Specific;
use crate::types::Abstract;

#[derive(Clone, Debug, Error)]
pub enum TypeErr {
    #[error("type unknown: '{0}'")]
    Unknown(String),
    #[error("empty Meta Layers for TypeKind '{0}' (Meta requires at least 1 Layer to be defined")]
    EmptyMeta(Abstract),
    #[error("{kind} Meta::by_layer({tried}) index out of bounds because exceeds layers length {len}")]
    MetaLayerIndexOutOfBounds{ kind: Abstract, tried: usize, len: usize,  },
    #[error("specific '{specific} not found in '{search_location}'")]
    SpecificNotFound{ search_location: String ,specific: Specific  },
}

impl TypeErr {
    pub fn unknown(src: impl ToString) -> Self {
        Self::Unknown(src.to_string())
    }

    pub fn empty_meta(k: Abstract) -> Self {
        Self::EmptyMeta(k)
    }

    pub fn meta_layer_index_out_of_bounds(kind: &Abstract, tried: &usize, len: usize) -> Self {

        let kind = kind.clone();
        let tried = tried.clone();
        Self::MetaLayerIndexOutOfBounds {kind, tried, len}
    }

    pub fn specific_not_found(specific: Specific, search_location: String) -> Self {
        Self::SpecificNotFound {search_location, specific}
    }
}