
static VERSION : &'static str = "0.0.1";

pub mod guest {
    use mesh_portal_serde::version::latest::portal::outlet;

    #[derive(Clone,Serialize,Deserialize)]
    pub struct Call{
        pub to: String,
        pub frame: outlet::Frame
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub enum Frame {
        Version(String),
        Call(Call),
    }
}

pub mod host {

    use mesh_portal_serde::version::latest::portal::inlet;

    #[derive(Clone,Serialize,Deserialize)]
    pub struct Call{
        pub to: String,
        pub frame: inlet::Frame
    }

    #[derive(Clone, Serialize, Deserialize)]
    pub enum Frame {
        Version(String),
        Call(Call),
    }
}
