use thiserror::Error;

#[derive(Error, Clone, Debug)]
pub enum PlatformErr {
    #[error("{0}")]
    Msg(String)
}