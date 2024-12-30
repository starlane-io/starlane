#![cfg(feature="types2")]

use strum_macros::EnumDiscriminants;

mod class;
mod schema;


pub mod registry;
pub mod specific;
pub mod err;

pub mod domain;
/// meaning where does this Type definition come from
/// * [DefSrc::Builtin] indicates a definition native to Starlane
/// * [DefSrc::Ext] indicates a definition extension defined outside of native Starlane
///                 potentially installed by a package
pub enum DefSrc {
    Builtin,
    Ext,
}

pub(crate) mod private {
    use super::{domain, err, SchemaKind, Type, TypeKind};
    use crate::err::ParseErrs;
    use crate::kind::Specific;
    use crate::parse::util::Span;
    use crate::parse::{camel_case, camel_case_chars, some, CamelCase, Res};
    use crate::point::Point;
    use crate::types::class::ClassKind;
    use crate::types::domain::DomainScope;
    use crate::types::parse::scoped;
    use indexmap::IndexMap;
    use itertools::Itertools;
    use nom::Parser;
    use nom_supreme::ParserExt;
    use rustls::pki_types::Der;
    use std::borrow::Borrow;
    use std::collections::HashMap;
    use std::fmt::Display;
    use std::marker::PhantomData;
    use std::ops::{Deref, DerefMut, Index};
    use std::str::FromStr;
    use tracing::Instrument;

    pub(crate) trait Kind: Clone+Into<TypeKind>+FromStr<Err=ParseErrs>{

        type Type;

        fn category(&self) -> super::TypeCategory;

        fn plus(self, scope: impl ToOwned<Owned=DomainScope>, specific: impl ToOwned<Owned=Specific>) -> Exact<Self> {
            Exact::scoped(scope,self,specific)
        }

        fn parser<I>() -> impl Fn(I) -> Res<I,Self>+Copy where I:Span
        {
            move |input| camel_case_chars.parse_from_str().parse(input)
        }


        fn type_kind(&self) -> TypeKind;
    }


    #[derive(Clone)]
    pub(crate) struct Scoped<I> where I: Clone {
        item: I,
        scope: domain::DomainScope
    }

    impl <K> Scoped<K> where K: Kind {
       pub fn plus_specific(self, specific: Specific ) -> Exact<K> {
           K::plus(self.item,self.scope,specific )
       }
    }

    impl <I> Scoped<I> {
        pub fn new(scope: domain::DomainScope, item:I ) -> Self {
            Self{
                scope,
                item,
            }
        }

        pub fn scope(&self) -> &domain::DomainScope {
            &self.scope
        }
    }

    impl <I> Deref for Scoped<I> {
        type Target = I;

        fn deref(&self) -> &Self::Target {
            &self.item
        }
    }

    impl <K> Into<K> for Scoped<K> where K: Kind {
        fn into(self) -> K {
            self.item
        }
    }

    impl Into<Specific> for Scoped<Specific> {
        fn into(self) -> Specific {
            self.item
        }
    }

    pub(crate) trait Typical: Display+Into<TypeKind>+Into<Type> { }


    pub(crate) struct Meta<K> where K: Kind {
        /// Type is built from `kind` and the specific of the last layer
        kind: K,
        /// types support inheritance and their
        /// multiple type definition layers that are composited.
        /// Layers define inheritance in regular order.  The last
        /// layer is the [Type] of this [Meta] composite.
        ///
        ///
        defs: IndexMap<Specific,Layer>
    }

    impl <K> Meta<K> where K: Kind {
        pub fn new(kind: K, layers: IndexMap<Specific,Layer>) -> Result<Meta<K>,err::TypeErr> {
            if layers.is_empty() {
                Err(err::TypeErr::empty_meta(kind.into()))
            } else {
                Ok(Meta {
                    kind ,
                    defs: Default::default(),
                })
            }
        }

        pub fn typical(&self) -> impl Typical {
            self.kind.clone().plus_specific(self.specific())
        }

        pub fn to_type(&self) -> TypeKind {
            self.typical().into()
        }

        pub fn describe(&self) -> &str{
            format!("Meta definitions for type '{}'", self.typical()).as_str()
        }

        pub fn kind(&self) -> & K{
            &self.kind
        }

        fn first(&self) -> &Layer {
            /// it's safe to unwrap because [Meta::new] will not accept empty defs
            self.defs.first().map(|(_,layer)| layer).unwrap()
        }

        fn layer_by_index(&self, index: impl ToOwned<Owned=usize> ) -> Result<&Layer,err::TypeErr> {
            self.defs.index(index.to_owned()).ok_or(err::TypeErr::meta_layer_index_out_of_bounds(self.kind.clone(), index, self.defs.len() ))
        }

        fn layer_by_specific(&self, specific: impl ToOwned<Owned=Specific> ) -> Result<&Layer,err::TypeErr> {
            self.defs.get(specific.borrow()).ok_or(err::TypeErr::specific_not_found(specific,self.describe()))
        }

        pub fn specific(&self) -> & Specific  {
            &self.first().specific
        }

        pub fn by_index<'x>(&self, index: &usize) -> Result<MetaLayerAccess<'x,K>,err::TypeErr> {
            Ok(MetaLayerAccess::new(self, self.layer_by_index(index)?))
        }

        pub fn by_specific<'x>(&self, specific: &Specific) -> Result<MetaLayerAccess<'x, K>,err::TypeErr> {
            Ok(MetaLayerAccess::new(self, self.layer_by_specific(specific)?))
        }

     }

    pub(crate) struct MetaBuilder<T> where T: Typical{
        typical: T,
        defs: IndexMap<Specific,Layer>
    }

    impl <T> MetaBuilder<T> where T: Typical{
        pub fn new(typical: T) -> MetaBuilder<T>{
            Self {
                typical,
                defs: Default::default()
            }
        }

        pub fn build(self) -> Result<Meta<T>,err::TypeErr> {
            Meta::new(self.typical.into(),self.defs)
        }
    }
    impl <T> Deref for MetaBuilder<T> where T: Typical {
        type Target = IndexMap<Specific,Layer>;

        fn deref(&self) -> &Self::Target {
            & self.defs
        }
    }

    impl <T> DerefMut for MetaBuilder<T> where T: Typical {
        fn deref_mut(&mut self) -> &mut Self::Target {
            & mut self.defs
        }
    }

    pub(crate) struct MetaLayerAccess<'y,K> where K: Kind{
        meta: &'y Meta<K>,
        layer: &'y Layer,
    }

    impl <'y, K> MetaLayerAccess<'y, K> where K: Kind{
        fn new(meta: &'y Meta<K>, layer: &'y Layer) -> MetaLayerAccess<'y, K> {
            Self {
                meta,
                layer
            }
        }

        pub fn get_type(&'y self) -> Exact<K> {
            self.meta.as_type()
        }


        pub fn meta(&'y self) -> &'y Meta<K>  {
            self.meta
        }

        pub fn specific(&'y self) -> &'y Specific  {
            self.meta.specific()
        }

        pub fn layer(&'y self) -> &'y Layer {
            self.layer
        }
    }

    #[derive(Clone)]
    pub(crate) struct Layer {
        specific: Specific,
        classes: HashMap<ClassKind,Ref<ClassKind>>,
        schema: HashMap<SchemaKind,Ref<SchemaKind>>
    }


   /// check if Ref follows constraints

    #[derive(Clone)]
    pub struct Ref<K> where K: Kind  {
        kind: K,
        point: Point,
    }



    #[derive(Clone, Debug, Eq, PartialEq, Hash, ,Serialize,Deserialize)]
    pub(crate) struct Exact<K> where K: Kind{
        scope: domain::DomainScope,
        kind: K,
        specific: Specific,
    }

    impl <K> Typical for Exact<K> where K: Kind{

    }


    impl <K> Into<Type> for Exact<K> where K: Kind
    {
        fn into(self) -> Type {
            self.kind.plus(self.scope,self.specific).into()
        }
    }




    impl <K> Exact<K> where K: Kind
    {
        pub fn new(kind: impl ToOwned<Owned=K>, specific: impl ToOwned<Owned=Specific>) -> Self {
            Self::scoped(kind,specific,Default::default())
         }

        pub fn scoped(scope: impl ToOwned<Owned=domain::DomainScope>,kind: impl ToOwned<Owned=K>, specific: impl ToOwned<Owned=Specific> ) -> Self {
            let kind = kind.to_owned();
            let specific = specific.to_owned();
            let scope = scope.to_owned();
            Self {
                kind,
                specific,
                scope
            }
        }

        pub fn plus_scope(self, scope: impl ToOwned<Owned=domain::DomainScope>) -> Self {
            Self::scoped(self.kind,self.specific,scope)
        }


        pub fn kind(&self) -> &K{
            &self.kind
        }
        pub fn specific(&self) -> &Specific  {
            &self.specific
        }
    }

}



#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TypeCategory))]
#[strum_discriminants(derive(Hash))]
pub enum Type {
    Schema(Schema),
    Class(Class),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants)]
pub enum TypeKind {
    Schema(SchemaKind),
    Class(ClassKind),
}


impl Type {
    pub fn specific(&self) -> &Specific {
        match self {
            Self::Class(class) => class.specific(),
            Self::Schema(schema) => schema.specific()
        }
    }


}

#[derive(Clone,Debug,Eq,PartialEq,Hash)]
struct PointTypeDef<Point,Type> {
    point: Point,
    r#type: Type,
}

#[derive(Clone,Debug,Eq,PartialEq,Hash)]
struct SrcDef<Point,Kind> {
   kind:  Kind,
   point: Point,
}

pub type PointKindDefSrc<Kind> = SrcDef<Point,Kind>;


pub type DataPoint = PointTypeDef<Point, SchemaKind>;


use crate::point::Point;
use crate::types::class::{Class, ClassKind};
use crate::types::private::Kind;
use crate::types::schema::Schema;
pub use schema::SchemaKind;
use starlane_space::kind::Specific;


pub mod parse {
    use crate::err::report::Report;
    use crate::kind::Specific;
    use crate::parse::util::{new_span, result, Span};
    use crate::parse::{camel_case, camel_case_chars, delim_kind, specific, Res, SpaceTree};
    use crate::types::class::{Class, ClassKind};
    use crate::types::private::{Exact, Kind, Scoped};
    use crate::types::schema::Schema;
    use crate::types::{domain::parse::domain, SchemaKind, Type, TypeKind};
    use crate::util::log;
    use crate::{err, types};
    use ascii::AsciiChar::i;
    use nom::branch::alt;
    use nom::combinator::opt;
    use nom::multi::{separated_list0, separated_list1};
    use nom::sequence::{delimited, pair, terminated, tuple};
    use nom::Parser;
    use nom_supreme::parser_ext::FromStrParser;
    use nom_supreme::tag::complete::tag;
    use nom_supreme::ParserExt;
    use std::str::FromStr;
    use wasmer_wasix::runtime::resolver::Source;

    pub(crate) fn parse<K>(s: impl AsRef<str> ) -> Result<Scoped<K>,err::ParseErrs> where K: Kind{
        let span = new_span(s.as_ref());
        result(scoped(span))
    }
    /*
    fn scoped<K:Kind, I: Span>(input: I) -> Res<I, Scoped<K>> where K: Kind {
        tuple((domain,tag("::"),kind))(input).map(|(input,(domain,_,kind))|{
            (input, Scoped::new(domain,kind))
        })
    }

     */
    pub fn scoped<I,F,T>( f: F) -> impl Fn(I) -> Res<I,Scoped<T>> where I: Span, F: Fn(I) -> Res<I,T>+Copy {
        move | input | {
            pair(opt_def(terminated(domain, tag("::"))), f)(input).map(|(input,(scope,item))|(input, Scoped::new(scope, item)))
        }
    }

    /// scope a specific:
    /// `some::scope::'starlane.io'::starlane.io:uberscott.com:starlane:dev:1.3.3`   ;
    pub fn scoped_specific<I>(input:I) -> Res<I,Scoped<Specific>> where I: Span  {
        scoped(specific)(input)
    }

    /// sprinkle in a `Specific` and let's get an `Exact`
    pub fn exact<I,K>( specific: impl ToOwned<Owned=Specific>) -> impl Fn(I) -> Res<I,Exact<K>> where I: Span, K: Kind {
        let specific = specific.to_owned();
        scoped(K::parser).map(|(input,scoped):(I,Scoped<K>)|(input,scoped.plus_specific(specific)))
    }
    /// scan `opt(f) -> Option<D>`  then [Option::unwrap_or_default]  to generate a [D::default] value
    pub fn opt_def<I,F,D>(f: F ) -> impl Fn(I) -> Res<I,D> where I: Span, F: Fn(I) -> Res<I,D>+Copy, D: Default {
        move | input |  {
            opt(f)(input).map(|(input,opt)|opt.unwrap_or_default())
        }
    }

    fn kind<K:Kind,I:Span>( input: I ) -> Res<I,K> where K: Kind {
        camel_case_chars.parse_from_str().parse(input)
    }

    pub fn type_kind<I>( input: I)  -> Res<I,TypeKind> where I: Span {
        alt((class_kind, schema_kind))(input).map(into)
    }

    fn into<I,O>((input,kind):(I,impl Into<O>)) -> (I,O) {
        (input.into(),kind.into())
    }

    pub fn class_kind<I: Span>( input: I)  -> Res<I,ClassKind> {
        camel_case_chars.parse_from_str().parse(input)
    }

    pub fn schema_kind<I: Span>(input: I) -> Res<I,SchemaKind> {
        camel_case_chars.parse_from_str().parse(input)
    }


    pub mod delim {
        use super::scoped;
        use crate::parse::util::Span;
        use crate::parse::Res;
        use crate::types::class::Class;
        use crate::types::private::Scoped;
        use nom::sequence::delimited;
        use nom_supreme::tag::complete::tag;

        pub fn class<I: Span>(input: I) -> Res<I,Scoped<Class>> {
            delimited(tag("<"),scoped,tag(">"))(input)
        }

        pub fn schema<I: Span>(input: I) -> Res<I,Scoped<Class>> {
            delimited(tag("["),scoped,tag("]"))(input)
        }


    }
    #[cfg(test)]
    pub mod test{
        use crate::parse::kind;
        use crate::parse::util::result;
        use crate::util::log;

        #[test]
        pub fn test() {
            let class = log(result(kind("Database"))).unwrap();
        }
    }


}


