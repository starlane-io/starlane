use serde::{Deserialize, Serialize};



pub mod inlet{
    use std::convert::TryFrom;
    use mesh_portal_serde::error::Error;
    use mesh_portal_serde::version::latest::artifact::{ArtifactRequest, ArtifactResponse};
    use mesh_portal_serde::version::latest::id::Address;
    use mesh_portal_serde::version::latest::messaging::{Request, Response};
    use mesh_portal_serde::version::latest::portal;
    use mesh_portal_serde::version::latest::portal::Exchanger;
    use serde::{Serialize,Deserialize};

    #[derive(Debug,Clone,Serialize,Deserialize)]
    pub enum LogSrc {
        Wasm(Address),
        Mechtron(Address)
    }

    impl ToString for LogSrc {
        fn to_string(&self) -> String {
           match self {
               LogSrc::Wasm(address) => format!("Wasm({})",address.to_string()),
               LogSrc::Mechtron(address) => format!("Mechtron({})",address.to_string())
           }
        }
    }

    #[derive(Debug,Clone,Serialize,Deserialize)]
    pub struct Log {
        pub message: String,
        pub src: LogSrc
    }

    impl ToString for Log {
        fn to_string(&self) -> String {
            format!("{}: {}", self.src.to_string(), self.message )
        }
    }

    impl Into<portal::inlet::Log> for Log {
        fn into(self) -> portal::inlet::Log {
            portal::inlet::Log {
                src: self.src.to_string(),
                message: self.message
            }
        }
    }

    #[derive(Debug,Clone,Serialize,Deserialize)]
    pub enum Frame {
        Log(Log),
        ArtifactRequest(Exchanger<ArtifactRequest>),
    }

    impl Into<portal::inlet::Frame> for Frame {
        fn into(self) -> portal::inlet::Frame {
            match self {
                Frame::Log(log) => {
                    portal::inlet::Frame::Log(log.into())
                }
                Frame::ArtifactRequest(request) => {
                    portal::inlet::Frame::Artifact(request)
                }
            }
        }
    }
}


pub mod outlet {
    use std::convert::TryFrom;
    use mesh_portal_serde::error::Error;
    use mesh_portal_serde::version::latest::artifact::ArtifactResponse;
    use mesh_portal_serde::version::latest::config::Assign;
    use mesh_portal_serde::version::latest::messaging::{Request, Response};
    use mesh_portal_serde::version::latest::portal;
    use mesh_portal_serde::version::latest::portal::Exchanger;
    use mesh_portal_serde::version::latest::resource::ResourceStub;
    use serde::{Serialize,Deserialize};

    #[derive(Debug,Clone,Serialize,Deserialize)]
    pub enum Frame {
        Assign(Assign),
        ArtifactResponse(Exchanger<ArtifactResponse>),
    }

    impl Into<portal::outlet::Frame> for Frame {
        fn into(self) -> portal::outlet::Frame {
            match self {
                Frame::Assign(assign) => {
                    let assign = Exchanger::new(assign);
                    portal::outlet::Frame::Assign(assign)
                }
                Frame::ArtifactResponse(response) => {
                    portal::outlet::Frame::Artifact(response)
                }
            }
        }
    }

    impl TryFrom<portal::outlet::Frame> for Frame {
        type Error = Error;

        fn try_from(frame: portal::outlet::Frame) -> Result<Self, Self::Error> {
            match frame {
                portal::outlet::Frame::Assign(assign) => {
                    Ok(Frame::Assign(assign.item))
                }
                portal::outlet::Frame::Artifact(response) => {
                    Ok(Frame::ArtifactResponse(response))
                }
              _ => {
                    Err("no matching mechtron Frame".into())
                }
            }
        }
    }
}
