use crate::lib::std::borrow::Borrow;
use crate::lib::std::borrow::ToOwned;
use crate::lib::std::string::{String, ToString};
use crate::lib::std::str::FromStr;
use crate::lib::std::vec::Vec;
use crate::lib::std::vec;
use crate::space::parse::nomplus::err::ParseErr;

pub struct ParseErrsDef<Src>
{
    pub src: Src,
    pub errs : Vec<ParseErr>
}

pub type ParseErrs<'a> = ParseErrsDef<&'a str>;
pub type ParseErrsOwn = ParseErrsDef<String>;

impl <'a> Borrow<ParseErrsDef<&'a str>> for ParseErrsDef<String>{
    fn borrow(&self) -> &'a ParseErrs<'a> {
        &ParseErrs{
            src : self.src.as_str(),
            errs : self.errs.clone()
        }
    }
}

impl <'a> ToOwned for ParseErrsDef<&'a str> {
    type Owned = ParseErrsDef<String>;

    fn to_owned(&self) -> Self::Owned {
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