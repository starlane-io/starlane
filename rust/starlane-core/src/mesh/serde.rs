use mesh_portal_serde::version::v0_0_1;

pub type State=v0_0_1::State;
pub type ArtifactRef=v0_0_1::ArtifactRef;
pub type Artifact=v0_0_1::Artifact;
pub type Port=v0_0_1::Port;


pub mod id {
    use mesh_portal_serde::version::v0_0_1::generic;
    use mesh_portal_serde::version::v0_0_1::id;
    use starlane_resources::ResourcePath;
    use crate::resource::{ResourceKey, ResourceKind};

    pub type ResourceType = ResourceType;
    pub type Key = ResourceKey;
    pub type Address = ResourcePath;
    pub type Kind = ResourceKind;

    pub enum IdentifierKind {
        Key,
        Address
    }

    pub type Identifiers = generic::id::Identifiers<Key, Address>;
    pub type Identifier = generic::id::Identifier<Key, Address>;
}

pub mod messaging {
    use mesh_portal_serde::version::v0_0_1::messaging;
    pub type ExchangeId = messaging::ExchangeId;
    pub type Exchange = messaging::Exchange;
}


pub mod log {
    use mesh_portal_serde::version::v0_0_1::log;

    pub type Log = log::Log;
}



pub mod frame {
    use mesh_portal_serde::version::v0_0_1::frame;

    pub type PrimitiveFrame = frame::PrimitiveFrame;
    pub type CloseReason = frame::CloseReason;
}

pub mod bin {
    use mesh_portal_serde::version::v0_0_1::bin;

    pub type BinSrc = bin::BinSrc;
    pub type BinRaw = bin::BinRaw;
    pub type Bin = bin::Bin;
    pub type BinParcel = bin::BinParcel;
}

pub mod command {
    use mesh_portal_serde::version::v0_0_1::command;

    pub type Command = command::Command;
    pub type CommandStatus = command::CommandStatus;
    pub type CliId = command::CliId;
    pub type CliEvent = command::CommandEvent;
}

pub mod http {
    use mesh_portal_serde::version::v0_0_1::http;

    pub type HttpRequest = http::HttpRequest;
    pub type HttpResponse = http::HttpResponse;
}

pub mod resource {
    use mesh_portal_serde::version::latest::id::{Key, Address, Kind};
    use serde::{Deserialize, Serialize};
    use mesh_portal_serde::version::v0_0_1::resource;

    use mesh_portal_serde::version::v0_0_1::{State, generic};

    pub type Status = resource::Status;

    pub type Create=generic::resource::Create<Key,Address,Kind>;
    pub type StateSrc=generic::resource::StateSrc;
    pub type CreateStrategy=generic::resource::CreateStrategy;
    pub type AddressSrc=generic::resource::AddressSrc;
    pub type Selector=generic::resource::Selector;
    pub type MetaSelector=generic::resource::MetaSelector;
    pub type ResourceStub = generic::resource::ResourceStub<Key,Address,Kind>;
    pub type Archetype = generic::resource::Archetype<Kind>;
}

pub mod config {
    use mesh_portal_serde::version::latest::id::{Key, Address, Kind};
    use mesh_portal_serde::version::v0_0_1::config;
    use mesh_portal_serde::version::v0_0_1::generic;

    pub type Info = generic::config::Info<Key,Address,Kind>;
    pub type PortalKind = config::PortalKind;
    pub type Config = config::Config;
    pub type SchemaRef = config::SchemaRef;
    pub type BindConfig = config::BindConfig;
    pub type PortConfig = config::PortConfig;
    pub type EntityConfig = config::EntityConfig;
    pub type ResourceConfig = config::ResourceConfig;
    pub type PayloadConfig = config::PayloadConfig;
}

pub mod payload {
    use mesh_portal_serde::version::latest::id::{Key, Address, Kind};
    use mesh_portal_serde::version::v0_0_1::payload;
    use mesh_portal_serde::version::v0_0_1::generic;
    use mesh_portal_serde::version::v0_0_1::bin::Bin;

    pub type PayloadType = payload::PayloadType;
    pub type Payload = generic::payload::Payload<Key,Address,Kind,Bin>;
    pub type PayloadAspect = generic::payload::PayloadAspect<Key,Address,Kind,Bin>;
}

pub mod entity {

    pub mod request {
        use mesh_portal_serde::version::v0_0_1::generic;
        use mesh_portal_serde::version::latest::id::{Key, Address, Kind,ResourceType};

        pub type ReqEntity = generic::entity::request::ReqEntity<Key,Address,Kind,ResourceType>;
        pub type Rc = generic::entity::request::Rc<Key,Address,Kind,ResourceType>;
        pub type Msg = generic::entity::request::Msg<Key,Address,Kind>;
        pub type Http = generic::entity::request::Http;
    }
    pub mod response{
        use mesh_portal_serde::version::v0_0_1::generic;
        use mesh_portal_serde::version::latest::fail;
        use mesh_portal_serde::version::latest::id::{Key, Address, Kind};

        pub type RespEntity = generic::entity::response::RespEntity<Key,Address,Kind,fail::Fail>;
    }
}

pub mod portal {
    pub mod inlet {
        use mesh_portal_serde::version::latest::id::{Key, Address, Kind, ResourceType};
        use std::convert::TryFrom;
        use std::convert::TryInto;

        use anyhow::Error;
        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::version::v0_0_1::generic;
        use mesh_portal_serde::version::v0_0_1::frame::PrimitiveFrame;

        pub type Request=generic::portal::inlet::Request<Key,Address,Kind,ResourceType>;
        pub type Response=generic::portal::inlet::Response<Key,Address,Kind>;
        pub type Frame=generic::portal::inlet::Frame<Key,Address,Kind,ResourceType>;


        pub mod exchange {
            use mesh_portal_serde::version::v0_0_1::generic;
            use mesh_portal_serde::version::latest::id::{Key, Address, Kind, ResourceType};

            pub type Request=generic::portal::inlet::exchange::Request<Key,Address,Kind,ResourceType>;
        }
    }

    pub mod outlet {
        use mesh_portal_serde::version::latest::id::{Key, Address, Kind, ResourceType};

        use std::convert::TryFrom;
        use std::convert::TryInto;

        use anyhow::Error;
        use serde::{Deserialize, Serialize};
        use mesh_portal_serde::version::v0_0_1::generic;
        use mesh_portal_serde::version::v0_0_1::frame::PrimitiveFrame;


        pub type Request=generic::portal::outlet::Request<Key,Address,Kind,ResourceType>;
        pub type Response=generic::portal::outlet::Response<Key,Address,Kind>;
        pub type Frame=generic::portal::outlet::Frame<Key,Address,Kind,ResourceType>;

        pub mod exchange {
            use mesh_portal_serde::version::v0_0_1::generic;
            use mesh_portal_serde::version::latest::id::{Key, Address, Kind,ResourceType};

            pub type Request=generic::portal::outlet::exchange::Request<Key,Address,Kind,ResourceType>;
        }
    }
}



pub mod fail {
    use serde::{Deserialize, Serialize};
    use mesh_portal_serde::version::v0_0_1::fail;

    pub mod mesh {
        use serde::{Deserialize, Serialize};
        use mesh_portal_serde::version::v0_0_1::fail::mesh;
        pub type Fail = mesh::Fail;
    }

    pub mod portal{
        use serde::{Deserialize, Serialize};
        use mesh_portal_serde::version::v0_0_1::fail::portal;
        pub type Fail = portal::Fail;
    }

    pub mod resource {
        use serde::{Deserialize, Serialize};
        use mesh_portal_serde::version::v0_0_1::fail::resource;
        use mesh_portal_serde::version::latest::id::Address;

        pub type Fail = resource::Fail;
        pub type Create= resource::Create;
        pub type Update = resource::Update;
    }


    pub mod port {
        use mesh_portal_serde::version::v0_0_1::fail::port;
        use mesh_portal_serde::version::latest::id::Address;
        pub type Fail = port::Fail;
    }

    pub mod http {
        use mesh_portal_serde::version::v0_0_1::fail::http;
        use mesh_portal_serde::version::latest::id::Address;
        use serde::{Deserialize, Serialize};

        pub type Error = http::Error;
    }

    pub type BadRequest = fail::BadRequest;
    pub type Conditional = fail::Conditional;
    pub type Messaging = fail::Messaging;
    pub type Timeout = fail::Timeout;
    pub type NotFound = fail::NotFound;
    pub type Bad = fail::Bad;
    pub type Identifier = fail::Identifier;
    pub type Illegal = fail::Illegal;
    pub type Wrong = fail::Wrong;
    pub type Fail = fail::Fail;
}

