use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::Deref;
use starlane_primitive_macros::{AsStr, Autobox};
use crate::space::parse::nom::Input;


pub trait InputCtx where Self: Into<&'static str> {
    fn as_str(&self) -> &'static str {
        self.into()
    }

}

#[derive(strum_macros::IntoStaticStr)]
pub enum RootCtx {
    Token,
    Lex
}

pub enum TokenCtx {

}



#[derive(AsStr)]
pub struct PointCtx;



pub type Stack = Vec<Box<dyn InputCtx>>;



