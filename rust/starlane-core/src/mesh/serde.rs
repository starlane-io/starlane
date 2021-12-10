
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

    pub type Address = id::Address;
    pub type ResourceType = id::ResourceType;
    pub type Kind = id::Kind;
    pub type Specific = id::Specific;
    pub type Version = id::Version;
    pub type AddressAndKind = generic::id::AddressAndKind<Address,Kind>;
    pub type AddressAndType = generic::id::AddressAndType<Address,ResourceType>;
    pub type Meta=id::Meta;
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
    pub type Bin = bin::Bin;
}

pub mod payload {
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::bin::Bin;
    use mesh_portal_serde::version::latest::id::{Address, Kind};
    use mesh_portal_serde::version::latest::payload;

    pub type Primitive = generic::payload::Primitive<Address,Kind>;
    pub type Payload = generic::payload::Payload<Address,Kind>;
    pub type PayloadType = payload::PayloadType;
    pub type PrimitiveType= payload::PrimitiveType;
    pub type PayloadRef = payload::PayloadRef;
    pub type PayloadDelivery = generic::payload::PayloadDelivery<Payload,PayloadRef>;
    pub type Call = generic::payload::Call<Address>;
    pub type CallKind = generic::payload::CallKind;
    pub type CallWithConfig = generic::payload::CallWithConfig<Address>;
    pub type MapPattern = generic::payload::MapPattern<Address,Kind>;
    pub type PayloadTypePattern = generic::payload::PayloadListPattern<Address,Kind>;
    pub type PayloadPattern = generic::payload::PayloadPattern<Address,Kind>;
    pub type ListPattern = generic::payload::ListPattern;
    pub type PayloadMap = generic::payload::PayloadMap<Address,Kind>;
    pub type PayloadFormat= generic::payload::PayloadFormat;
    pub type Range = generic::payload::Range;
    pub type RcCommand = payload::RcCommand;

}

pub mod command {
    use mesh_portal_serde::version::latest::command;

    pub type Command = command::Command;
    pub type CommandStatus = command::CommandStatus;
    pub type CommandEvent = command::CommandEvent;
}

pub mod http {
    use mesh_portal_serde::version::latest::http;
    use mesh_portal_serde::version::latest::Bin;

    pub type HttpRequest = http::HttpRequest;
    pub type HttpResponse = http::HttpResponse;
}


pub mod config {
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::id::{Address, Kind};
    use mesh_portal_serde::version::latest::config;

    pub type PortalKind = config::PortalKind;
    pub type Info = generic::config::Info<Address,Kind>;
    pub type Config = config::Config;
    pub type SchemaRef = config::SchemaRef;
    pub type BindConfig = config::BindConfig;
    pub type PortConfig = config::PortConfig;
    pub type EntityConfig = config::EntityConfig;
    pub type ResourceConfig = config::ResourceConfig;
    pub type PayloadConfig = config::PayloadConfig;
}

pub mod entity {

    use mesh_portal_serde::version::latest::entity;
    pub type EntityType= entity::EntityType;

    pub mod request {
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType};
        use mesh_portal_serde::version::latest::bin::Bin;
        use mesh_portal_serde::version::latest::payload::PayloadDelivery;

        pub type ReqEntity = generic::entity::request::ReqEntity<PayloadDelivery>;
        pub type Rc = generic::entity::request::Rc<PayloadDelivery>;
        pub type Msg = generic::entity::request::Msg<PayloadDelivery>;
        pub type Http = generic::entity::request::Http<PayloadDelivery>;
    }

    pub mod response{
        use mesh_portal_serde::version::latest::{fail, generic};
        use mesh_portal_serde::version::latest::id::{Address,  Kind};
        use mesh_portal_serde::version::latest::payload::PayloadDelivery;

        pub type RespEntity = generic::entity::response::RespEntity<PayloadDelivery,fail::Fail>;
    }

}

pub mod resource {
    use serde::{Deserialize, Serialize};

    use mesh_portal_serde::version::latest::resource;
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType};

    pub type Status = resource::Status;

    pub type Archetype= generic::resource::Archetype<Kind,Address>;
    pub type ResourceStub = generic::resource::ResourceStub<Address,Kind>;
}

pub mod portal {

    pub mod inlet {
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType};
        use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
        use mesh_portal_serde::error::Error;
        use mesh_portal_serde::version::latest::payload::PayloadDelivery;

        pub type Request=generic::portal::inlet::Request<Address,PayloadDelivery>;
        pub type Response=generic::portal::inlet::Response<Address,PayloadDelivery>;
        pub type Frame=generic::portal::inlet::Frame<Address,PayloadDelivery>;

        pub mod exchange {
            use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType };
            use mesh_portal_serde::version::latest::generic;
            use mesh_portal_serde::version::latest::payload::PayloadDelivery;
            pub type Request=generic::portal::inlet::exchange::Request<Address,PayloadDelivery>;
        }
    }

    pub mod outlet {
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::portal;
        use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType};
        use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
        use mesh_portal_serde::error::Error;
        use mesh_portal_serde::version::latest::payload::PayloadDelivery;

        pub type Request=portal::outlet::Request;
        pub type Response=portal::outlet::Response;
        pub type Frame=portal::outlet::Frame;

        pub mod exchange {
            use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType};
            use mesh_portal_serde::version::latest::generic;
            use mesh_portal_serde::version::latest::payload::PayloadDelivery;

            pub type Request=generic::portal::outlet::exchange::Request<Address,PayloadDelivery>;
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


        pub type AddressAndKind<ADDRESS,KIND> = generic::id::AddressAndKind<ADDRESS,KIND>;
        pub type AddressAndType<KEY, RESOURCE_TYPE> = generic::id::AddressAndType<KEY,RESOURCE_TYPE>;
    }

    pub mod config {
        use std::fmt::Debug;
        use std::hash::Hash;
        use std::str::FromStr;

        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::version::latest::ArtifactRef;
        use mesh_portal_serde::version::latest::config::{Config, PortalKind};
        use mesh_portal_serde::version::latest::generic::resource::Archetype;
        use mesh_portal_serde::version::latest::generic;

        pub type Info<ADDRESS, KIND>=generic::config::Info<ADDRESS,KIND>;
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

            pub type ReqEntity<PAYLOAD> = generic::entity::request::ReqEntity<PAYLOAD>;
            pub type Rc<PAYLOAD> = generic::entity::request::Rc<PAYLOAD>;
            pub type Msg<PAYLOAD> = generic::entity::request::Msg<PAYLOAD>;
            pub type Http<PAYLOAD> = generic::entity::request::Http<PAYLOAD>;
        }

        pub mod response {
            use std::fmt::Debug;
            use std::hash::Hash;
            use std::str::FromStr;

            use mesh_portal_serde::version::latest::bin::Bin;
            use mesh_portal_serde::version::latest::generic;

            use serde::{Deserialize, Serialize};

            pub type RespEntity<PAYLOAD,FAIL> = generic::entity::response::RespEntity<PAYLOAD,FAIL>;
        }
    }


    pub mod resource {
        use std::collections::{HashMap, HashSet};
        use std::fmt::Debug;
        use std::hash::Hash;
        use std::str::FromStr;

        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::error::Error;
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::generic::id::{AddressAndKind};
        use mesh_portal_serde::version::latest::State;

        pub type Archetype<KIND,ADDRESS>=generic::resource::Archetype<KIND,ADDRESS>;
        pub type ResourceStub<ADDRESS, KIND > = generic::resource::ResourceStub<ADDRESS, KIND>;
        pub type Resource<ADDRESS,KIND> = generic::resource::Resource<ADDRESS,KIND>;
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

            pub type Request<IDENTIFIER, PAYLOAD> = inlet::Request<IDENTIFIER,PAYLOAD>;
            pub type Response<IDENTIFIER, PAYLOAD> = inlet::Response<IDENTIFIER,PAYLOAD>;
            pub type Frame<IDENTIFIER, PAYLOAD> = inlet::Frame<IDENTIFIER,PAYLOAD>;

            pub mod exchange {
                use std::fmt::Debug;
                use std::hash::Hash;
                use std::str::FromStr;

                use serde::{Deserialize, Serialize};
                use crate::mesh::serde::generic::portal::inlet::exchange;

                pub type Request<IDENTIFIER, PAYLOAD> = exchange::Request<IDENTIFIER,PAYLOAD>;
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

            pub type Request<ADDRESS, PAYLOAD> =  outlet::Request<ADDRESS,PAYLOAD>;
            pub type Response<ADDRESS, PAYLOAD> =  outlet::Response<ADDRESS,PAYLOAD>;
            pub type Frame< ADDRESS,KIND,PAYLOAD> =  outlet::Frame<ADDRESS,KIND,PAYLOAD>;

            pub mod exchange {
                use std::fmt::Debug;
                use std::hash::Hash;
                use std::str::FromStr;

                use serde::{Deserialize, Serialize};

                use crate::mesh::serde::generic::portal::outlet::exchange;

                pub type Request<IDENTIFIER, PAYLOAD> = exchange::Request<IDENTIFIER,PAYLOAD>;
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

        pub type Payload<ADDRESS,KIND> = payload::Payload<ADDRESS,KIND>;
        pub type PayloadMap<ADDRESS, KIND> = payload::PayloadMap<ADDRESS,KIND>;
        pub type Primitive<ADDRESS,KIND> = payload::Primitive<ADDRESS,KIND>;
        pub type PayloadDelivery<PAYLOAD,PAYLOAD_REF> = payload::PayloadDelivery<PAYLOAD,PAYLOAD_REF>;
        pub type Call<ADDRESS> = payload::Call<ADDRESS>;
        pub type CallKind = payload::CallKind;
        pub type CallWithConfig<ADDRESS> = payload::CallWithConfig<ADDRESS>;
        pub type MapPattern<ADDRESS,KIND>= payload::MapPattern<ADDRESS,KIND>;
        pub type ListPattern = payload::ListPattern;
        pub type PayloadListPattern<ADDRESS, KIND>= payload::PayloadTypePattern<ADDRESS,KIND>;
        pub type PayloadPattern<ADDRESS,KIND> = payload::PayloadPattern<ADDRESS, KIND>;
        pub type Range= payload::Range;
        pub type RcCommand = payload::RcCommand;
        pub type PayloadFormat = payload::PayloadFormat;
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
    pub type Illegal=mesh_portal_serde::version::latest::fail::Illegal;
    pub type Wrong=mesh_portal_serde::version::latest::fail::Wrong;
    pub type Messaging=mesh_portal_serde::version::latest::fail::Messaging;
    pub type Fail=mesh_portal_serde::version::latest::fail::Fail;
}

pub mod util {
    use mesh_portal_serde::version::latest::util;

    pub type ValuePattern<V> = util::ValuePattern<V>;
    pub type ValueMatcher<V> = util::ValueMatcher<V>;
    pub type RegexMatcher = util::RegexMatcher;
    pub type StringMatcher= util::StringMatcher;
}

pub mod error {
    pub type Error=mesh_portal_serde::version::latest::error::Error;

}





