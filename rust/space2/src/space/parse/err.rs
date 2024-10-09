use crate::space::parse::util::Trace;


pub struct CtxStack {

    contexts: Vec<ParseCtx>
}
pub struct ParseCtx {
    track: Trace,
    loc: ParseLoc
}

pub enum ParseLoc{
   Doc
}


pub struct ParseErrs {


}
