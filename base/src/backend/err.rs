use thiserror::Error;
use crate::backend::Backend;

#[derive(Debug,Error,strum_macros::Display)]
pub enum Error {
    /// meaning any `backend` error that the backend generates... for example if
    /// a request is issued to create a new database this error should provide a useful
    /// message to help the administrator resolve the problem.
    // disable for now
    // Handler(B::Result),
    /// an unexpected error that the backend encountered not anticipated by this backend
    System(#[from] Box<dyn std::error::Error>)
}