use std::sync::Arc;
use strum_macros::Display;
use thiserror::Error;
use starlane_space::types::specific::File;
use crate::repo::Repo;

pub trait Cache {
    fn get(&self, file: &File) -> Result<Vec<u8>,CacheErr>;
}

#[derive(Debug,Error)]
pub enum CacheErr {
    #[error("Not Found")]
    NotFound
}