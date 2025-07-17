use std::collections::HashMap;
use std::ops::Deref;
use std::sync::Arc;
use derive_builder::Builder;
use semver::Version;
use serde_derive::{Deserialize, Serialize};
use strum_macros::EnumDiscriminants;
use starlane_macros::Autobox;
use crate::parse2::Input;
use crate::parse::{SkewerCase, VarCase};
use crate::parse2::token::TokenDef;
use crate::point::Point;
use crate::types2::{Primitive, Type};
use crate::types2::data::{Data, DataType};


pub type Document<'a> = DocumentDef<'a,Arc<String>>;
pub type DocumentProto<'a> = DocumentDef<'a,()>;
pub struct DocumentDef<'a,S> {
    pub source: S,
    pub doc: Definitions<'a> 
}

impl <'a> DocumentProto<'a> {
    pub fn new(doc: Definitions<'a> ) -> DocumentDef<'a,()> {
        Self {
            source: (),
            doc,
        }
    }
    
    pub fn promote( self, source: Arc<String>) -> Document<'a> {
        Document {
            source,
            doc: self.doc,
        }
    }
}


pub struct Definitions<'a> {
    pub header: Unit<'a,Header<'a>>,
    pub arg: Declarations<'a>,
    pub env: Declarations<'a>,
    pub property: Declarations<'a>,
}
pub type Declarations<'a> = Unit<'a,HashMap<VarCase, Unit<'a,Declaration<'a>>>>;
pub struct Reference<'a> {
   pub name: Unit<'a,VarCase>,
   pub r#type: Unit<'a,Type>,
}

impl <'a> Reference<'a> {
    pub fn new(name: Unit<'a,VarCase>, r#type: Unit<'a,Type>) -> Self {
        Self { name, r#type }
    }
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    EnumDiscriminants,
    strum_macros::Display,
    Serialize,
    Deserialize,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(RuleType))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]

pub enum Rule {
    #[strum(to_string = "{0}")]
    Optional(Type),
    #[strum(to_string = "{0}")]
    Default(Type),
    #[strum(to_string = "{0}")]
    Type(Type),
}


#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    Hash,
    EnumDiscriminants,
    strum_macros::Display,
    Serialize,
    Deserialize,
)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(AssignmentType))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]

pub enum Assignment{
    #[strum(to_string = "?")]
    Option(Rule),
    #[strum(to_string = "{0}")]
    Type(Type),
}




pub struct Declaration<'a> {
    pub reference: Unit<'a,Reference<'a>>,
    pub assignment: Unit<'a, Rule>,
}

#[derive(Clone,Debug)]
pub struct Unit<'a,I> {
    pub span: Input<'a>,
    pub kind: I 
}

impl <'a,I> Unit<'a,I> {
    pub fn new(span: Input<'a>, kind: I ) -> Unit<'a,I> {
        Self {
            span,
            kind
        }
    }

    fn with<T2>(self, kind: T2) -> Unit<'a, T2> {
        Unit {
            span: self.span,
            kind,
        }
    }
}

impl <'a,I> Deref for Unit<'a,I> {
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.kind
    }
}

pub struct Header<'a> {
    pub version: Unit<'a,Version>, 
}

