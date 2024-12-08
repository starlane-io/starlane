use std::borrow::Borrow;
use dyn_clone::clone;
use thiserror::Error;
use crate::types::TypeKind;

#[derive(Clone, Debug, Error)]
pub enum TypeErr {
    #[error("type unknown: '{0}'")]
    Unknown(String),
    #[error("empty Meta Layers for TypeKind '{0}' (Meta requires at least 1 Layer to be defined")]
    EmptyMeta(TypeKind),
    #[error("{kind} Meta::by_layer({tried}) index out of bounds because exceeds layers length {len}")]
    MetaLayerIndexOutOfBounds{ kind: TypeKind, tried: usize, len: usize,  },


}

impl TypeErr {
    pub fn unknown(src: impl ToString) -> Self {
        Self::Unknown(src.to_string())
    }

    pub fn empty_meta(k: TypeKind) -> Self {
        Self::EmptyMeta(k)
    }

    pub fn meta_layer_index_out_of_bounds(kind: &TypeKind, tried: &usize, len: impl ToOwned<Owned=usize>) -> Self {

        let kind = kind.clone();
        let tried = tried.clone();
        let len = len.to_owned();
        Self::MetaLayerIndexOutOfBounds {kind, tried, len}
    }
}