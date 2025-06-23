use strum_macros::Display;
use thiserror::Error;
use crate::types::specific::SpecificLoc;
use crate::types::{Absolute, Type};

#[derive(Clone, Debug, Error)]
pub enum TypeErr {
    #[error("type unknown: '{0}'")]
    Unknown(String),
    #[error("empty Meta Layers for TypeKind '{0}' (Meta requires at least 1 Layer to be defined")]
    EmptyMeta(Type),
    #[error("{kind} Meta::by_layer({tried}) index out of bounds because exceeds layers length {len}")]
    MetaLayerIndexOutOfBounds{ kind: Type, tried: usize, len: usize,  },
    #[error("absolute'{absolute} not found in '{search_location}'")]
    AbsoluteNotFound{ search_location: String ,absolute: String},
    #[error("specific '{specific} not found in '{search_location}'")]
    SpecificNotFound{ search_location: String ,specific: SpecificLoc},
    #[error("type '{ty} not found in '{search_location}'")]
    TypeNotFound{ search_location: String ,ty: Type},
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

    pub fn specific_not_found(specific: SpecificLoc, search_location: String) -> Self {
        Self::SpecificNotFound {search_location, specific}
    }

    pub fn absolute_not_found(absolute: Absolute, search_location: String) -> Self {
        let absolute = absolute.to_string();
        Self::AbsoluteNotFound {search_location, absolute}
    }


    pub fn type_not_found(ty: Type, search_location: String) -> Self {
        Self::TypeNotFound{search_location, ty}
    }
}