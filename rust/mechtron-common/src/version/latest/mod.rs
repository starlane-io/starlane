use crate::version::v0_0_1 as version;

static VERSION : &'static str = "0.0.1";



pub mod guest {
    use crate::version::v0_0_1;

    pub type Frame = v0_0_1::guest::Frame;
    pub type Request = v0_0_1::guest::Request;
    pub type Response = v0_0_1::guest::Response;

    pub mod generic {
        use crate::version::v0_0_1;
        pub type Request<IDENTIFIER,PAYLOAD> = v0_0_1::guest::generic::Request<IDENTIFIER,PAYLOAD>;
        pub type Response<IDENTIFIER,PAYLOAD> = v0_0_1::guest::generic::Response<IDENTIFIER,PAYLOAD>;
    }
}

pub mod host {
    use crate::version::v0_0_1;
    pub type Frame= v0_0_1::host::Frame;

    pub type Request = v0_0_1::host::Request;
    pub type Response = v0_0_1::host::Response;

    pub mod generic {
        use crate::version::v0_0_1;
        pub type Request<IDENTIFIER,PAYLOAD> = v0_0_1::host::generic::Request<IDENTIFIER,PAYLOAD>;
        pub type Response<PAYLOAD> = v0_0_1::host::generic::Response<PAYLOAD>;
    }
}


