use crate::version::v0_0_1 as version;

static VERSION : &'static str = "0.0.1";

pub mod guest {
    use crate::version::v0_0_1 as version;

    pub type Call = version::guest::Call;
    pub type Frame = version::guest::Frame;
}

pub mod host {
    use crate::version::v0_0_1 as version;

    pub type Call = version::host::Call;
    pub type Frame = version::host::Frame;
}