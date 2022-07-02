use std::collections::HashMap;
use std::convert::From;
use std::convert::TryInto;
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use cosmic_api::version::v0_0_1 as current;

pub type State = current::State;

pub mod artifact {
    use cosmic_api::version::v0_0_1 as current;
    pub type Artifact = current::artifact::Artifact;
    pub type ArtifactRequest = current::artifact::ArtifactRequest;
    pub type ArtifactResponse = current::artifact::ArtifactResponse;
}

pub mod id {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::id;

    pub type Uuid = id::id::Uuid;
    pub type ResourceType = id::id::BaseKind;
    pub type ResourceKind = id::id::KindParts;
    pub type AddressAndKind = id::id::PointKind;
    pub type AddressAndType = id::id::AddressAndType;
    pub type Meta = id::id::Meta;
    pub type HostKey = id::id::HostKey;
    pub type Version = id::id::Version;
    pub type Tks = dyn id::id::Tks;
    pub type Specific = id::id::Specific;
    pub type RouteSegment = id::id::RouteSeg;
    pub type AddressSegment = id::id::PointSeg;
    pub type Point = id::id::Point;
    pub type Port = id::id::Port;
    pub type ToPort = dyn id::id::ToPort;
    pub type ToPoint = dyn id::id::ToPoint;
    pub type TargetLayer = id::id::Layer;
    pub type Topic = id::id::Topic;
    pub type KindParts = id::id::KindParts;
}

pub mod path {
    use cosmic_api::version::v0_0_1 as current;
    pub type Path = current::path::Path;
}

pub mod selector {
    use cosmic_api::version::v0_0_1::selector;
    use cosmic_api::version::{v0_0_1 as current, v0_0_1};

    pub type GenericKindSelector = selector::selector::KindBaseSelector;
    pub type GenericSubKindSelector = selector::selector::SubKindSelector;
    pub type PointSelector = selector::selector::Selector;
    pub type KindSelector = selector::selector::KindSelector;
    pub type VersionReq = selector::selector::VersionReq;
    pub type PointSegSelector = selector::selector::PointSegSelector;
    pub type KeySegment = selector::selector::KeySegment;
    pub type ExactSegment = selector::selector::ExactPointSeg;
    pub type SpecificPattern = selector::selector::SpecificSelector;
    pub type LabeledPrimitiveType = selector::selector::LabeledPrimitiveType;
    pub type PrimitiveType = selector::selector::PayloadType2;
    pub type Format = selector::selector::Format;

    pub type Block = v0_0_1::selector::PayloadBlock;
    pub type UploadBlock = v0_0_1::selector::UploadBlock;
    pub type CreateBlock = v0_0_1::selector::CreateBlock;
    pub type PatternBlock = v0_0_1::selector::PatternBlock;
    pub type MapEntryPattern = selector::selector::MapEntryPattern;
    pub type Hop = selector::selector::Hop;
    pub type Pattern<P> = selector::selector::Pattern<P>;
    pub type EmptyPattern<P> = selector::selector::EmptyPattern<P>;
    pub type PointKindHierarchy = selector::selector::PointHierarchy;
    pub type PointKindSeg = selector::selector::PointKindSeg;

    pub mod specific {
        use cosmic_api::version::v0_0_1 as current;
        use cosmic_api::version::v0_0_1::selector;

        pub type VersionReq = selector::selector::specific::VersionReq;
        pub type VendorSelector = selector::selector::specific::VendorSelector;
        pub type ProductSelector = selector::selector::specific::ProductSelector;
        pub type VariantSelector = selector::selector::specific::VariantSelector;
        pub type VersionPattern = selector::selector::specific::VersionPattern;
    }
}

pub mod messaging {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::wave;

    pub type RequestHandler = dyn wave::DirectedHandler;
    pub type ReqShell = wave::Ping;
    pub type RespShell = wave::Pong;
    pub type ReqProto = wave::PingProto;
    pub type Agent = wave::Agent;
    pub type Session = wave::Session;
    pub type Scope = wave::Scope;
    pub type Priority = wave::Priority;
    pub type Karma = wave::Karma;
    pub type Handling = wave::Handling;
    pub type HandlingKind = wave::HandlingKind;
    pub type CmdMethod = wave::CmdMethod;
    pub type Method = wave::Method;
    pub type MethodPattern = wave::MethodPattern;

    pub type SysMethod = wave::SysMethod;
}

pub mod frame {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::frame;

    pub type PrimitiveFrame = frame::frame::PrimitiveFrame;
    pub type CloseReason = frame::frame::CloseReason;
}

pub mod bin {
    use cosmic_api::version::v0_0_1 as current;

    pub type Bin = current::bin::Bin;
}

pub mod parse {
    use cosmic_api::version::v0_0_1 as current;

    pub type Res<I, O> = cosmic_nom::Res<I, O>;
    pub type Env = current::parse::Env;

    pub mod model {
        use cosmic_api::version::v0_0_1 as current;
        pub type ScopeFilter = current::parse::model::ScopeFilter;
        pub type ScopeFilters = current::parse::model::ScopeFilters;
    }
    use cosmic_api::error::MsgErr;
    use cosmic_api::version::v0_0_1::config::config::bind::RouteSelector;

    pub fn route_attribute(s: &str) -> Result<RouteSelector, MsgErr> {
        current::parse::route_attribute(s)
    }

    pub fn route_attribute_value(s: &str) -> Result<RouteSelector, MsgErr> {
        current::parse::route_attribute_value(s)
    }
}

pub mod payload {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::substance;

    pub type Substance = substance::substance::Substance;
    pub type PayloadMap = substance::substance::SubstanceMap;
    pub type PrimitiveList = substance::substance::SubstanceList;
    pub type PayloadType = substance::substance::SubstanceKind;
    pub type Errors = substance::substance::Errors;
    pub type ListPattern = substance::substance::ListPattern;
    pub type Range = substance::substance::NumRange;
    pub type PayloadPattern = substance::substance::SubstancePattern;
    pub type CallWithConfig = substance::substance::CallWithConfig;
    pub type Call = substance::substance::Call;
    pub type CallKind = substance::substance::CallKind;
    pub type MsgCall = substance::substance::MsgCall;
    pub type HttpCall = substance::substance::HttpCall;
    pub type PayloadFormat = substance::substance::SubstanceFormat;
    pub type MapPattern = substance::substance::MapPattern;
}

pub mod command {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::command;

    pub mod request {
        use cosmic_api::version;
        use cosmic_api::version::v0_0_1 as current;
    }

    pub mod common {
        use cosmic_api::version::v0_0_1 as current;
        use cosmic_api::version::v0_0_1::command;

        pub type StateSrc = command::command::common::StateSrc;
        pub type SetLabel = command::command::common::SetLabel;
        pub type SetProperties = command::command::common::SetProperties;
        pub type PropertyMod = command::command::common::PropertyMod;
        pub type SetRegistry = command::command::common::SetRegistry;
    }
}

pub mod security {
    use cosmic_api::version::v0_0_1 as current;

    pub type Access = current::security::Access;
    pub type Privileges = current::security::EnumeratedPrivileges;
    pub type EnumeratedAccess = current::security::EnumeratedAccess;
    pub type Permissions = current::security::Permissions;
    pub type PermissionsMask = current::security::PermissionsMask;
    pub type PermissionsMaskKind = current::security::PermissionsMaskKind;
    pub type ChildPerms = current::security::ChildPerms;
    pub type ParticlePerms = current::security::ParticlePerms;
    pub type AccessGrant = current::security::AccessGrant;
    pub type AccessGrantKind = current::security::AccessGrantKind;
}

pub mod msg {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::msg;

    pub type MsgRequest = msg::MsgRequest;
    pub type MsgMethod = msg::MsgMethod;
}

pub mod http {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::http;

    pub type HttpRequest = http::HttpRequest;
    pub type HttpMethod = http::HttpMethod;
}

pub mod config {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::config;

    pub type PortalKind = config::config::PortalKind;
    pub type Info = config::config::Info;
    pub type PortalConfig = config::config::PortalConfig;
    pub type PointConfig<BODY> = config::config::PointConfig<BODY>;
    pub type Document = config::config::Document;
    pub type ParticleConfigBody = config::config::ParticleConfigBody;

    pub mod bind {
        use cosmic_api::version::v0_0_1 as current;
        use cosmic_api::version::v0_0_1::config;

        pub type RouteSelector = config::config::bind::RouteSelector;
        pub type BindConfig = config::config::bind::BindConfig;
        pub type ConfigScope<T, E> = config::config::bind::ConfigScope<T, E>;
        pub type Pipeline = config::config::bind::Pipeline;
        pub type PipelineStep = config::config::bind::PipelineStep;
        pub type PipelineStop = config::config::bind::PipelineStop;
        pub type PatternBlock = config::config::bind::PatternBlock;
        pub type Whitelist = config::config::bind::Whitelist;
        pub type CallPattern = config::config::bind::CallPattern;
        pub type StepKind = config::config::bind::WaveKind;
    }
}

pub mod entity {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::{entity, wave};

    pub type EntityType = wave::MethodKind;

    pub mod request {
        use cosmic_api::version::v0_0_1::{command, entity};
        use cosmic_api::version::{v0_0_1 as current, v0_0_1};

        pub type Method = v0_0_1::wave::Method;
        pub type ReqCore = v0_0_1::wave::DirectedCore;
        pub type Rc = command::request::Rc;
        pub type RcCommandType = command::request::RcCommandType;

        pub mod create {
            use cosmic_api::version::v0_0_1 as current;
            use cosmic_api::version::v0_0_1::{command, entity};

            pub type Create = command::request::create::Create;
            pub type Template = command::request::create::Template;
            pub type KindTemplate = command::request::create::KindTemplate;
            pub type Fulfillment = command::request::create::Fulfillment;
            pub type Strategy = command::request::create::Strategy;
            pub type PointTemplate = command::request::create::PointTemplate;
            pub type PointSegFactory = command::request::create::PointSegTemplate;
            pub type CreateOp = command::request::create::Create;
            pub type Require = command::request::create::Require;
            pub type Set = command::request::set::Set;
        }

        pub mod select {
            use cosmic_api::version::v0_0_1 as current;
            use cosmic_api::version::v0_0_1::{command, entity};

            pub type SelectIntoPayload = command::request::select::SelectIntoSubstance;
            pub type Select = command::request::select::Select;
            pub type SelectionKind = command::request::select::SelectKind;
            pub type SubSelector = command::request::select::SubSelect;
            pub type PropertiesPattern = command::request::select::PropertiesPattern;
        }

        pub mod update {
            use cosmic_api::version::v0_0_1 as current;
            use cosmic_api::version::v0_0_1::{command, entity};

            pub type Update = command::request::update::Update;
        }

        pub mod query {
            use cosmic_api::version::v0_0_1 as current;
            use cosmic_api::version::v0_0_1::{command, entity};

            pub type Query = command::request::query::Query;
            pub type QueryResult = command::request::query::QueryResult;
        }

        pub mod get {
            use cosmic_api::version::v0_0_1 as current;
            use cosmic_api::version::v0_0_1::{command, entity};

            pub type Get = command::request::get::Get;
            pub type GetOp = command::request::get::GetOp;
        }

        pub mod set {
            use cosmic_api::version::v0_0_1 as current;
            use cosmic_api::version::v0_0_1::{command, entity};

            pub type Set = command::request::set::Set;
        }
    }

    pub mod response {
        use cosmic_api::version::v0_0_1::entity;
        use cosmic_api::version::{v0_0_1 as current, v0_0_1};

        pub type RespCore = v0_0_1::wave::ReflectedCore;
    }
}

pub mod particle {
    use cosmic_api::version::v0_0_1 as current;
    use cosmic_api::version::v0_0_1::particle;

    pub type StatusUpdate = particle::particle::StatusUpdate;
    pub type Status = particle::particle::Status;
    pub type Code = particle::particle::Code;
    pub type Progress = particle::particle::Progress;
    pub type Properties = particle::particle::Properties;
    pub type Archetype = particle::particle::Archetype;
    pub type Stub = particle::particle::Stub;
    pub type Resource = particle::particle::Particle;
    pub type Property = particle::particle::Property;
}

pub mod util {
    use crate::error::MsgErr;
    use cosmic_api::version::v0_0_1 as current;

    pub type ValuePattern<T> = current::util::ValuePattern<T>;
    pub type ValueMatcher<T> = dyn current::util::ValueMatcher<T>;
    pub type RegexMatcher = current::util::RegexMatcher;
    pub type StringMatcher = current::util::StringMatcher;
    pub type Convert<A> = dyn current::util::Convert<A>;
    pub type ConvertFrom<A> = dyn current::util::ConvertFrom<A>;

    pub fn uuid() -> String {
        current::util::uuid()
    }

    pub fn log<R>(result: Result<R, MsgErr>) -> Result<R, MsgErr> {
        current::util::log(result)
    }
}

pub mod fail {
    use cosmic_api::version::v0_0_1 as current;

    pub mod mesh {
        use cosmic_api::version::v0_0_1 as current;

        pub type Fail = current::fail::mesh::Fail;
    }

    pub mod portal {
        use cosmic_api::version::v0_0_1 as current;

        pub type Fail = current::fail::portal::Fail;
    }

    pub mod resource {
        use cosmic_api::version::v0_0_1 as current;

        pub type Fail = current::fail::resource::Fail;
        pub type Create = current::fail::resource::Create;
        pub type Update = current::fail::resource::Update;
        pub type Select = current::fail::resource::Select;
    }

    pub mod msg {
        use cosmic_api::version::v0_0_1 as current;

        pub type Fail = current::fail::msg::Fail;
    }

    pub mod http {
        use cosmic_api::version::v0_0_1 as current;

        pub type Fail = current::fail::http::Error;
    }

    pub type BadRequest = current::fail::BadRequest;
    pub type BadCoercion = current::fail::BadCoercion;
    pub type Conditional = current::fail::Conditional;
    pub type Timeout = current::fail::Timeout;
    pub type NotFound = current::fail::NotFound;
    pub type Bad = current::fail::Bad;
    pub type Identifier = current::fail::Identifier;
    pub type Illegal = current::fail::Illegal;
    pub type Wrong = current::fail::Wrong;
    pub type Messaging = current::fail::Messaging;
    pub type Fail = current::fail::Fail;
}

pub mod log {
    use cosmic_api::version::v0_0_1 as current;
    pub type Log = current::log::Log;
    pub type LogSpan = current::log::LogSpanEvent;
    pub type LogSpanKind = current::log::LogSpanEventKind;
    pub type LogPayload = current::log::LogPayload;
    pub type LogAppender = dyn current::log::LogAppender;
    pub type RootLogger = current::log::RootLogger;
    pub type RootLogBuilder = current::log::RootLogBuilder;
    pub type LogSource = current::log::LogSource;
    pub type SpanLogBuilder = current::log::SpanLogBuilder;
    pub type PointlessLog = current::log::PointlessLog;
    pub type PointLogger = current::log::PointLogger;
    pub type SpanLogger = current::log::SpanLogger;
}

pub mod cli {
    use cosmic_api::version::v0_0_1 as current;
    pub type CommandTemplate = current::cli::CommandTemplate;
    pub type RawCommand = current::cli::RawCommand;
    pub type Transfer = current::cli::Transfer;
}

pub mod service {
    use cosmic_api::version::{v0_0_1 as current, v0_0_1};

    pub type Global = dyn current::service::Global;
    pub type AccessProvider = dyn current::service::AccessProvider;
    pub type AllAccessProvider = current::service::AllAccessProvider;
}

pub mod sys {
    use cosmic_api::version::{v0_0_1 as current, v0_0_1};
    pub type Assign = current::sys::Assign;
    pub type AssignmentKind = current::sys::AssignmentKind;
    pub type Sys = current::sys::Sys;
}
