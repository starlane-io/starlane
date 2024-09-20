use crate::err::StarErr;
use crate::hyper::space::err::{ErrKind, HyperErr};
use starlane_space::err::SpaceErr;


impl HyperErr for StarErr {
    fn to_space_err(&self) -> SpaceErr {
        SpaceErr::server_error(self.to_string())
    }

    fn new<S>(message: S) -> Self
    where
        S: ToString,
    {
        StarErr::new(message)
    }

    fn status_msg<S>(status: u16, message: S) -> Self
    where
        S: ToString,
    {
        StarErr::new(message)
    }

    fn status(&self) -> u16 {
        if let ErrKind::Status(code) = self.kind {
            code
        } else {
            500u16
        }
    }

    fn kind(&self) -> ErrKind {
        self.kind.clone()
    }

    fn with_kind<S>(kind: ErrKind, msg: S) -> Self
    where
        S: ToString,
    {
        StarErr {
            kind,
            message: msg.to_string(),
        }
    }
}