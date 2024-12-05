use thiserror::Error;
use crate::base::foundation;

#[derive(Error, Clone, Debug)]
pub enum PlatformErr{
    #[error("{0}")]
    Msg(String)
}