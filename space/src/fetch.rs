use thiserror::Error;

#[derive(Error, Debug, strum_macros::Display)]
pub enum FetchErr {
    NotFound,
}
