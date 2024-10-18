use alloc::borrow::ToOwned;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use crate::space::parse::nomplus::err::ParseErr;

pub struct ParseErrsDef<Src>
{
    pub src: Src,
    pub errs : Vec<ParseErr>
}

pub type ParseErrs<'a> = ParseErrsDef<&'a str>;
pub type ParseErrsOwn = ParseErrsDef<String>;

impl <'a> ToOwned for ParseErrsDef<&'a str> {
    type Owned = ParseErrsOwn;

    fn to_owned(&'a self) -> Self::Owned {
        ParseErrsDef::many(self.src.to_string(), self.errs.clone())
    }
}


impl <Src> ParseErrsDef<Src> {
  pub fn new(src: Src, err: ParseErr) -> ParseErrsDef<Src> {
      Self {
          src,
          errs: vec![err]
      }
  }

   pub fn many(src: Src, errs: Vec<ParseErr>) -> ParseErrsDef<Src> {
        Self {
            src,
            errs
        }
    }
}