use crate::err::StarErr;
use crate::hyper::space::err::{ErrKind, HyperErr};
use ascii::FromAsciiError;
use bincode::ErrorKind;
use starlane_space::err::{SpaceErr, StatusErr};
use std::io::Error;
use strum::ParseError;
use tokio::sync::oneshot::error::RecvError;
use zip::result::ZipError;

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
