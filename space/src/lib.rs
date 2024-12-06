use std::str::FromStr;
use once_cell::sync::Lazy;
use crate::point::Point;

pub mod artifact;
pub mod asynch;
pub mod command;
pub mod config;
pub mod err;
pub mod fail;
pub mod frame;
pub mod hyper;
pub mod kind;
pub mod parse;
pub mod particle;
pub mod wave;

#[cfg(feature = "kind2")]
pub mod kind2;

pub mod loc;
pub mod log;
pub mod path;
pub mod point;
pub mod security;
pub mod selector;
pub mod settings;
pub mod substance;
pub mod util;
pub mod wasm;

pub mod prelude;
pub mod progress;


pub static VERSION: Lazy<semver::Version> =
    Lazy::new(|| semver::Version::from_str(env!("CARGO_PKG_VERSION").trim()).unwrap());

pub static HYPERUSER: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:hyperuser").expect("point"));
pub static ANONYMOUS: Lazy<Point> =
    Lazy::new(|| Point::from_str("hyperspace:users:anonymous").expect("point"));

#[cfg(test)]
pub mod test {
    use crate::VERSION;

    #[test]
    pub fn test_version() {
        println!("{}", VERSION.to_string());
    }
}
