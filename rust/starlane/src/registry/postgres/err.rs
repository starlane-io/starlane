use crate::err::StarErr;
use crate::hyperspace::err::{ErrKind, HyperErr};
use strum::ParseError;

pub trait PostErr: HyperErr + From<sqlx::Error> + From<ParseError> {
    fn dupe() -> Self;
}

impl PostErr for StarErr {
    fn dupe() -> Self {
        Self {
            kind: ErrKind::Dupe,
            message: "Dupe".to_string(),
        }
    }
}
