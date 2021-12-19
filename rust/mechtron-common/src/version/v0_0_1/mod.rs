
static VERSION : &'static str = "0.0.1";

pub mod core {
    use serde::{Serialize,Deserialize};
    use mesh_portal_serde::version::latest;
    use mesh_portal_serde::version::latest::id::{Address, ResourceType, Kind};
    use mesh_portal_serde::version::latest::payload::Payload;


    #[derive(Clone, Serialize, Deserialize)]
    pub enum Frame {
        Version(String),
        Create(latest::config::Info),
        Assign(latest::config::Info),
        Destroy(latest::id::Address),
        Request(Request)
    }

    pub type Request = latest::portal::outlet::Request;
    pub type Response = latest::portal::outlet::Response;
}

pub mod shell {

    use serde::{Serialize,Deserialize};
    use mesh_portal_serde::version::latest;

    #[derive(Clone, Serialize, Deserialize)]
    pub enum Frame {
        Version(String),
        Request(Request),
        Respond(Response)
    }

    pub type Request = latest::portal::inlet::Request;
    pub type Response = latest::portal::inlet::Response;
}


