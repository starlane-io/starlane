use alloc::string::ToString;
use alloc::vec::Vec;
use core::ops::Deref;
use starlane_primitive_macros::Autobox;
use crate::space::parse::nom::Input;


pub trait ParseCtx: Into<&'static str>{
    fn as_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Autobox,strum_macros::IntoStaticStr)]
pub enum RootCtx {
    TokenCtx(TokenCtx)
}
impl ParseCtx for RootCtx {}

#[derive(Autobox,strum_macros::IntoStaticStr)]
pub enum TokenCtx {

}

impl ParseCtx for TokenCtx{}




pub struct Stack<'a,S> where S: 'a+ Input
{
    top: &'a dyn S,
    spans: Vec<&'a dyn S>
}



impl <'a,S> Deref for Stack<S> where S: 'a+ Input
{
    type Target =  dyn S;

    fn deref(&self) -> &'a Self::Target {
       self.top
    }
}


