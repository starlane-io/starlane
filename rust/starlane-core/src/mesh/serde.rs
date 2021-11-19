use std::collections::HashMap;
use std::convert::From;
use std::convert::TryInto;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use mesh_portal_serde::version::latest::bin::Bin;

pub type State=mesh_portal_serde::version::latest::State;

pub type ArtifactRef=mesh_portal_serde::version::latest::ArtifactRef;
pub type Artifact=mesh_portal_serde::version::latest::Artifact;
pub type Port=mesh_portal_serde::version::latest::Port;

pub mod id {
    use mesh_portal_serde::version::latest::id;
    use mesh_portal_serde::version::latest::generic;
    use crate::resource::{ResourceKey, ResourceKind};
    use starlane_resources::{ResourcePath, ResourceIdentifier};

    pub type Key = ResourceKey;
    pub type Address = ResourcePath;
    pub type ResourceType = crate::resource::ResourceType;
    pub type Kind = ResourceKind;
    pub type Specific = id::Specific;
    pub type Version = id::Version;
    pub type Identifier = ResourceIdentifier;
    pub type Identifiers = generic::id::Identifiers<Key,Address>;
    pub type AddressAndKind = generic::id::AddressAndKind<Address,Kind>;
    pub type AddressAndType = generic::id::AddressAndType<Address,ResourceType>;
    pub type Meta=id::Meta;
    pub type IdentifierKind = id::IdentifierKind;
}

pub mod messaging {
    use mesh_portal_serde::version::latest::messaging;

    pub type ExchangeId = messaging::ExchangeId;
    pub type Exchange = messaging::Exchange;
}


pub mod log {
    use mesh_portal_serde::version::latest::log;
    pub type Log = log::Log;
}

pub mod frame {
    use mesh_portal_serde::version::latest::frame;
    pub type PrimitiveFrame = frame::PrimitiveFrame;
    pub type CloseReason = frame::CloseReason;
}

pub mod bin {
    use mesh_portal_serde::version::latest::bin;

    pub type BinSrc = bin::BinSrc;
    pub type BinRaw = bin::BinRaw;
    pub type BinSet = bin::BinSet;
    pub type Bin = bin::Bin;
    pub type BinParcel = bin::BinParcel;
}

pub mod payload {
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::bin::Bin;
    use mesh_portal_serde::version::latest::id::{Address, Key, Kind};
    use mesh_portal_serde::version::latest::payload;

    pub type Primitive = generic::payload::Primitive<Key,Address,Kind,Bin>;
    pub type Payload = generic::payload::Payload<Key,Address,Kind,Bin>;
    pub type PayloadType = payload::PayloadType;
    pub type PrimitiveType= payload::PrimitiveType;
}

pub mod command {
    use mesh_portal_serde::version::latest::command;

    pub type Command = command::Command;
    pub type CommandStatus = command::CommandStatus;
    pub type CommandEvent = command::CommandEvent;
}

pub mod http {
    use mesh_portal_serde::version::latest::http;
    use mesh_portal_serde::version::latest::bin::Bin;

    pub type HttpRequest = http::HttpRequest;
    pub type HttpResponse = http::HttpResponse;
}


pub mod config {
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::id::{Address, Key, Kind};
    use mesh_portal_serde::version::latest::config;

    pub type PortalKind = config::PortalKind;
    pub type Info = generic::config::Info<Key,Address,Kind>;
    pub type Config = config::Config;
    pub type SchemaRef = config::SchemaRef;
    pub type BindConfig = config::BindConfig;
    pub type PortConfig = config::PortConfig;
    pub type EntityConfig = config::EntityConfig;
    pub type ResourceConfig = config::ResourceConfig;
    pub type PayloadConfig = config::PayloadConfig;
}

pub mod entity {

    pub mod request {
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::id::{Address, Key, Kind, ResourceType};
        use crate::mesh::serde::bin::Bin;

        pub type ReqEntity = generic::entity::request::ReqEntity<Key,Address,Kind,ResourceType>;
        pub type Rc = generic::entity::request::Rc<Key,Address,Kind>;
        pub type Msg = generic::entity::request::Msg<Key,Address,Kind>;
        pub type Http = generic::entity::request::Http;
    }

    pub mod response{
        use mesh_portal_serde::version::latest::{fail, generic};
        use mesh_portal_serde::version::latest::id::{Address, Key, Kind};

        pub type RespEntity = generic::entity::response::RespEntity<Key,Address,Kind,fail::Fail>;
    }

}

pub mod resource {
    use serde::{Deserialize, Serialize};

    use mesh_portal_serde::version::latest::resource;
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::id::{Address, Identifier, Key, Kind, ResourceType};

    pub type Status = resource::Status;

    pub type Archetype= generic::resource::Archetype<Kind,Address>;
    pub type ResourceStub = generic::resource::ResourceStub<Key,Address,Kind>;
}

pub mod portal {

    pub mod inlet {
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest;
        use crate::mesh::serde::id::{Address, Key, Kind, ResourceType,Identifier};
        use crate::mesh::serde::error::Error;
        use std::convert::TryFrom;
        use crate::mesh::serde::entity::request::ReqEntity;
        use mesh_portal_serde::version::v0_0_1::generic::entity::request::{Rc, Http};

        pub type Request=generic::portal::inlet::Request<Key,Address,Kind,ResourceType>;
        pub type Response=generic::portal::inlet::Response<Key,Address,Kind>;
        pub type Frame=generic::portal::inlet::Frame<Key,Address,Kind,ResourceType>;

        pub mod exchange {
            use mesh_portal_serde::version::latest::id::{Address, Key, Kind, ResourceType};
            use mesh_portal_serde::version::latest::generic;
            pub type Request=generic::portal::inlet::exchange::Request<Key,Address,Kind,ResourceType>;
        }
    }

    pub mod outlet {
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::id::{Address, Key, Kind, ResourceType};
        use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
        use mesh_portal_serde::version::latest::error::Error;

        pub type Request=generic::portal::outlet::Request<Key,Address,Kind,ResourceType>;
        pub type Response=generic::portal::outlet::Response<Key,Address,Kind>;
        pub type Frame=generic::portal::outlet::Frame<Key,Address,Kind,ResourceType>;

        pub mod exchange {
            use mesh_portal_serde::version::latest::id::{Address, Key, Kind, ResourceType};
            use mesh_portal_serde::version::latest::generic;
            pub type Request=generic::portal::outlet::exchange::Request<Key,Address,Kind,ResourceType>;
        }
    }
}

pub mod generic {

    pub mod id {
        use std::fmt::Debug;
        use std::hash::Hash;
        use std::str::FromStr;
        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::version::latest::generic;

        pub type Identifier<KEY, ADDRESS> = generic::id::Identifier<KEY,ADDRESS>;
        pub type Identifiers<KEY, ADDRESS> = generic::id::Identifiers<KEY,ADDRESS>;
        pub type AddressAndKind<KEY, ADDRESS> = generic::id::AddressAndKind<KEY,ADDRESS>;
        pub type AddressAndType<KEY, RESOURCE_TYPE> = generic::id::AddressAndType<KEY,RESOURCE_TYPE>;
    }

    pub mod config {
        use std::fmt::Debug;
        use std::hash::Hash;
        use std::str::FromStr;

        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::version::latest::ArtifactRef;
        use mesh_portal_serde::version::latest::config::{Config, PortalKind};
        use mesh_portal_serde::version::latest::generic::id::{Identifier, Identifiers};
        use mesh_portal_serde::version::latest::generic::resource::Archetype;
        use mesh_portal_serde::version::latest::generic;

        pub type Info<KEY, ADDRESS, KIND>=generic::config::Info<KEY,ADDRESS,KIND>;
    }

    pub mod entity {
        pub mod request {
            use std::hash::Hash;
            use std::str::FromStr;

            use serde::{Deserialize, Serialize};
            use serde::__private::fmt::Debug;

            use mesh_portal_serde::version::latest::{http, State};
            use mesh_portal_serde::version::latest::bin::Bin;
            use mesh_portal_serde::version::latest::generic;
            use mesh_portal_serde::version::latest::generic::payload::Primitive;
            use mesh_portal_serde::version::latest::generic::payload::Payload;

            pub type ReqEntity<KEY, ADDRESS, KIND, RESOURCE_TYPE> = generic::entity::request::ReqEntity<KEY,ADDRESS,KIND,RESOURCE_TYPE>;
            pub type Rc<KEY,ADDRESS,KIND> = generic::entity::request::Rc<KEY,ADDRESS,KIND,Bin>;
            pub type Msg<KEY, ADDRESS, KIND> = generic::entity::request::Msg<KEY,ADDRESS,KIND>;
            pub type Http = generic::entity::request::Http;
        }

        pub mod response {
            use std::fmt::Debug;
            use std::hash::Hash;
            use std::str::FromStr;

            use mesh_portal_serde::version::latest::bin::Bin;
            use mesh_portal_serde::version::latest::generic;

            use serde::{Deserialize, Serialize};

            pub type RespEntity<KEY, ADDRESS, KIND, FAIL> = generic::entity::response::RespEntity<KEY,ADDRESS,KIND,FAIL>;
        }
    }


    pub mod resource {
        use std::collections::{HashMap, HashSet};
        use std::fmt::Debug;
        use std::hash::Hash;
        use std::str::FromStr;

        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::version::latest::bin::BinSet;
        use mesh_portal_serde::version::latest::error::Error;
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::generic::id::{AddressAndKind, Identifier};
        use mesh_portal_serde::version::latest::State;

        pub type Archetype<KIND,ADDRESS>=generic::resource::Archetype<KIND,ADDRESS>;
        pub type ResourceStub<KEY, ADDRESS, KIND > = generic::resource::ResourceStub<KEY,ADDRESS,KIND>;
        pub type Resource<KEY, ADDRESS, KIND, BIN > = generic::resource::Resource<KEY,ADDRESS,KIND,BIN>;
    }

    pub mod portal {
        pub mod inlet {
            use std::convert::TryFrom;
            use std::convert::TryInto;
            use std::fmt::Debug;
            use std::hash::Hash;
            use std::str::FromStr;

            use serde::{Deserialize, Serialize};

            use mesh_portal_serde::version::latest::generic::portal::inlet;

            pub type Request<KEY, ADDRESS, KIND, RESOURCE_TYPE> = inlet::Request<KEY,ADDRESS,KIND,RESOURCE_TYPE>;
            pub type Response<KEY, ADDRESS, KIND> = inlet::Response<KEY,ADDRESS,KIND>;
            pub type Frame<KEY, ADDRESS, KIND, RESOURCE_TYPE> = inlet::Frame<KEY,ADDRESS,KIND,RESOURCE_TYPE>;

            pub mod exchange {
                use std::fmt::Debug;
                use std::hash::Hash;
                use std::str::FromStr;

                use serde::{Deserialize, Serialize};
                use mesh_portal_serde::version::latest::generic::portal::inlet::exchange;

                pub type Request<KEY, ADDRESS, KIND, RESOURCE_TYPE> = exchange::Request<KEY,ADDRESS,KIND,RESOURCE_TYPE>;
            }
        }

        pub mod outlet {
            use std::convert::TryFrom;
            use std::convert::TryInto;
            use std::fmt::Debug;
            use std::hash::Hash;
            use std::str::FromStr;

            use serde::{Deserialize, Serialize};

            use mesh_portal_serde::version::latest::generic::portal::outlet;

            pub type Request<KEY, ADDRESS, KIND,RESOURCE_TYPE> =  outlet::Request<KEY,ADDRESS,KIND,RESOURCE_TYPE>;
            pub type Response<KEY, ADDRESS, KIND> =  outlet::Response<KEY,ADDRESS,KIND>;
            pub type Frame<KEY, ADDRESS, KIND,RESOURCE_TYPE> =  outlet::Frame<KEY,ADDRESS,KIND,RESOURCE_TYPE>;

            pub mod exchange {
                use std::fmt::Debug;
                use std::hash::Hash;
                use std::str::FromStr;

                use serde::{Deserialize, Serialize};

                use mesh_portal_serde::version::latest::generic::portal::outlet::exchange;

                pub type Request<KEY, ADDRESS, KIND,RESOURCE_TYPE> = exchange::Request<KEY,ADDRESS,KIND,RESOURCE_TYPE>;
            }
        }
    }

    pub mod payload {
        use std::collections::HashMap;
        use std::fmt::Debug;
        use std::hash::Hash;
        use std::str::FromStr;

        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::version::latest::generic::payload;

        pub type Payload<KEY, ADDRESS, KIND, BIN> = payload::Payload<KEY,ADDRESS,KIND,BIN>;
        pub type Primitive<KEY, ADDRESS, KIND, BIN> = payload::Primitive<KEY,ADDRESS,KIND,BIN>;
    }

}


pub mod fail {
    use serde::{Deserialize, Serialize};

    pub mod mesh {
        pub type Fail=mesh_portal_serde::version::latest::fail::mesh::Fail;
    }

    pub mod portal {
        pub type Fail=mesh_portal_serde::version::latest::fail::portal::Fail;
    }

    pub mod resource {
        pub type Fail=mesh_portal_serde::version::latest::fail::resource::Fail;
        pub type Create=mesh_portal_serde::version::latest::fail::resource::Create;
        pub type Update=mesh_portal_serde::version::latest::fail::resource::Update;
    }

    pub mod port {
        pub type Fail=mesh_portal_serde::version::latest::fail::port::Fail;
    }

    pub mod http {
        pub type Error=mesh_portal_serde::version::latest::fail::http::Error;
    }

    pub type BadRequest=mesh_portal_serde::version::latest::fail::BadRequest;
    pub type Conditional=mesh_portal_serde::version::latest::fail::Conditional;
    pub type Timeout=mesh_portal_serde::version::latest::fail::Timeout;
    pub type NotFound=mesh_portal_serde::version::latest::fail::NotFound;
    pub type Bad=mesh_portal_serde::version::latest::fail::Bad;
    pub type Identifier=mesh_portal_serde::version::latest::fail::Identifier;
    pub type Illegal=mesh_portal_serde::version::latest::fail::Illegal;
    pub type Wrong=mesh_portal_serde::version::latest::fail::Wrong;
    pub type Messaging=mesh_portal_serde::version::latest::fail::Messaging;
    pub type Fail=mesh_portal_serde::version::latest::fail::Fail;
}

pub mod error {
    pub type Error=mesh_portal_serde::version::latest::error::Error;

}




