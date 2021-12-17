
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
    use crate::resource;

    pub type Address = id::Address;
    pub type AddressSegment = id::AddressSegment;
    pub type ResourceType = resource::ResourceType;
    pub type Kind = resource::Kind;
    pub type Specific = id::Specific;
    pub type Version = id::Version;
    pub type AddressAndKind = generic::id::AddressAndKind<Kind>;
    pub type AddressAndType = generic::id::AddressAndType<ResourceType>;
    pub type KindParts = generic::id::KindParts<ResourceType>;
    pub type Meta=id::Meta;
}

pub mod pattern {
    use crate::mesh::serde::id::ResourceType;
    use crate::mesh::serde::id::Kind;
    use mesh_portal_serde::version::latest::generic::pattern;
    pub type TksPattern = pattern::TksPattern<ResourceType,Kind>;
    pub type AddressKindPattern = pattern::AddressKindPattern<ResourceType,Kind>;
    pub type SegmentPattern = pattern::SegmentPattern;
    pub type ExactSegment= pattern::ExactSegment;
    pub type Hop = pattern::Hop<ResourceType, Kind>;
    pub type AddressKindPath = pattern::AddressKindPath<ResourceType,Kind>;
    pub type AddressKindSegment = pattern::AddressKindSegment<Kind>;
}

pub mod messaging {
    use mesh_portal_serde::version::latest::messaging;

    pub type ExchangeId = messaging::ExchangeId;
    pub type Exchange = messaging::Exchange;
    pub type ExchangeType = messaging::ExchangeType;
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
    use mesh_portal_serde::version::latest::payload;
    use crate::mesh::serde::id::Kind;
    use crate::mesh::serde::id::ResourceType;
    use crate::mesh::serde::pattern::TksPattern;

    pub type Primitive = generic::payload::Primitive<Kind>;
    pub type Payload = generic::payload::Payload<Kind>;
    pub type PayloadType = payload::PayloadType;
    pub type PrimitiveType= payload::PrimitiveType;
    pub type PrimitiveList= generic::payload::PrimitiveList<Kind>;
    pub type PayloadRef = payload::PayloadRef;
    pub type PayloadDelivery = generic::payload::PayloadDelivery<Payload,PayloadRef>;
    pub type Call = generic::payload::Call;
    pub type CallKind = generic::payload::CallKind;
    pub type CallWithConfig = generic::payload::CallWithConfig;
    pub type MapPattern = generic::payload::MapPattern;
    pub type PayloadTypePattern = generic::payload::PayloadTypePattern;
    pub type PayloadPattern = generic::payload::PayloadPattern;
    pub type ListPattern = generic::payload::ListPattern;
    pub type PayloadMap = generic::payload::PayloadMap<Kind>;
    pub type PayloadFormat= generic::payload::PayloadFormat;
    pub type Range = generic::payload::Range;
    pub type RcCommand = generic::payload::RcCommand<ResourceType,Kind>;
}

pub mod command {
    use mesh_portal_serde::version::latest::command;

    pub type Command = command::Command;
    pub type CommandStatus = command::CommandStatus;
    pub type CommandEvent = command::CommandEvent;
}

pub mod http {
    use mesh_portal_serde::version::latest::http;

    pub type HttpRequest = http::HttpRequest;
    pub type HttpResponse = http::HttpResponse;
}


pub mod config {
    use crate::mesh::serde::id::Kind;
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::config;

    pub type PortalKind = config::PortalKind;
    pub type Info = generic::config::Info<Kind>;
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
        use crate::mesh::serde::id::Kind;
        use crate::mesh::serde::id::ResourceType;
        use crate::mesh::serde::pattern::TksPattern;
        use crate::mesh::serde::payload::Payload;
        use mesh_portal_serde::version::latest::id::Address;
        use mesh_portal_serde::version::latest::bin::Bin;

        pub type ReqEntity = generic::entity::request::ReqEntity<ResourceType,Kind>;
        pub type Rc = generic::entity::request::Rc<ResourceType,Kind>;
        pub type Msg = generic::entity::request::Msg<Payload>;
        pub type Http = generic::entity::request::Http<Payload>;
    }

    pub mod response{
        use mesh_portal_serde::version::latest::{fail, generic};
        use crate::mesh::serde::payload::Payload;

        pub type RespEntity = generic::entity::response::RespEntity<Payload>;
    }

}

pub mod resource {
    use serde::{Deserialize, Serialize};

    use mesh_portal_serde::version::latest::resource;
    use mesh_portal_serde::version::latest::generic;
    use mesh_portal_serde::version::latest::id::{Address};
    use crate::resource::ResourceType;
    use crate::resource::Kind;
    pub type Status = resource::Status;

    pub type Archetype= generic::resource::Archetype<Kind>;
    pub type ResourceStub = generic::resource::ResourceStub<Kind>;
    pub type Resource = generic::resource::Resource<Kind>;
    pub type Properties = generic::resource::Properties<Kind>;

    pub mod command {
        use mesh_portal_serde::version::latest::generic;
        use serde::{Deserialize, Serialize};
        use crate::mesh::serde::id::ResourceType;
        use crate::mesh::serde::id::Kind;
        use crate::mesh::serde::pattern::TksPattern;

        pub type RcCommand = generic::resource::command::RcCommand<ResourceType,Kind>;
        pub type RcCommandType = generic::resource::command::RcCommandType;

        pub mod common {
            use mesh_portal_serde::version::latest::generic;
            use crate::mesh::serde::id::ResourceType;
            use crate::mesh::serde::id::Kind;
            use crate::mesh::serde::pattern::TksPattern;

            pub type StateSrc = generic::resource::command::common::StateSrc<Kind>;
            pub type SetProperties = generic::resource::command::common::SetProperties<Kind>;
            pub type SetLabel = generic::resource::command::common::SetLabel;
            pub type SetRegistry = generic::resource::command::common::SetRegistry;
        }

        pub mod create {
            use mesh_portal_serde::version::latest::generic;
            use crate::mesh::serde::id::ResourceType;
            use crate::mesh::serde::id::Kind;
            use crate::mesh::serde::pattern::TksPattern;

            pub type Create = generic::resource::command::create::Create<Kind>;
            pub type AddressTemplate  = generic::resource::command::create::AddressTemplate;
            pub type AddressSegmentTemplate = generic::resource::command::create::AddressSegmentTemplate;
            pub type KindTemplate = generic::resource::command::create::KindTemplate;
            pub type Strategy = generic::resource::command::create::Strategy;
            pub type Template = generic::resource::command::create::Template;
        }

        pub mod select {
            use mesh_portal_serde::version::latest::generic;
            use crate::mesh::serde::id::ResourceType;
            use crate::mesh::serde::id::Kind;
            use crate::mesh::serde::pattern::TksPattern;

            pub type Select=generic::resource::command::select::Select<ResourceType,Kind>;
            pub type SubSelector=generic::resource::command::select::SubSelector<ResourceType,Kind>;
            pub type PropertiesPattern = generic::resource::command::select::PropertiesPattern;
            pub type SelectionKind=generic::resource::command::select::SelectionKind<ResourceType,Kind>;
        }

        pub mod update {
            use mesh_portal_serde::version::latest::generic;
            use crate::mesh::serde::id::ResourceType;
            use crate::mesh::serde::id::Kind;
            use crate::mesh::serde::pattern::TksPattern;

            pub type Update=generic::resource::command::update::Update<Kind>;
        }

        pub mod query{
            use mesh_portal_serde::version::latest::generic;
            use crate::mesh::serde::id::Kind;
            use crate::mesh::serde::id::ResourceType;

            pub type Query=generic::resource::command::query::Query;
            pub type QueryResult=generic::resource::command::query::QueryResult<ResourceType,Kind>;
        }
    }

}

pub mod portal {

    pub mod inlet {
        use crate::resource::ResourceType;
        use crate::resource::Kind;
        use crate::mesh::serde::payload::Payload;
        use crate::mesh::serde::entity::request::ReqEntity;

        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::id::{Address};
        use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
        use mesh_portal_serde::error::Error;

        pub type Request=generic::portal::inlet::Request<ReqEntity>;
        pub type Response=generic::portal::inlet::Response<Payload>;
        pub type Frame=generic::portal::inlet::Frame<Address,Payload>;

/*        pub mod exchange {
            use crate::resource::ResourceType;
            use crate::resource::Kind;
            use crate::mesh::serde::payload::Payload;
            use crate::mesh::serde::entity::request::ReqEntity;
            use mesh_portal_serde::version::latest::id::{Address};
            use mesh_portal_serde::version::latest::generic;
            use mesh_portal_serde::version::latest::payload::PayloadDelivery;
            pub type Request=generic::portal::inlet::exchange::Request<Address,PayloadDelivery>;
        }

 */
    }

    pub mod outlet {
        use mesh_portal_serde::version::latest::generic;
        use mesh_portal_serde::version::latest::portal;
        use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType};
        use mesh_portal_serde::version::latest::frame::PrimitiveFrame;
        use mesh_portal_serde::error::Error;
        use mesh_portal_serde::version::latest::payload::Payload;

        pub type Request=portal::outlet::Request;
        pub type Response=portal::outlet::Response;
        pub type Frame=portal::outlet::Frame;

        /*
        pub mod exchange {
            use mesh_portal_serde::version::latest::id::{Address, Kind, ResourceType};
            use mesh_portal_serde::version::latest::generic;
            use mesh_portal_serde::version::latest::payload::Payload;

            pub type Request=generic::portal::outlet::exchange::Request<Address,Payload>;
        }
         */
    }
}

pub mod generic {

    pub mod id {
        use std::fmt::Debug;
        use std::hash::Hash;
        use std::str::FromStr;
        use serde::{Deserialize, Serialize};

        use mesh_portal_serde::version::latest::generic;


        pub type AddressAndKind<KIND> = generic::id::AddressAndKind<KIND>;
        pub type AddressAndType<RESOURCE_TYPE> = generic::id::AddressAndType<RESOURCE_TYPE>;
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

        pub type Info<KIND>=generic::config::Info<KIND>;
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

            pub type ReqEntity<ResourceType,Kind> = generic::entity::request::ReqEntity<ResourceType,Kind>;
            pub type Rc<ResourceType,Kind> = generic::entity::request::Rc<ResourceType,Kind>;
            pub type Msg<Kind> = generic::entity::request::Msg<Kind>;
            pub type Http<Kind> = generic::entity::request::Http<Kind>;
        }

        pub mod response {
            use std::fmt::Debug;
            use std::hash::Hash;
            use std::str::FromStr;

            use mesh_portal_serde::version::latest::bin::Bin;
            use mesh_portal_serde::version::latest::generic;

            use serde::{Deserialize, Serialize};

            pub type RespEntity<PAYLOAD> = generic::entity::response::RespEntity<PAYLOAD>;
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

        pub type Archetype<KIND>=generic::resource::Archetype<KIND>;
        pub type ResourceStub<KIND > = generic::resource::ResourceStub<KIND>;
        pub type Resource<KIND> = generic::resource::Resource<KIND>;
        pub type Properties<KIND> = generic::resource::Properties<KIND>;
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

            pub type Request<Entity> = inlet::Request<Entity>;
            pub type Response<PAYLOAD> = inlet::Response<PAYLOAD>;
            pub type Frame<ReqEntity,Payload> = inlet::Frame<ReqEntity,Payload>;

            pub mod exchange {
                use std::fmt::Debug;
                use std::hash::Hash;
                use std::str::FromStr;

                use serde::{Deserialize, Serialize};
                use crate::mesh::serde::generic::portal::inlet::exchange;

//                pub type Request<PAYLOAD> = exchange::Request<PAYLOAD>;
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

            pub type Request<Entity> =  outlet::Request<Entity>;
            pub type Response<PAYLOAD> =  outlet::Response<PAYLOAD>;
            pub type Frame< KIND,PAYLOAD,ReqEntity> =  outlet::Frame<KIND,PAYLOAD,ReqEntity>;

            pub mod exchange {
                use std::fmt::Debug;
                use std::hash::Hash;
                use std::str::FromStr;

                use serde::{Deserialize, Serialize};

                use crate::mesh::serde::generic::portal::outlet::exchange;

              //  pub type Request<IDENTIFIER, PAYLOAD> = exchange::Request<IDENTIFIER,PAYLOAD>;
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

        pub type Payload<KIND> = payload::Payload<KIND>;
        pub type PayloadMap<KIND> = payload::PayloadMap<KIND>;
        pub type Primitive<KIND> = payload::Primitive<KIND>;
        pub type Call = payload::Call;
        pub type CallKind = payload::CallKind;
        pub type CallWithConfig = payload::CallWithConfig;
        pub type MapPattern= payload::MapPattern;
        pub type ListPattern = payload::ListPattern;
        pub type PayloadListPattern= payload::PayloadTypePattern;
        pub type PayloadPattern = payload::PayloadPattern;
        pub type Range= payload::Range;
        pub type RcCommand<ResourceType,Kind> = payload::RcCommand<ResourceType,Kind>;
        pub type PayloadFormat = payload::PayloadFormat;
    }

    pub mod pattern {
        use mesh_portal_serde::version::latest::generic::pattern;

        pub type Pattern<P> = pattern::Pattern<P>;
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
        pub type Select=mesh_portal_serde::version::latest::fail::resource::Select;
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
    pub type Error=mesh_portal_serde::error::Error;

}





