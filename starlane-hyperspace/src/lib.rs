pub mod err;
pub mod global;
pub mod layer;
pub mod machine;
pub mod reg;
pub mod star;

pub mod driver;
pub mod executor;
pub mod foundation;
pub mod host;
pub mod hyperlane;
pub mod platform;
pub mod properties;
pub mod registry;
pub mod shutdown;
pub mod tests;

pub mod database;
pub mod service;
pub mod template;

pub mod starlane {
    pub extern crate starlane_space as space;
}

mod space {
    pub use starlane_space::*;
}

#[cfg(test)]
pub mod tests {
    #[test]
    pub fn test() {}
}
