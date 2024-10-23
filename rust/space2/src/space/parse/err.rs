use crate::lib::std::borrow::Borrow;
use crate::lib::std::borrow::ToOwned;
use crate::lib::std::string::{String, ToString};
use crate::lib::std::str::FromStr;
use crate::lib::std::vec::Vec;
use crate::lib::std::vec;
use crate::space::parse::nomplus::err::ParseErr;


#[derive(Debug)]
pub struct ParseErrsDef<Src>
{
    pub src: Src,
    pub errs : Vec<ParseErr>
}

pub type ParseErrs<'a> = ParseErrsDef<&'a str>;
pub type ParseErrsOwn = ParseErrsDef<String>;


impl ParseErrsDef<&str> {
    fn to_owned(&self) -> ParseErrsOwn {
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