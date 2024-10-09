use std::ops::{Deref, DerefMut};
use crate::space::parse::util::{SliceStr, Span, Wrap};

pub struct ParseCtx {
    slice: SliceStr,
    loc: ParseLoc
}


pub trait ParseLoc {

}

pub struct SpanCtx<'a,S,L> where S: 'a+Span, L: 'a+ParseLoc{
    span: &'a dyn S,
    loc: L
}

impl <'a,S,L> SpanCtx<'a,S,L> where S: 'a+Span , L: 'a+ParseLoc {
    pub fn push<'b,C>(&'a self, span: &'b S, loc: C) -> SpanCtx<'b,S,C> where C: 'b+ParseLoc {
        SpanCtx {
            span,
            loc,
        }
    }
}

impl <'a,S,L> Drop for SpanCtx<'a,S,L> where S: 'a+Span , L: 'a+ParseLoc {
    fn drop(&mut self) {
        // must somehow report jhow
    }
}

pub struct Stack<'a,S> where S: 'a+Span {
    top: &'a dyn S,
    spans: Vec<&'a dyn S>
}



impl <'a,S> Deref for Stack<S> where S: 'a+Span {
    type Target =  dyn S;

    fn deref(&self) -> &'a Self::Target {
       self.top
    }
}


