use serde::{Deserialize, Serialize};

use crate::space::util::ValueMatcher;
use crate::space::wave::core::hyper::HypMethod;

#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    PartialEq,
    Hash,
    strum_macros::Display,
    strum_macros::EnumString,
)]
pub enum CmdMethod {
    Init,
    Read,
    Update,
    Bounce,
    Knock,
    Greet,
    Command,
    RawCommand,
    Log,
}
impl Default for CmdMethod{
    fn default() -> Self {
        Self::Init
    }
}

impl ValueMatcher<CmdMethod> for CmdMethod {
    fn is_match(&self, x: &CmdMethod) -> Result<(), ()> {
        if *x == *self {
            Ok(())
        } else {
            Err(())
        }
    }
}
