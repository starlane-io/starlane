use std::slice::Iter;
use crate::parse2::token::{DocType, Token, TokenKind, WhiteSpace};
use semver::Version;
use crate::parse2::ast::err::{AstErr, AstErrKind};
use crate::parse2::ParseOp;

pub(crate) fn ast(tokens: Tokens) {
    let mut iter = tokens.iter();
    match package::header(& mut iter) {
        Ok(_) => {
            panic!("don't know how to take success yet!")
        }
        Err(err) => {
            let errs =vec![err];
            tokens.op.report(&errs);
        }
    }
} 

mod package {
    use crate::parse2::ast::err::{AstErr};
    use crate::parse2::ast::{AstTokenIter, Header};
    use crate::parse2::token::{DocType, Ident, TokenKind};

    pub fn header<'a>(iter: &'a mut AstTokenIter<'a>) -> Result<Header, AstErr> {
        /// if it succe
        iter.expect(&TokenKind::Ident(Ident::Camel(DocType::Package.into())))?;
        let doc_type = DocType::Package;
        todo!();
    }
}



#[derive(Debug,Clone)]
struct Header {
    doc_type: DocType,
    version: Version
}

pub mod err {
    use crate::parse::CamelCase;
    use crate::parse2::token::{Token, TokenKind, WhiteSpace};
    use crate::parse2::{range, ParseOp};
    use ariadne::{Label, Report, ReportKind, Source};
    use std::fmt::{Display, Formatter};
    use std::slice::Iter;
    use thiserror::Error;

    #[derive(Debug, Clone, Error)]
    pub enum AstErr<'a> {
        Token { token: Token<'a>, err: AstErrKind },
        Err(AstErrKind),
    }

    impl<'a> Display for AstErr<'a> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                AstErr::Token { token, err } => {
                    write!(f, "{err} -- range: [{}..{}]", token.span.location_offset(), (token.span.location_offset() + token.span.fragment().len()))
                }
                AstErr::Err(kind) => {
                    write!(f, "{kind}")
                }
            }
        }
    }

    impl<'a> AstErr<'a> {
        pub fn token_err(token: Token<'a>, err: AstErrKind) -> AstErr<'a> {
            Self::Token { token, err }
        }

        pub fn err(err: AstErrKind) -> AstErr<'a> {
            AstErr::Err(err)
        }
    }

    #[derive(Debug, Clone, Error)]
    pub enum AstErrKind {
        #[error("Document type not recognized: '{0}'")]
        DocumentTypeNotRecognized(CamelCase),
        #[error("Expected token type: '{kind}' found: '{found}'")]
        Expected { kind: TokenKind, found: TokenKind },
        #[error("Expected token type: '{0}' but instead reach EOF (End of File)")]
        UnexpectedEof(TokenKind),
        #[error("Expected whitespace. Found: '{0}'")]
        Whitespace(TokenKind),
    }
    impl<'a> ParseOp<'a> {
        pub fn report(&'a self, errs: &'a Vec<AstErr>) {
            let r = 0..self.data.len();
            let mut builder = Report::build(ReportKind::Error, r.clone());
            //for err in errs {
            let err = errs.first().unwrap();
            match err {
                AstErr::Token { token, err } => {
                    let report = builder
                        .with_message("some errors")
                        .with_label(Label::new(range(&token.span)).with_message(err.to_string()))
                        .finish();

                    report.print(Source::from(self.data)).ok();
                }
                AstErr::Err(_) => {
                    panic!();
                }
                // }
            }
        }
    }
}

pub struct Tokens<'a> {
    pub op: &'a ParseOp<'a>,
    pub tokens: Vec<Token<'a>>,
}

impl <'a> Tokens<'a> {
    pub fn new( op: &'a ParseOp<'a>, tokens: Vec<Token<'a>> ) -> Self {
        Self{ op, tokens }
    }
    
    pub fn iter(&'a self) -> AstTokenIter<'a> {
        let iter = self.tokens.iter();
        AstTokenIter::new(iter)
    }
}

pub struct AstTokenIter<'a> {
    iter: Iter<'a,Token<'a>>,
}

impl <'a> AstTokenIter<'a> {
    pub fn new(iter:Iter<'a,Token<'a>>) -> Self {
        AstTokenIter { iter }
    }
    
    /// return the next token that is not whitespace: [TokenKind::Space] || [TokenKind::Newline]
    pub fn skip_ws(&'a mut self) -> Option<&'a Token<'a>> {
        while let Some(token) = self.iter.next()  {
             if !token.kind.is_whitespace(&WhiteSpace::Either) {
                 return Some(token)
             }
        }
        /// out of tokens
        None
    }
    
    pub fn expect(&'a mut self, expect: &TokenKind) -> Result<&'a Token<'a>, AstErr<'a>> {
        let token = self.skip_ws().ok_or_else(move || AstErr::Err(AstErrKind::UnexpectedEof(expect.clone())))?;
        if token.kind == *expect {
            Ok(token)
        } else {
            Err(AstErr::token_err(token.clone(),AstErrKind::Expected { kind: expect.clone(), found: token.kind.clone() } ))
        }
    }

    pub fn space(&'a mut self) -> Result<&'a Token<'a>, AstErr<'a>> {
        self.whitespace_kind(& WhiteSpace::Space)
    }

    pub fn newline(&'a mut self) -> Result<&'a Token<'a>, AstErr<'a>> {
        self.whitespace_kind(& WhiteSpace::Newline)
    }

    pub fn whitespace(&'a mut self) -> Result<&'a Token<'a>, AstErr<'a>> {
        self.whitespace_kind(& WhiteSpace::Either)
    }
    fn whitespace_kind(&'a mut self, whitespace: &'static WhiteSpace) -> Result<&'a Token<'a>, AstErr<'a>> {
        let token = self.next().ok_or_else(move || AstErr::Err(AstErrKind::UnexpectedEof(TokenKind::Space)))?;

        if token.kind.is_whitespace(whitespace) {
            Ok(token)
        } else {
            Err(AstErr::token_err(token.clone(),AstErrKind::Whitespace(token.kind.clone())))
        }
    }



}

impl <'a> Iterator for AstTokenIter<'a> {
    type Item = &'a Token<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}
    
#[cfg(test)]
pub mod test {
    use insta::assert_snapshot;
    use crate::parse2::parse;
    use crate::parse2::token::result;

    #[test]
   fn test() {
        let doc = "Blah";
   }

    #[test]
    pub fn tokenz() {
        let op = parse(
            "tokenz",
            r#"
Release(version=1.3.7){
  + <SomeClass>;
}       
        "#,
        );

        op.parse();
        
        
    }
}    