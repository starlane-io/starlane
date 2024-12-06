pub mod config;
pub mod err;


pub trait Platform {
    type Config: config::PlatformConfig;
}

