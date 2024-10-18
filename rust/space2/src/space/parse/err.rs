use alloc::vec::Vec;
use crate::space::parse::nomplus::err::ParseErr;

pub struct ParseErrs<'a>
{
    pub src: &'a str,
    pub errs : Vec<ParseErr>
}