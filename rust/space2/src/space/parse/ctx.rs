use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::Deref;
use nom_supreme::context::ContextError;
use thiserror::Error;
use starlane_primitive_macros::{AsStr, Autobox};
use crate::space::parse::nomplus::Input;


pub trait ToInputCtx  {
    fn to(self) -> impl Fn()->InputCtx;
}

#[derive(Clone,Error,strum_macros::IntoStaticStr)]
pub enum InputCtx {
 #[error(tranparent)]
 Prim(PrimCtx),
 #[error(tranparent)]
 Case(CaseCtx),
 #[error(tranparent)]
 Point(PointCtx)
}

impl ToInputCtx for InputCtx {
    fn to(self) -> impl Fn()->InputCtx
    {
        move || self
    }
}

#[derive(Clone,Error,strum_macros::IntoStaticStr)]
pub enum PrimCtx {
    #[error("token")]
    Token,
    #[error("lex")]
    Lex,
}

impl ToInputCtx for PrimCtx{
    fn to(self) -> impl Fn()->InputCtx
    {
        move || InputCtx::Prim(self)
    }
}


#[derive(Clone,Debug,Error)]
pub enum CaseCtx {
    #[error("expected skewer case value (lowercase alphanumeric & '-')")]
    SkewerCase,
    #[error("expected CamelCase name (mixed case alphanumeric)")]
    CamelCase,
    #[error("expected variable case name (lowercase alphanumeric & '_')")]
    VarCase,
    #[error("expected filename (mixed case alphanumeric & '_' & '-')")]
    FileCase,
    #[error("expected domain case (mixed case alphanumeric & '-' & '.' )")]
    DomainCase
}



impl ToInputCtx for CaseCtx{
    fn to(self) -> impl Fn()->InputCtx
    {
        move || InputCtx::Case(self)
    }
}




#[derive(Clone,AsStr,Error,Debug,Clone)]
pub enum PointCtx {
    #[error("Var def")]
    Var,
    #[error("RouteSeg")]
    RouteSeg,
    #[error("BasePointSeg")]
    BaseSeg
}

impl ToInputCtx for PointCtx{
    fn to(self) -> impl Fn()->InputCtx
    {
        move || InputCtx::Point(self)
    }
}




pub type Stack = Vec<InputCtx>;



