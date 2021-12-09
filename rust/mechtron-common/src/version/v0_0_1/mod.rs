
static VERSION : &'static str = "0.0.1";

pub mod guest {
    use serde::{Serialize,Deserialize};
    use mesh_portal_serde::version::latest;


    #[derive(Clone, Serialize, Deserialize)]
    pub enum Frame {
        Version(String),
        Create(latest::config::Info),
        Assign(latest::config::Info),
        Destroy(latest::id::Address),
        Request(Request)
    }

    pub type Request = crate::version::v0_0_1::guest::generic::Request<latest::id::Identifier,latest::payload::Payload>;
    pub type Response = crate::version::v0_0_1::guest::generic::Response<latest::id::Identifier,latest::payload::Payload>;

    pub mod generic {
        use serde::{Serialize,Deserialize};
        use mesh_portal_serde::version::latest;
        use crate::version::v0_0_1::host;

        #[derive(Clone,Serialize,Deserialize)]
        pub struct Request<IDENTIFIER, PAYLOAD> {
            pub to: IDENTIFIER,
            pub from: IDENTIFIER,
            pub entity: latest::generic::entity::request::ReqEntity<PAYLOAD>,
            pub exchange: latest::messaging::Exchange
        }

        impl<IDENTIFIER, PAYLOAD> Request<IDENTIFIER, PAYLOAD> {
            pub fn ok( self, payload: PAYLOAD ) -> host::generic::Response<PAYLOAD> {
                host::generic::Response {
                    entity: latest::generic::entity::response::RespEntity::Ok(payload),
                }
            }

            pub fn fail( self, fail: latest::fail::Fail ) -> host::generic::Response<PAYLOAD> {
                host::generic::Response {
                    entity: latest::generic::entity::response::RespEntity::Fail(fail),
                }
            }
        }

        #[derive(Clone,Serialize,Deserialize)]
        pub struct Response<IDENTIFIER,PAYLOAD> {
            pub to: IDENTIFIER,
            pub from: IDENTIFIER,
            pub entity: latest::generic::entity::response::RespEntity<PAYLOAD,latest::fail::Fail>
        }
    }

}

pub mod host {

    use serde::{Serialize,Deserialize};
    use mesh_portal_serde::version::latest;

    #[derive(Clone, Serialize, Deserialize)]
    pub enum Frame {
        Version(String),
        Request(Request),
        Respond(Response)
    }

    pub type Request = crate::version::v0_0_1::host::generic::Request<latest::id::Identifier,latest::payload::Payload>;
    pub type Response = crate::version::v0_0_1::host::generic::Response<latest::payload::Payload>;


    pub mod generic {
        use serde::{Serialize,Deserialize};
        use mesh_portal_serde::version::latest;

        // host should be able to ascertain who it is from
        #[derive(Clone, Serialize, Deserialize)]
        pub struct Request<IDENTIFIER, PAYLOAD> {
            pub to: IDENTIFIER,
            pub from: IDENTIFIER,
            pub entity: latest::generic::entity::request::ReqEntity<PAYLOAD>,
            pub exchange: latest::messaging::ExchangeType
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct Response<PAYLOAD> {
            pub entity: latest::generic::entity::response::RespEntity<PAYLOAD,latest::fail::Fail>,
        }
    }

}


