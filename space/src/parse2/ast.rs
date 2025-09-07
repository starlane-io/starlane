use crate::parse::model::{BlockSymbol, LexBlock};
use crate::parse2::ast::err::AstErr;
use crate::parse2::document::{Declarations, Definitions, Unit};
use crate::parse2::err::{ParseErrs2, ParseErrs2Def, ParseErrs2Proto};
use crate::parse2::token::{DocType, Token, TokenIter, TokenKind, Tokens};
use crate::parse2::ParseResultProto;
use semver::Version;
use std::fmt::Debug;
use std::marker::PhantomData;

pub(crate) fn ast<'a>(tokens: Tokens<'a>) -> ParseResultProto<'a> {
    let errs = ParseErrs2Def::new();
    let mut iter = tokens.iter();
    todo!();
}


pub enum Ast<'a> {
    Definitions(Block<'a,Definitions<'a>>),
    Err(ParseErrs2<'a>)
}

mod package {
    use crate::parse2::ast::Header;
    use crate::parse2::err::ParseErrs2Proto;
    use crate::parse2::token::TokenIter;

    pub fn header_decl<'a>(iter: &'a mut TokenIter<'a>) -> Result<Header, ParseErrs2Proto<'a>> {
        /// if it success
        todo!()
        //iter.expect("Document Type",&TokenKind::Ident(Ident::Camel(DocType::Package.into())))?;
        //let doc_type = DocType::Package;
    }
}



#[derive(Debug,Clone)]
struct Header {
    doc_type: DocType,
    version: Version
}

pub mod err {
    use crate::parse::CamelCase;
    use crate::parse2::document::Unit;
    use crate::parse2::err::{ParseErrs2Def, ParseErrs2Proto};
    use crate::parse2::token::TokenKind;
    use crate::parse2::Input;
    use std::fmt::{Display, Formatter};
    use std::ops::{Deref, DerefMut};
    use thiserror::Error;

    #[derive(Clone)]
    pub struct Errs<'a> {
        errs: Vec<AstErr<'a>>,
    }
    
    impl <'a> Default for Errs<'a> {
        fn default() -> Self {
            Self::new()
        }
    }
    impl <'a> Errs<'a> {
        
        pub fn new() -> Self {
            Self {
                errs: Default::default()
            }
        } 
    }
    
    impl <'a> Deref for Errs<'a> {
        type Target = Vec<AstErr<'a>>;

        fn deref(&self) -> &Self::Target {
            & self.errs
        }
    }
    
    impl <'a> DerefMut for Errs<'a> {
        fn deref_mut(&mut self) -> &mut Self::Target {
           & mut self.errs
        }
    }
        

    pub type AstErr<'a> = Unit<'a,AstErrKind>;

    impl <'a> Into<ParseErrs2Proto<'a>> for AstErr<'a> {
        fn into(self) -> ParseErrs2Proto<'a> {
            let mut errs = ParseErrs2Proto::new();
            errs.add(self);
            errs
        }
    }

    impl<'a> Display for AstErr<'a> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                write!(f, "{} -- range: [{}..{}]: {}", self.kind, self.span.location_offset(), (self.span.location_offset() + self.span.fragment().len()),self.kind)
        }
    }

    #[derive(Debug, Clone, Error)]
    pub enum AstErrKind {
        #[error("Document type not recognized: '{0}'")]
        DocumentTypeNotRecognized(CamelCase),
        #[error("Expected: {id} {kind}' found: '{found}'")]
        ExpectedKind { id: &'static str, kind: TokenKind, found: TokenKind },
        #[error("Expected '{literal}' found: '{found}'")]
        ExpectedLiteral{ literal: &'static str, found: TokenKind },
        #[error("Expected token type: '{0}' but instead reach EOF (End of File)")]
        UnexpectedEof(TokenKind),
        #[error("Expected whitespace. Found: '{0}'")]
        Whitespace(TokenKind),
        #[error("Version Format Error. Expecting format: 'major.minor.patch-release+label' i.e.: '3.7.8', '2.0.5-rc', '1.2.1-beta+preview'")]
        VersionFormat,
    }

    impl AstErrKind {
        pub fn with(self, span: Input) -> AstErr {
            AstErr::new(span,self)
        }
    }
    
    impl <'a,S> ParseErrs2Def<'a,S> {
        pub fn report(&self, errs: &Vec<AstErr>) {
            todo!("repornt not ready");
            /*
            let r = 0..self.source;
            let mut builder = Report::build(ReportKind::Error, r.clone());
            //for err in errs {
            let err = errs.first().unwrap();
            match &err.kind {
                AstErrKind::DocumentTypeNotRecognized(_) => {}
                AstErrKind::ExpectedKind { .. } => {}
                AstErrKind::ExpectedLiteral { .. } => {}
                AstErrKind::UnexpectedEof(_) => {}
                AstErrKind::Whitespace(_) => {}
                AstErrKind::VersionFormat => {}
            }
            panic!();

             */
        }
    }
}


pub struct Alt<P,O> where P: AstParser, O: AstParser{
    branch: Vec<BranchParser<P,O>>
}

impl <P,O> Alt<P,O> where P: AstParser, O: AstParser{
    pub fn add( & mut self, alt: BranchParser<P,O> ) {
        self.branch.push(alt)
    }
}

struct BranchParser<Pre,Block> where Pre: AstParser, Block: AstParser {
    /// preparser peeks ahead for pattern
    pub pre: Pre,
    pub block: Block
}

impl <P,O> BranchParser<P,O> where P: AstParser, O: AstParser {
    pub fn new<B>(pre: P, block: B) -> Self where B: 'a+AstParser<'a,Output=O> {
        Self {
            pre,
            block: Box::new(block)
        }
    }
}


pub trait AstParser {
  fn parse<'a,O>(&self, tokens: &'a mut TokenIter<'a>) -> Result<O,ParseErrs2Proto<'a>> where O: 'a+Into<Ast<'a>>;
}


#[derive(Debug,Clone)]
pub enum AstBlock<'a> {
    /// `Defs`(version=1.0.1)  { ... }
    Header(Header),
    /// + `arg` {  ... }
    Arg(Declarations<'a>),
    /// + `env` { ... }
    Env(Declarations<'a>),
    /// + `properties`
    PropertyDefs,
    /// a line block is terminated by a semi-colon `;`
    /// `some-param[str] = something`;
    Line,
}


#[derive(Debug,Clone)]
struct Block<'a,C> where C: 'a+Debug+Clone {
    kind: AstBlock<'a>,
    symbol: BlockSymbol,
    open: Unit<'a,&'static str>,
    close: Unit<'a,&'static str>,
    content: Unit<'a,C>
}

pub struct DelimitedParser<'a,O> {
    pre: Box<dyn AstParser<'a,Output=_>>,
    content: Box<dyn AstParser<'a,Output=O>>,
    post: Box<dyn AstParser<'a,Output=_>>,
}

struct LiteralParser(TokenKind);

impl LiteralParser {
    pub fn new(kind: TokenKind) -> Self {
        Self(kind)
    }
}

impl <'a> AstParser<'a> for LiteralParser {
    type Output = &'a Token<'a>;

    fn parse(&self, tokens: &'a mut TokenIter<'a>) -> Result<Self::Output, ParseErrs2Proto<'a>> {
        if let Some(token) = tokens.expect("literal", self.0.clone().into() ) && self.0 == token.kind {
            Ok(token)
        } else {
            let mut errs = ParseErrs2Proto::new();
            let err = AstErr::
            errs.add( )
            
        }
    }
}



impl <'a,O> DelimitedParser<'a,O> {
    pub fn new(kind: BlockSymbol) -> Self {
    }
}



impl <'a,I> AstParser for BlockParser<'a,I> where I: AstParser {
    type Output = Block<'a,I>;
    fn parse<'a>(tokens: &'a mut TokenIter<'a>) -> Result<Self::Output, ParseErrs2Proto<'a>> {
        
    }
}


impl <'a,P> BlockParser<P> where P: AstParser<Output=Ast<'a>> {
    pub fn new(kind: BlockSymbol) -> Self {
        LexBlock
        Self {
            kind,
            inner: PhantomData::default()
        }
    } 
}




