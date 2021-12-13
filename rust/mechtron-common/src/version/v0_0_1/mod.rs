
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

    pub type Request = crate::version::v0_0_1::guest::generic::Request<latest::id::ResourceType,latest::id::Kind>;
    pub type Response = crate::version::v0_0_1::guest::generic::Response<latest::payload::Payload>;

    pub mod generic {
        use serde::{Serialize,Deserialize};
        use mesh_portal_serde::version::latest;
        use crate::version::v0_0_1::host;
        use mesh_portal_serde::version::latest::id::Address;
        use mesh_portal_serde::version::latest::generic::payload::Payload;

        #[derive(Clone,Serialize,Deserialize)]
        pub struct Request<ResourceType,Kind> {
            pub to: Address,
            pub from: Address,
            pub entity: latest::generic::entity::request::ReqEntity<ResourceType,Kind>,
            pub exchange: latest::messaging::Exchange
        }

        impl<ResourceType,Kind> Request<ResourceType,Kind> {
            pub fn ok( self, payload: Payload<Kind>) -> host::generic::Response<Payload<Kind>> {
                host::generic::Response {
                    entity: latest::generic::entity::response::RespEntity::Ok(payload),
                }
            }

            pub fn fail( self, fail: latest::fail::Fail ) -> host::generic::Response<Payload<Kind>> {
                host::generic::Response {
                    entity: latest::generic::entity::response::RespEntity::Fail(fail),
                }
            }
        }

        #[derive(Clone,Serialize,Deserialize)]
        pub struct Response<PAYLOAD> {
            pub to: Address,
            pub from: Address,
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

    pub type Request = crate::version::v0_0_1::host::generic::Request<latest::id::ResourceType,latest::id::Kind>;
    pub type Response = crate::version::v0_0_1::host::generic::Response<latest::payload::Payload>;


    pub mod generic {
        use serde::{Serialize,Deserialize};
        use mesh_portal_serde::version::latest;
        use mesh_portal_serde::version::latest::id::Address;

        // host should be able to ascertain who it is from
        #[derive(Clone, Serialize, Deserialize)]
        pub struct Request<ResourceType,Kind> {
            pub to: Address,
            pub from: Address,
            pub entity: latest::generic::entity::request::ReqEntity<ResourceType,Kind>,
            pub exchange: latest::messaging::ExchangeType
        }

        #[derive(Clone, Serialize, Deserialize)]
        pub struct Response<Payload> {
            pub entity: latest::generic::entity::response::RespEntity<Payload,latest::fail::Fail>,
        }
    }

}


