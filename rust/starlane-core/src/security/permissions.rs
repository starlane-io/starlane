

pub enum Pattern {
    None,
    Any, // *
    Exact(ResourceAddress),

}


pub struct Access<P> {
    pub agent: ResourceKey,
    pub pattern: P,
    pub permission: Permission,
}

pub struct Grant {
    pub permission: Permission,
    pub resource: ResourceKey
}





pub struct Permissions {
    pub create: bool,
    pub read: bool,
    pub write: bool,
    pub execute: bool
}

