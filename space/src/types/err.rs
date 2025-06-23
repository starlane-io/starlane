use strum_macros::Display;
use thiserror::Error;
use crate::types::def::SpecificSliceLoc;
use crate::types::specific::SpecificLoc;
use crate::types::Type;

#[derive(Clone, Debug, Error)]
pub enum TypeErr {
    #[error("type unknown: '{0}'")]
    Unknown(String),
    #[error("empty Meta Layers for TypeKind '{0}' (Meta requires at least 1 Layer to be defined")]
    EmptyMeta(Type),
    #[error("{kind} Meta::by_layer({tried}) index out of bounds because exceeds layers length {len}")]
    MetaLayerIndexOutOfBounds{ kind: Type, tried: usize, len: usize,  },
    #[error("specific '{specific} not found in '{search_location}'")]
    SpecificNotFound{ search_location: String ,specific: SpecificSliceLoc },
}

impl TypeErr {
    pub fn unknown(src: impl ToString) -> Self {
        Self::Unknown(src.to_string())
    }

    pub fn empty_meta(k: Type) -> Self {
        Self::EmptyMeta(k)
    }

    pub fn meta_layer_index_out_of_bounds(kind: &Type, tried: &usize, len: usize) -> Self {

        let kind = kind.clone();
        let tried = tried.clone();
        Self::MetaLayerIndexOutOfBounds {kind, tried, len}
    }

    pub fn specific_not_found(specific: SpecificSliceLoc, search_location: String) -> Self {
        Self::SpecificNotFound {search_location, specific}
    }
}