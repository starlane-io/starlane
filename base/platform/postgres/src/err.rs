use starlane_space::err::ParseErrs;

#[derive(Debug, Clone, thiserror::Error)]
pub enum PostErr {
    #[error("{0}")]
    ParseErrs(#[from] ParseErrs),
}