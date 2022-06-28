#![allow(warnings)]

#[macro_use]
extern crate cosmic_macros;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate async_trait;

pub mod driver;
pub mod field;
pub mod guest;
pub mod host;
pub mod lifecycle;
pub mod machine;
pub mod portal;
pub mod shell;
pub mod star;
pub mod state;
pub mod traversal;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
