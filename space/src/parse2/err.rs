use crate::parse::Ctx;
use crate::parse2::ast::err::{AstErr, AstErrKind};
use crate::parse2::{range, Input};
use ariadne::{Report, ReportKind};
use nom_supreme::error::{BaseErrorKind, GenericErrorTree};
use std::ops::Deref;
use std::sync::Arc;

pub type ErrTree<'a> = GenericErrorTree<Input<'a>, &'static str, Ctx, AstErrKind>;

pub struct ParseErrs2Def<'a,S> {
    pub source: S,
    pub errs: Vec<AstErr<'a>>,
}

pub type ParseErrs2Proto<'a> = ParseErrs2Def<'a,()>;
pub type ParseErrs2<'a> = ParseErrs2Def<'a,Arc<String>>;

impl <'a> ParseErrs2Proto<'a> {
    pub fn new() -> ParseErrs2Proto<'a> {
        Self {
            source: (),
            errs: Default::default(),
        }
    }

    pub fn err(&mut self, err: AstErr<'a>) {
        self.errs.push(err)
    }

    pub fn add(&mut self, err: AstErr<'a>) {
        self.errs.push(err)
    }

    pub fn add_all(&'a mut self, errs: &'a Self) {
        for err in errs.iter() {
            self.errs.push(err.clone())
        }
    }

    pub fn promote( self, source: Arc<String>) -> ParseErrs2{
        ParseErrs2 {
            source,
            errs: self.errs,
        }
    }
}


impl <'a,S> Deref for ParseErrs2Def<'a,S> {
    type Target = Vec<AstErr<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.errs
    }
}

impl <'a> From<&'a ErrTree<'a>> for ParseErrs2Proto<'a> {
    fn from(errs: &'a ErrTree<'a>) -> ParseErrs2Proto<'a> {
        let mut rtn = Vec::new();
        match &errs {
            ErrTree::Base { location, ref kind } => {
                let range = range(&location);
                let mut builder = Report::build(ReportKind::Error, range.clone());
                match kind {
                    BaseErrorKind::Expected(expect) => {
                        panic!();
                    }
                    BaseErrorKind::Kind(kind) => {
                        panic!()
                    }
                    BaseErrorKind::External(external) => {
                        rtn.push(external.clone());
                    }
                }
            }
            ErrTree::Stack { base, contexts } => {
                let stacked: ParseErrs2Proto<'a> = base.as_ref().into();
                for s in stacked.errs {
                    rtn.push(s);
                }
            },
            ErrTree::Alt(_) => {
                panic!("\n\nERR !ALT\n\n");
            }
        }
        ParseErrs2Proto{ errs: rtn, source: () }
    }   
}

