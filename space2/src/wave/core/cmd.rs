

use crate::util::ValueMatcher;

#[derive(
    Debug,
    Clone,


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
impl Default for CmdMethod {
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
