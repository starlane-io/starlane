use thiserror::Error;
#[derive(Debug, Error)]
pub enum ProviderErr {
  #[error("StateErr")]
  StateErr,
}

