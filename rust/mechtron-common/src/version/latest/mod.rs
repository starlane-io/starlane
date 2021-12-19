use crate::version::v0_0_1 as version;

static VERSION : &'static str = "0.0.1";

pub mod core {
    use crate::version::v0_0_1;

    pub type Frame = v0_0_1::core::Frame;
    pub type Request = v0_0_1::core::Request;
    pub type Response = v0_0_1::core::Response;

}

pub mod shell {
    use crate::version::v0_0_1;
    pub type Frame= v0_0_1::shell::Frame;

    pub type Request = v0_0_1::shell::Request;
    pub type Response = v0_0_1::shell::Response;
}


