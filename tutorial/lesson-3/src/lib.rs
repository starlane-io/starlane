#![allow(warnings)]


#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate cosmic_macros;

#[macro_use]
extern crate cosmic_macros_primitive;



pub mod err;
pub mod platform;
pub mod guest;
pub mod mechtron;
pub mod factory;


#[cfg(test)]
pub mod test {
    #[test]
    pub fn test() {}
}
