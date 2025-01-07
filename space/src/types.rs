use std::fmt::Display;
use std::marker::PhantomData;
use std::str::FromStr;
use derive_name::Name;
use nom::branch::alt;
use nom::combinator::{into, opt};
use nom::error::{ErrorKind, FromExternalError};
use nom::sequence::{terminated, tuple};
use nom_supreme::tag::complete::tag;
use once_cell::sync::Lazy;
use strum_macros::EnumDiscriminants;

pub mod class;
pub mod schema;

pub mod registry;
pub mod specific;
pub mod err;

pub mod scope;
pub mod selector;
pub mod def;
pub mod id;
pub mod tag;
pub mod parse;
#[cfg(test)]
pub mod test;
pub mod exact;

//pub(crate) trait Typical: Display+Into<TypeKind>+Into<Type> { }


/// [class::Class::Database] is an example of an [Type] because it is not an [ExactDef]
/// which references a definition in [Specific]
#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants,strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(TypeDiscriminant))]
#[strum_discriminants(derive( Hash, strum_macros::EnumString, strum_macros::ToString, strum_macros::IntoStaticStr ))]
pub enum Type {
    Schema(Schema),
    Class(Class),
}

pub type AsType = dyn Into<ExtType>;
pub type AsTypeKind = dyn Into<Type>;

pub type ExtType = Ext<TypeIdentifier>;

pub type GenExt<G:Generic> = Ext<GenericIdentifier<G>>;
pub type ClassExt = Ext<ClassIdentifier>;

pub type SchemaExt = Ext<SchemaIdentifier>;


#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct Ext<V> where V: ExtVariant
{
    phantom: PhantomData<V>,
    scope: V::Scope,
    r#type: V::Type,
    specific: V::Specific,
}



impl<V> Display for Ext<V> where V: ExtVariant

{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scope = self.scope.to_string();
        let str = if !scope.is_empty() {
            format!("{}::{}@{}", self.scope, self.r#type, self.specific)
        } else {
            format!("{}@{}", self.r#type, self.specific)
        };
        write!(f, "{}", str)
    }
}



/// binds the various elements to support `Identifier`, `Selector` and `Context` variants
pub trait ExtVariant {
   type Scope: PrimitiveArchetype<Parser:PrimitiveParser<Output=Self::Scope>>;
   type Type: Archetype;
   type Specific: SpecificVariant;

   fn parser() -> impl ExtParser<Self> {
       ExtParser::new()
   }
}



pub struct TypeIdentifier;
pub struct ClassIdentifier;
pub struct SchemaIdentifier;
pub struct GenericIdentifier<G>(PhantomData<G>) where G: Generic;

impl ExtVariant for TypeIdentifier {
    type Scope = Scope;
    type Type = Type;
    type Specific = Specific;
}

impl ExtVariant for ClassIdentifier {
    type Scope = Scope;
    type Type = Class;
    type Specific = Specific;
}
impl ExtVariant for SchemaIdentifier {
    type Scope = Scope;
    type Type = Schema;
    type Specific = Specific;
}

impl <G> ExtVariant for GenericIdentifier<G> where G: Generic{
    type Scope = Scope;
    type Type = G;
    type Specific = Specific;
}

/// is able to ascertain the desired abstract
pub trait TypeFactory {


}





pub static CLASS_NESTED_BLOCK_KIND: Lazy<Option<NestedBlockKind>> =
    Lazy::new(|| Option::Some(NestedBlockKind::Angle));

pub static SCHEMA_NESTED_BLOCK_KIND: Lazy<Option<NestedBlockKind>> =
    Lazy::new(|| Option::Some(NestedBlockKind::Square));


pub static BLOCKS: Lazy<Vec<BlockKind>> =
    Lazy::new(|| vec![BlockKind::Nested(NestedBlockKind::Angle),BlockKind::Nested(NestedBlockKind::Square)]);

impl ExtType {

    fn parse_outer<I>(input: I) -> Res<I,Self> where I: Span{
        todo!()

    }

}



/*
impl <Scope,Abstract,Specific> ExactGen<Scope,Abstract,Specific> {
    pub fn new( scope: Scope, r#abstract: Abstract, specific: Specific ) -> ExactGen<Scope,Abstract,Specific>{
        Self {scope, r#abstract, specific}
    }
}

 */

impl From<Class> for Type {
    fn from(kind: Class) -> Self {
        Self::Class(kind)
    }
}

impl From<Schema> for Type {
    fn from(kind: Schema) -> Self {
        Self::Schema(kind)
    }
}

impl Type {
    pub fn convention(&self) -> Case {
        /// it so happens everything is CamelCase, but that may change...
        Case::CamelCase
    }
}

pub enum Case {
    CamelCase,
    SkewerCase
}

impl Case {
    pub fn validate(&self, text: &str) -> Result<(),ParseErrs> {

        /// transform from [Result<Whatever,ParseErrs>] -> [Result<(),ParseErrs?]
        fn strip_ok<Ok,Err>( result: Result<Ok,Err>) -> Result<(), Err> {
            result.map(|_|())
        }

        match self {
            Case::CamelCase =>  strip_ok(CamelCase::from_str(text)),

            Case::SkewerCase => strip_ok(SkewerCase::from_str(text))
        }
    }

    /*
    pub fn parser<I,O>(&self) -> impl Fn(I) -> Res<I,O> where I: Span {
        match self {
            Case::CamelCase => CamelCase::parser,
            Case::SkewerCase => SkewerCase::parser,
        }
    }

     */

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


pub type DataPoint = PointTypeDef<Point, Schema>;


/// meaning where does this Type definition come from
/// * [DefSrc::Builtin] indicates a definition native to Starlane
/// * [DefSrc::Ext] indicates a definition extension defined outside of native Starlane
///                 potentially installed by a package
pub enum DefSrc {
    Builtin,
    Ext,
}

















use crate::err::ParseErrs;
use crate::parse::{lex_block_alt, CamelCase, NomErr, Res, SkewerCase};
use crate::point::Point;
use crate::types::private::Generic;
pub use schema::Schema;
use specific::Specific;
use starlane_space::types::private::Variant;
use starlane_space::types::specific::SpecificVariant;
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::Span;
use crate::types::class::Class;
use crate::types::parse::{Archetype, ExtParser, PrimitiveArchetype, PrimitiveParser, SpecificParser, TypeParsers};
use crate::types::scope::Scope;


pub(crate) mod private {
    use super::{err, Type, ExtType, Ext, Schema, Case, parse, GenExt, ExtVariant};
    use super::specific::Specific;
    use crate::parse::util::Span;
    use crate::parse::{camel_case, CamelCase, NomErr, Res};
    use crate::point::Point;
    use crate::types;
    use crate::types::class::Class;
    use crate::types::scope::{Scope, Segment};
    use indexmap::IndexMap;
    use itertools::Itertools;
    use nom::{IResult, Parser};
    use std::collections::{HashMap, HashSet};
    use std::fmt::{Debug, Display, Formatter};
    use std::hash::Hash;
    use std::marker::PhantomData;
    use std::ops::{Deref, DerefMut, Index};
    use std::str::FromStr;
    use std::sync::Arc;
    use ascii::AsciiChar::i;
    use chrono::ParseResult;
    use cliclack::input;
    use derive_name::Name;
    use nom::bytes::complete::tag;
    use nom::combinator::{cond, fail, into, opt, peek, value};
    use nom::error::{ErrorKind, FromExternalError, ParseError};
    use nom::error::VerboseErrorKind::Nom;
    use nom::sequence::{delimited, pair};
    use nom_supreme::ParserExt;
    use strum_macros::EnumDiscriminants;
    use starlane_space::types::parse::TypeParsers;
    use crate::parse::model::{BlockKind, NestedBlockKind};
    use crate::types::parse::{PrimitiveArchetype, PrimitiveParser};
    use crate::types::parse::util::VariantStack;

    pub(crate) trait Generic: PrimitiveArchetype<Parser: TypeParsers<Discriminant=Self::Discriminant>> +TryFrom<VariantStack<Self>>+Name+Clone+Into<Type>+Clone+FromStr+Display{

        type Discriminant;

        type Segment:  PrimitiveArchetype<Parser:PrimitiveParser>;


        fn max_stack_size() -> usize {
            2usize
        }

        fn of_type() -> &'static super::TypeDiscriminant;

        fn plus(self, scope: Scope, specific: Specific) -> GenExt<Self> {
            GenExt::new(scope,self,specific)
        }





        fn block() -> &'static NestedBlockKind;

        /// wrap the string value in it's `type` wrapper.
        ///
        /// for example:  [Class::to_string] for [Class::Database] would `Database`, or a variant like
        /// [Class::Service(Service::Database)] to_string would return `Service<Database>` and
        /// [Class::wrapped_string] would return `<Database>` and `<Service<Database>` respectively
        fn wrapped_string(&self) -> String {
            Self::block().wrap(self.to_string())
        }
    }


    /// [Variant] implies inheritance from a
    pub(crate) trait Variant where Self: Into<Self::Type> {
        /// the base [Type] variant [Class] or [Schema]
        type Type: Generic+?Sized;

        type Discriminant;
    }



    #[cfg(feature="groups")]
    pub mod group {
        #[derive(Clone, Debug, Eq, PartialEq, Hash)]
        pub struct AbstractSegment(CamelCase);
        impl Segment for AbstractSegment {
            fn delimiter() -> &'static str {
                ":"
            }

            fn parse<I>(input: I) -> Res<I, Self>
            where
                I: Span
            {
                camel_case(input).map(|(next, camel)| (next, Self(camel)))
            }
        }

        impl Display for AbstractSegment {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0.to_string())
            }
        }


        pub trait Segment: Clone + Debug + Eq + PartialEq + Hash + Serialize + Deserialize + Display {
            fn delimiter() -> &'static str;

            fn parse<I>(input: I) -> Res<I, Self>
            where
                I: Span;
        }

        #[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
        pub struct Segments<S>
        where
            S: Segment + ?Sized
        {
            segments: Vec<S>
        }

        impl<S> Display for Segments<S>
        where
            S: Segment
        {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.segments.iter().join(S::delimiter()))
            }
        }

    }


    /*


    /// [Member] of a [Group] for scoping purposes
    pub(crate) trait Member {
        fn group(&self) -> Group;

        fn root(&self) -> Abstract {
            match self.group() {
                Group::Root(root) => root,
                Group::Parent(s) => s.root(),
            }
        }
    }

    #[derive(EnumDiscriminants,strum_macros::Display)]
    #[strum_discriminants(vis(pub))]
    #[strum_discriminants(name(SuperDiscriminant))]
    #[strum_discriminants(derive( Hash, strum_macros::EnumString, strum_macros::ToString, strum_macros::IntoStaticStr ))]
    pub enum Super<A,V> where A: Generic+?Sized, V: Variant<Root=A>+?Sized {
        /// the `root` [Abstract] variant [Generic] that a [Variant] derives from.
        Root(A),
        /// the `super` [Variant] of this [Variant] (which is not a `root`)
        Super(V),
    }

    #[derive(EnumDiscriminants,strum_macros::Display)]
    #[strum_discriminants(vis(pub))]
    #[strum_discriminants(name(GroupDiscriminant))]
    #[strum_discriminants(derive( Hash, strum_macros::EnumString, strum_macros::ToString, strum_macros::IntoStaticStr ))]
    pub enum Group {
        /// the `root` group must be an [Abstract]
        Root(Abstract),
        /// parent
        Parent(Box<dyn Member>),
    }



     */

    /*
    impl <K> Into<K> for Scoped<K> where K: Kind {
        fn into(self) -> K {
            self.item
        }
    }

     */


    pub struct Group {
        members: HashSet<Type>,
        subs: HashMap<Type, Box<HashSet<Type>>>
    }

    pub(crate) struct Meta<G> where G: Generic
    {
        /// Type is built from `kind` and the specific of the last layer
        generic: G,
        /// types support inheritance and their
        /// multiple type definition layers that are composited.
        /// Layers define inheritance in regular order.  The last
        /// layer is the [Ext] of this [Meta] composite.
        defs: IndexMap<Specific,Layer>
    }

    impl <K> Meta<K> where K: Generic
    {
        pub fn new(kind: K, layers: IndexMap<Specific,Layer>) -> Result<Meta<K>,err::TypeErr> {
            if layers.is_empty() {
                Err(err::TypeErr::empty_meta(kind.into()))
            } else {
                Ok(Meta {
                    generic: kind,
                    defs: Default::default(),
                })
            }
        }

        pub fn to_abstract(&self) -> Type {
            self.generic.clone().into()
        }

        pub fn describe(&self) -> String {
            todo!()
//            format!("Meta definitions for type '{}'", Self::name(())
        }

        pub fn generic(&self) -> & K{
            &self.generic
        }

        fn first(&self) -> &Layer {
            /// it's safe to unwrap because [Meta::new] will not accept empty defs
            self.defs.first().map(|(_,layer)| layer).unwrap()
        }

        fn layer_by_index(&self, index: usize ) -> Result<&Layer,err::TypeErr> {
            self.defs.get_index(index).ok_or(err::TypeErr::meta_layer_index_out_of_bounds(&self.generic.clone().into(), &index, self.defs.len() )).map(|(_,layer)|layer)
        }

        fn layer_by_specific(&self, specific: &Specific ) -> Result<&Layer,err::TypeErr> {
            self.defs.get(&specific).ok_or(err::TypeErr::specific_not_found(specific.clone(),self.describe()))
        }

        pub fn specific(&self) -> & Specific  {
            &self.first().specific
        }

        pub fn by_index<'x>(&'x self, index: usize) -> Result<MetaLayerAccess<'x,K>,err::TypeErr> {
            Ok(MetaLayerAccess::new(self, self.layer_by_index(index)?))
        }

        pub fn by_specific<'x>(&'x self, specific: &Specific) -> Result<MetaLayerAccess<'x, K>,err::TypeErr> {
            Ok(MetaLayerAccess::new(self, self.layer_by_specific(specific)?))
        }

    }

    pub(crate) struct MetaBuilder<T> where T: Generic{
        r#type: T,
        defs: IndexMap<Specific,Layer>
    }

    impl <T> MetaBuilder<T> where T: Generic
    {
        pub fn new(typical: T) -> MetaBuilder<T>{
            Self {
                r#type: typical,
                defs: Default::default()
            }
        }

        pub fn build(self) -> Result<Meta<T>,err::TypeErr> {
            todo!();
//            Meta::new(self.r#type.into(), self.defs)
        }
    }
    impl <T> Deref for MetaBuilder<T> where T: Generic{
        type Target = IndexMap<Specific,Layer>;

        fn deref(&self) -> &Self::Target {
            & self.defs
        }
    }

    impl <T> DerefMut for MetaBuilder<T> where T: Generic{
        fn deref_mut(&mut self) -> &mut Self::Target {
            & mut self.defs
        }
    }

    pub(crate) struct MetaLayerAccess<'y,K> where K: Generic
    {
        meta: &'y Meta<K>,
        layer: &'y Layer,
    }

    impl <'y, K> MetaLayerAccess<'y, K> where K: Generic
    {
        fn new(meta: &'y Meta<K>, layer: &'y Layer) -> MetaLayerAccess<'y, K> {
            Self {
                meta,
                layer
            }
        }

        pub fn get_type(&'y self) -> Type {
            self.meta.to_abstract()
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
        classes: HashMap<Class,ClassPointRef>,
        schema: HashMap<Schema,SchemaPointRef>
    }

    pub type ClassPointRef = Ref<Point,Class>;
    pub type SchemaPointRef = Ref<Point,Schema>;
    pub type GenericPointRef<G:Generic> = Ref<Point,G>;
    pub type ExactPointRef = Ref<Point, ExtType>;

    #[derive(Clone,Eq,PartialEq,Hash)]
    pub struct Ref<I,K> where I: Clone+Eq+PartialEq+Hash, K: Clone+Eq+PartialEq+Hash
    {
        id: I,
        r#type: K,
    }






    impl <V> Ext<V> where  V: ExtVariant
    {

            pub fn new(scope: V::Scope, r#type: V::Type, specific: V::Specific) -> Self {
                Self {
                    phantom: PhantomData::default(),
                    scope,
                    r#type,
                    specific
                }
            }


        pub fn plus_scope(self, scope: V::Scope) -> Self {
            Self::new(scope, self.r#type, self.specific)
        }

        pub fn plus_specific(self, specific: V::Specific ) -> Self {
            Self::new(self.scope, self.r#type, specific)
        }

        pub fn get_type(&self) -> &V::Type {
            &self.r#type
        }
        pub fn specific(&self) -> &V::Specific {
            &self.specific
        }
    }



}



