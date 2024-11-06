use thiserror::Error;
use crate::space::err::SpaceErr;

/// Can be a long-running goal being executed consists of Tasks comprised of Steps
pub trait Operation where Self: Sized {
    fn new<C>( config: &C) -> Result<Self,OpErr>;
}

pub trait OperationConfig where Self::Operation: Operation {
    type Operation;

}



pub trait Task {
    fn name() -> &'static str;

    fn desc() -> &'static str;
}

pub trait Step {
    fn name() -> &'static str;
    fn desc() -> &'static str;
}


#[derive(Clone,Debug,Error)]
pub enum OpErr {
    #[error("wrong kind of '{kind}' expected '{expected}' found: '{0}'")]
    WrongType{ kind: &'static str, expected: &'static str, found: &'static str }
}
