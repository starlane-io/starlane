

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
pub enum HypMethod {
    Init,
    Assign,
    Host,
    Provision,
    Knock,
    Hop,
    Transport,
    HyperWave,
    Search,
}

impl Default for HypMethod {
    fn default() -> Self {
        Self::Init
    }
}

// this need ei
/*
impl HypMethod {

    pub fn as_str(&self) -> & 'static str {
        match self {
            HypMethod::Init => "Hyp<Init>",
            HypMethod::Assign => "Hyp<Assign>",
            HypMethod::Host => "Hyp<Host>",
            HypMethod::Provision => "Hyp<Provision>",
            HypMethod::Knock => "Hyp<Knock>",
            HypMethod::Hop => "Hyp<Hop>",
            HypMethod::Transport => "Hyp<Transport>",
            HypMethod::HyperWave => "Hyp<HyperWave>",
            HypMethod::Search => "Hyp<Search>"
        }
}
}

 */

impl ValueMatcher<HypMethod> for HypMethod {
    fn is_match(&self, x: &HypMethod) -> Result<(), ()> {
        if *x == *self {
            Ok(())
        } else {
            Err(())
        }
    }
}
