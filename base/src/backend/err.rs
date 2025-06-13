use thiserror::Error;
use crate::backend::Backend;

#[derive(Debug,Error)]
pub enum Error<B> where B: Backend{
    /// meaning any `backend` error that the backend generates... for example if
    /// a request is issued to create a new database this error should provide a useful
    /// message to help the administrator resolve the problem.
    Handler(B::Result::Error),
    /// an unexpected error that the backend encountered not anticipated by this backend
    System(#[from] Box<dyn std::error::Error>)
}