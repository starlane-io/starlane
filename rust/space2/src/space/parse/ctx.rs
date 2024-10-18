use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::Deref;
use nom_supreme::context::ContextError;
use starlane_primitive_macros::{AsStr, Autobox};
use crate::space::parse::nomplus::Input;


pub trait ToInputCtx  {
    fn to<F>(self) -> F where F: Fn() -> InputCtx;
}

#[derive(strum_macros::IntoStaticStr)]
pub enum InputCtx {
Prim(PrimCtx)
}

impl ToInputCtx for InputCtx {
    fn to<F>(self) -> F
    where
        F: Fn() -> InputCtx
    {
        move || self
    }
}

#[derive(strum_macros::IntoStaticStr)]
pub enum PrimCtx {
    #[strum(serialize = "token")]
    Token,
    Lex
}

impl ToInputCtx for PrimCtx{
    fn to<F>(self) -> F
    where
        F: Fn() -> InputCtx
    {
        move || InputCtx::Prim(self)
    }
}




#[derive(AsStr)]
pub struct PointCtx;


pub type Stack = Vec<InputCtx>;



