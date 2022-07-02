use alloc::string::String;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use nom_locate::LocatedSpan;


pub mod command;
pub mod config;
pub mod entity;
pub mod frame;
pub mod http;
pub mod id;
pub mod log;
pub mod wave;
pub mod msg;
pub mod parse;
pub mod particle;
pub mod substance;
pub mod portal;
pub mod security;
pub mod selector;
pub mod cli;
pub mod service;
pub mod util;
pub mod sys;
pub mod quota;

use serde::{Deserialize, Serialize};

use crate::error::MsgErr;
use crate::version::v0_0_1::bin::Bin;
use crate::version::v0_0_1::id::id::Uuid;

pub type State = HashMap<String, Bin>;

extern "C" {
    pub fn mesh_portal_uuid() -> Uuid;
    pub fn mesh_portal_timestamp() -> DateTime<Utc>;
}

pub mod artifact {
    use crate::version::v0_0_1::bin::Bin;
    use crate::version::v0_0_1::id::id::Point;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Artifact {
        pub point: Point,
        pub bin: Bin,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ArtifactRequest {
        pub point: Point,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ArtifactResponse {
        pub to: Point,
        pub payload: Bin,
    }
}

pub mod path {
    use crate::error::MsgErr;
    use crate::version::v0_0_1::parse::consume_path;
    use cosmic_nom::new_span;
    use alloc::format;
    use alloc::string::{String, ToString};
    use alloc::vec::Vec;
    use serde::{Deserialize, Serialize};
    use std::str::FromStr;

    #[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
    pub struct Path {
        string: String,
    }

    impl Path {
        fn new(string: &str) -> Self {
            Path {
                string: string.to_string(),
            }
        }

        pub fn make_absolute(string: &str) -> Result<Self, MsgErr> {
            if string.starts_with("/") {
                Path::from_str(string)
            } else {
                Path::from_str(format!("/{}", string).as_str())
            }
        }

        pub fn bin(&self) -> Result<Vec<u8>, MsgErr> {
            let bin = bincode::serialize(self)?;
            Ok(bin)
        }

        pub fn is_absolute(&self) -> bool {
            self.string.starts_with("/")
        }

        pub fn cat(&self, path: &Path) -> Result<Self, MsgErr> {
            if self.string.ends_with("/") {
                Path::from_str(format!("{}{}", self.string.as_str(), path.string.as_str()).as_str())
            } else {
                Path::from_str(
                    format!("{}/{}", self.string.as_str(), path.string.as_str()).as_str(),
                )
            }
        }

        pub fn parent(&self) -> Option<Path> {
            let s = self.to_string();
            let parent = std::path::Path::new(s.as_str()).parent();
            match parent {
                None => Option::None,
                Some(path) => match path.to_str() {
                    None => Option::None,
                    Some(some) => match Self::from_str(some) {
                        Ok(parent) => Option::Some(parent),
                        Err(error) => {
                            eprintln!("{}", error.to_string());
                            Option::None
                        }
                    },
                },
            }
        }

        pub fn last_segment(&self) -> Option<String> {
            let split = self.string.split("/");
            match split.last() {
                None => Option::None,
                Some(last) => Option::Some(last.to_string()),
            }
        }

        pub fn to_relative(&self) -> String {
            let mut rtn = self.string.clone();
            rtn.remove(0);
            rtn
        }
    }

    impl FromStr for Path {
        type Err = MsgErr;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            let (_, path) = consume_path(new_span(s))?;
            Ok(Self {
                string: path.to_string(),
            })
        }
    }

    impl ToString for Path {
        fn to_string(&self) -> String {
            self.string.clone()
        }
    }
}

pub mod bin {
    use std::collections::HashMap;
    use std::sync::Arc;

    use serde::{Deserialize, Serialize};

    pub type Bin = Arc<Vec<u8>>;
}



pub mod fail {
    use alloc::string::String;
    use serde::{Deserialize, Serialize};

    use crate::error::MsgErr;
    use crate::version::v0_0_1::id::id::Specific;

    pub mod mesh {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Error(String),
        }
    }

    pub mod portal {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        use crate::version::v0_0_1::fail::{http, msg, resource};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Error(String),
            Resource(resource::Fail),
            Msg(msg::Fail),
            Http(http::Error),
        }
    }

    pub mod http {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub struct Error {
            pub message: String,
        }
    }

    pub mod resource {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        use crate::version::v0_0_1::fail::{
            Bad, BadCoercion, BadRequest, Conditional, Messaging, NotFound,
        };
        use crate::version::v0_0_1::id::id::Point;

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Create(Create),
            Update(Update),
            Select(Select),
            BadRequest(BadRequest),
            Conditional(Conditional),
            Messaging(Messaging),
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Create {
            AddressAlreadyInUse(String),
            WrongParentResourceType { expected: String, found: String },
            CannotUpdateArchetype,
            InvalidProperty { expected: String, found: String },
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Update {
            Immutable,
        }

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Select {
            WrongAddress { required: Point, found: Point },
            BadSelectRouting { required: String, found: String },
            BadCoercion(BadCoercion),
        }
    }

    pub mod msg {
        use alloc::string::String;
        use serde::{Deserialize, Serialize};

        use crate::version::v0_0_1::fail::{BadRequest, Conditional};

        #[derive(Debug, Clone, Serialize, Deserialize)]
        pub enum Fail {
            Error(String),
            BadRequest(BadRequest),
            Conditional(Conditional),
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum BadRequest {
        NotFound(NotFound),
        Bad(Bad),
        Illegal(Illegal),
        Wrong(Wrong),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct BadCoercion {
        pub from: String,
        pub into: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Conditional {
        Timeout(Timeout),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Timeout {
        pub waited: i32,
        pub message: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum NotFound {
        Any,
        ResourceType(String),
        Kind(String),
        Specific(String),
        Address(String),
        Key(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Bad {
        ResourceType(String),
        Kind(String),
        Specific(String),
        Address(String),
        Key(String),
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Identifier {
        ResourceType,
        Kind,
        Specific,
        Address,
        Key,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Illegal {
        Immutable,
        EmptyToFieldOnMessage,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Wrong {
        pub received: String,
        pub expected: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Messaging {
        RequestReplyExchangesRequireOneAndOnlyOneRecipient,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub enum Fail {
        Mesh(mesh::Fail),
        Resource(resource::Fail),
        Portal(portal::Fail),
        Error(String),
    }

    impl ToString for Fail {
        fn to_string(&self) -> String {
            "Fail".to_string()
        }
    }

    /*    impl Into<MsgErr> for Fail {
           fn into(self) -> MsgErr {
               MsgErr {
                   status: 500,
                   message: "Fail".to_string(),
               }
           }
       }

    */
}
