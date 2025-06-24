use starlane_space::err::ParseErrs0;

#[derive(Debug, Clone, thiserror::Error)]
pub enum PostErr {
    #[error("{0}")]
    ParseErrs(#[from] ParseErrs0),
}
