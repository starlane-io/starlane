use derive_name::Name;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::combinator::{into, not, opt, value};
use nom::error::{ErrorKind, ParseError};
use nom::multi::many1;
use nom::sequence::{delimited, tuple, Tuple};
use nom_supreme::context::ContextError;
use std::fmt::Display;
use std::str::FromStr;
use strum_macros::EnumDiscriminants;
use thiserror::Error;

pub mod class;
pub mod data;

pub mod err;
pub mod registry;
pub mod specific;

pub mod id;
pub mod parse;
pub mod scope;
pub mod selector;
pub mod tag;
#[cfg(test)]
pub mod test;
//pub(crate) trait Typical: Display+Into<TypeKind>+Into<Type> { }

/// [class::Class::Database] is an example of an [Abstract] because it is not an [ExactDef]
/// which references a definition in [Specific]
#[derive(Clone, Debug, Eq, PartialEq, Hash, EnumDiscriminants, strum_macros::Display)]
#[strum_discriminants(vis(pub))]
#[strum_discriminants(name(AbstractDisc))]
#[strum_discriminants(derive(
    Hash,
    strum_macros::EnumString,
    strum_macros::ToString,
    strum_macros::IntoStaticStr
))]
pub enum Abstract {
    Class(Class),
    Data(Data),
}

impl Abstract {
    pub fn outer_parser<I, F, O>(inner: F) -> impl FnMut(I) -> Res<I, O>
    where
        F: FnMut(I) -> Res<I, O> + Copy,
        O: From<Class> + From<Data>,
        I: Span,
    {
        move |i| {
            alt((
                Class::delimited_parser(inner),
                Data::delimited_parser(inner),
            ))(i)
        }
    }
}

impl AbstractParsable for Abstract {

    fn abstract_parser<I>(disc: &AbstractDisc) -> impl FnMut(I) -> Res<I, Self>
    where
        I: Span,
    {
        match disc {
            AbstractDisc::Class => |i| into(Class::parser)(i),
            AbstractDisc::Data => |i| into(Data::parser)(i),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Generic<Scope, Abstract, Specific>
where
    Scope: Parsable + Default,
    Abstract: Clone,
    Specific: Clone,
{
    pub scope: Scope,
    pub r#abstract: Abstract,
    pub specific: Specific,
}

/// [Cat] stands for `category` ... a [Generic] is a category
/// if a [Specific] is not supplied
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Cat<Scope, Abstract>
where
    Scope: Parsable + Default,
{
    pub scope: Scope,
    pub r#abstract: Abstract,
}

impl<Scope,Abstract,Specific> From<Generic<Scope,Abstract,Specific>> for Cat<Scope,Abstract> where Scope: Parsable+Default, Specific: std::clone::Clone, Specific: std::clone::Clone, Abstract: std::clone::Clone{
    fn from(gen: Generic<Scope, Abstract, Specific>) -> Self {
        Self {
            scope: gen.scope,
            r#abstract: gen.r#abstract,
        }
    }
}

impl AbstractDisc {
    pub fn get_delimiters(&self) -> (&'static str, &'static str) {
        match self {
            AbstractDisc::Class => ("<", ">"),
            AbstractDisc::Data => ("[", "]"),
        }
    }

    pub fn delimited_parser<I, O>(
        &self,
        f: impl FnMut(I) -> Res<I, O>,
    ) -> impl FnMut(I) -> Res<I, O>
    where
        I: Span,
    {
        match self {
            AbstractDisc::Class => Class::delimited_parser(f),
            AbstractDisc::Data => Class::delimited_parser(f),
        }
    }
}

impl From<Class> for Abstract {
    fn from(value: Class) -> Self {
        Abstract::Class(value)
    }
}

impl From<Data> for Abstract {
    fn from(value: Data) -> Self {
        Abstract::Data(value)
    }
}

pub type AsType = dyn Into<Full>;
pub type AsTypeKind = dyn Into<Abstract>;

pub type FullAbstract<Abstract: Parsable> = Generic<Scope, Abstract, Specific>;

pub type Full = Generic<Scope, Abstract, Specific>;

pub type FullSelector = Generic<Pattern<Scope>, Pattern<Abstract>, SpecificSelector>;


#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct GenericLex<Scope,Specific> where Scope: Parsable+Default, Specific: Parsable {
    generic: Generic<Scope,CamelCase,Option<Specific>>,
    disc: AbstractDisc,
}

impl <Specific> Parsable for GenericLex<Scope,Specific> where Specific: Parsable{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span
    {
        let (_,disc) = identify_abstract_disc(input.clone())?;
        tuple((Scope::parser, camel_case, opt(preceded(tag("@"),Specific::parser))))(input).map( |(next,(scope,r#abstract,specific))| {
            let generic = Generic {
                scope,
                r#abstract,
                specific,
            };

            let lex = Self {
                generic,
                disc
            };

            (next,lex)
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash,Error)]
pub enum CastErr {
    #[error("a `Cat` (category) cannot be turned into a `Generic` that requires a `Specific`")]
    MissingSpecific
}

impl <Scope,Abstract,Specific> TryInto<Generic<Scope,Abstract,Specific>> for GenericLex<Scope,Specific> where Scope: Parsable+Default, Specific: Parsable, Abstract: From<CamelCase>+Clone{
    type Error = CastErr;

    fn try_into(self) -> Result<Generic<Scope, Abstract, Specific>, Self::Error> {
        if let Some(specific) = self.generic.specific {
            Ok(Generic {
                scope: self.generic.scope,
                r#abstract: self.generic.r#abstract.into(),
                specific
            })
        } else {
            Err(CastErr::MissingSpecific)
        }
    }
}



impl <Scope,Abstract,Specific> Into<Cat<Scope,Abstract>> for GenericLex<Scope,Specific> where Scope: Parsable+Default, Abstract: From<CamelCase>+Clone, Specific: Parsable {
    fn into(self) -> Cat<Scope, Abstract> {
        Cat{
            scope: self.generic.scope,
            r#abstract: self.generic.r#abstract.into(),
        }
    }
}


impl<Scope, Specific> Generic<Scope, Class, Specific>
where
    Scope: Parsable + Default,
    Specific: Parsable,
{
    fn abstract_disc(&self) -> &'static AbstractDisc {
        &AbstractDisc::Class
    }
}

impl<Scope, Specific> Generic<Scope, Data, Specific>
where
    Scope: Parsable + Default,
    Specific: Parsable,
{
    fn abstract_disc(&self) -> &'static AbstractDisc {
        &AbstractDisc::Data
    }
}




impl<Scope, Specific> AbstractParsable for Generic<Scope, Abstract, Specific>
where
    Scope: Parsable + Default,
    Specific: Parsable,
{
    fn abstract_parser<I>(disc: &AbstractDisc) -> impl FnMut(I) -> Res<I, Self>
    where
        I: Span,
    {
         |i| {
             delimited(tag(disc.get_delimiters().0), tuple((Scope::parser, Abstract::abstract_parser(disc), tag("@"), Specific::parser)), tag(disc.get_delimiters().1))(i)
                 .map( |(next,(scope,r#abstract,_,specific))|{
                     ( next,
                    Self {
                        scope,
                        r#abstract,
                        specific
                    } )
                 })
         }
    }
}

/*
impl <Scope,Abstract,Specific> ExactGen<Scope,Abstract,Specific> {
    pub fn new( scope: Scope, r#abstract: Abstract, specific: Specific ) -> ExactGen<Scope,Abstract,Specific>{
        Self {scope, r#abstract, specific}
    }
}

 */

impl Abstract {
    pub fn convention(&self) -> Convention {
        /// it so happens everything is CamelCase, but that may change...
        Convention::CamelCase
    }
}

pub enum Convention {
    CamelCase,
    SkewerCase,
}

impl Convention {
    pub fn validate(&self, text: &str) -> Result<(), ParseErrs> {
        /// transform from [Result<Whatever,ParseErrs>] -> [Result<(),ParseErrs?]
        fn strip_ok<Ok, Err>(result: Result<Ok, Err>) -> Result<(), Err> {
            result.map(|_| ())
        }

        match self {
            Convention::CamelCase => strip_ok(CamelCase::from_str(text)),

            Convention::SkewerCase => strip_ok(SkewerCase::from_str(text)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct PointTypeDef<Point, Type> {
    point: Point,
    r#type: Type,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct SrcDef<Point, Kind> {
    kind: Kind,
    point: Point,
}

pub type PointKindDefSrc<Kind> = SrcDef<Point, Kind>;

pub type DataPoint = PointTypeDef<Point, Data>;

/// meaning where does this Type definition come from
/// * [DefSrc::Builtin] indicates a definition native to Starlane
/// * [DefSrc::Ext] indicates a definition extension defined outside of native Starlane
///                 potentially installed by a package
pub enum DefSrc {
    Builtin,
    Ext,
}

/// tag identifier [Tag::id] and `type`
pub struct Tag<T> {
    id: SkewerCase,
    r#type: T,
}

/// wraps a generic `segment` with a potential [Tag<T>]
pub enum TagWrap<S, T> {
    Tag(Tag<T>),
    Segment(S),
}

use crate::err::ParseErrs;
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::parse::util::{preceded, Span};
use crate::parse::{camel_case, delim_kind_lex, lex_block, unwrap_block, CamelCase, ErrCtx, NomErr, Res, SkewerCase};
use crate::point::Point;
use crate::selector::Pattern;
use crate::types::class::{Class, ClassDiscriminant};
use crate::types::parse::delim::delim;
use crate::types::parse::{class, data, identify_abstract_disc, unwrap_abstract};
use crate::types::private::{AbstractParsable, Delimited, Parsable};
use crate::types::scope::Scope;
use crate::types::specific::{Specific, SpecificSelector};
pub use data::Data;
use starlane_space::types::private::Variant;

pub(crate) mod private {
    use super::specific::Specific;
    use super::{err, Abstract, AbstractDisc, Data, Full, Generic};
    use crate::err::ParseErrs;
    use crate::parse::util::Span;
    use crate::parse::{ErrCtx, NomErr, Res};
    use crate::point::Point;
    use indexmap::IndexMap;
    use itertools::Itertools;
    use nom::bytes::complete::tag;
    use nom::combinator::into;
    use nom::error::{ErrorKind, ParseError};
    use nom::sequence::delimited;
    use nom::Parser;
    use nom_supreme::context::ContextError;
    use nom_supreme::tag::TagError;
    use std::collections::HashMap;
    use std::fmt::Display;
    use std::hash::Hash;
    use std::ops::{Deref, DerefMut};
    use std::str::FromStr;
    use strum_macros::EnumDiscriminants;
    use crate::types::class::Class;

    /// anything that can be parsed
    pub(crate) trait Parsable: Clone
    where
        Self: Sized,
    {
        fn parser<I>(input: I) -> Res<I, Self>
        where
            I: Span;
    }

    pub(crate) trait AbstractParsable
    where
        Self: Sized,
    {
        fn abstract_parser<I>(disc: &AbstractDisc) -> impl FnMut(I) -> Res<I, Self>
        where
            I: Span;
        /*
            move |input:I| {
                let err = NomErr::from_error_kind(input.clone(), ErrorKind::Fail);
                Err(nom::Err::Error(NomErr::add_context(
                    input,
                    ErrCtx::UnrecognizedTypeDelimeter(disc.get_delimiters().0),
                    err,
                )))
            }
        }

         */
    }

    pub trait Delimited: Parsable + Sized {
        fn delimiters() -> (&'static str, &'static str);

        fn delimited_parser<I, O>(f: impl FnMut(I) -> Res<I, O>) -> impl FnMut(I) -> Res<I, O>
        where
            I: Span,
        {
            delimited(tag(Self::delimiters().0), f, tag(Self::delimiters().1))
        }
    }

    /// [Variant] implies inheritance from a parent construct
    pub(crate) trait Variant {
        /// the base [Abstract] variant [Class] or [Data]
        type Root: Parsable + ?Sized;

        /// return the parent which may be another [Variant] or
        /// the base level [Abstract]
        fn parent(&self) -> Super<Self::Root>;

        fn root(&self) -> Self::Root {
            match self.parent() {
                Super::Root(root) => root,
                Super::Super(s) => s.root(),
            }
        }
    }

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

    #[derive(EnumDiscriminants, strum_macros::Display)]
    #[strum_discriminants(vis(pub))]
    #[strum_discriminants(name(SuperDisc))]
    #[strum_discriminants(derive(
        Hash,
        strum_macros::EnumString,
        strum_macros::ToString,
        strum_macros::IntoStaticStr
    ))]
    pub enum Super<A>
    where
        A: Parsable + ?Sized,
    {
        /// the `root` [Abstract] variant [Parsable] that a [Variant] derives from.
        Root(A),
        /// the `super` [Variant] of this [Variant] (which is not a `root`)
        Super(Box<dyn Variant<Root = A>>),
    }

    #[derive(EnumDiscriminants, strum_macros::Display)]
    #[strum_discriminants(vis(pub))]
    #[strum_discriminants(name(GroupDisc))]
    #[strum_discriminants(derive(
        Hash,
        strum_macros::EnumString,
        strum_macros::ToString,
        strum_macros::IntoStaticStr
    ))]
    pub enum Group {
        /// the `root` group must be an [Abstract]
        Root(Abstract),
        /// parent
        Parent(Box<dyn Member>),
    }

    /*
    impl <K> Into<K> for Scoped<K> where K: Kind {
        fn into(self) -> K {
            self.item
        }
    }

     */

    pub(crate) struct Meta<G>
    where
        G: Parsable + Into<Abstract>,
    {
        /// Type is built from `kind` and the specific of the last layer
        generic: G,
        /// types support inheritance and their
        /// multiple type definition layers that are composited.
        /// Layers define inheritance in regular order.  The last
        /// layer is the [Generic] of this [Meta] composite.
        defs: IndexMap<Specific, Layer>,
    }

    impl<K> Meta<K>
    where
        K: Parsable + Into<Abstract>,
    {
        pub fn new(kind: K, layers: IndexMap<Specific, Layer>) -> Result<Meta<K>, err::TypeErr> {
            if layers.is_empty() {
                Err(err::TypeErr::empty_meta(kind.into()))
            } else {
                Ok(Meta {
                    generic: kind,
                    defs: Default::default(),
                })
            }
        }

        pub fn to_abstract(&self) -> Abstract {
            self.generic.clone().into()
        }

        pub fn describe(&self) -> String {
            todo!()
            //            format!("Meta definitions for type '{}'", Self::name(())
        }

        pub fn generic(&self) -> &K {
            &self.generic
        }

        fn first(&self) -> &Layer {
            /// it's safe to unwrap because [Meta::new] will not accept empty defs
            self.defs.first().map(|(_, layer)| layer).unwrap()
        }

        fn layer_by_index(&self, index: usize) -> Result<&Layer, err::TypeErr> {
            self.defs
                .get_index(index)
                .ok_or(err::TypeErr::meta_layer_index_out_of_bounds(
                    &self.generic.clone().into(),
                    &index,
                    self.defs.len(),
                ))
                .map(|(_, layer)| layer)
        }

        fn layer_by_specific(&self, specific: &Specific) -> Result<&Layer, err::TypeErr> {
            self.defs
                .get(&specific)
                .ok_or(err::TypeErr::specific_not_found(
                    specific.clone(),
                    self.describe(),
                ))
        }

        pub fn specific(&self) -> &Specific {
            &self.first().specific
        }

        pub fn by_index<'x>(
            &'x self,
            index: usize,
        ) -> Result<MetaLayerAccess<'x, K>, err::TypeErr> {
            Ok(MetaLayerAccess::new(self, self.layer_by_index(index)?))
        }

        pub fn by_specific<'x>(
            &'x self,
            specific: &Specific,
        ) -> Result<MetaLayerAccess<'x, K>, err::TypeErr> {
            Ok(MetaLayerAccess::new(
                self,
                self.layer_by_specific(specific)?,
            ))
        }
    }

    pub(crate) struct MetaBuilder<T>
    where
        T: Parsable,
    {
        r#type: T,
        defs: IndexMap<Specific, Layer>,
    }

    impl<T> MetaBuilder<T>
    where
        T: Parsable + Into<Abstract>,
    {
        pub fn new(typical: T) -> MetaBuilder<T> {
            Self {
                r#type: typical,
                defs: Default::default(),
            }
        }

        pub fn build(self) -> Result<Meta<T>, err::TypeErr> {
            todo!();
            //            Meta::new(self.r#type.into(), self.defs)
        }
    }
    impl<T> Deref for MetaBuilder<T>
    where
        T: Parsable,
    {
        type Target = IndexMap<Specific, Layer>;

        fn deref(&self) -> &Self::Target {
            &self.defs
        }
    }

    impl<T> DerefMut for MetaBuilder<T>
    where
        T: Parsable + Into<Abstract>,
    {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.defs
        }
    }

    pub(crate) struct MetaLayerAccess<'y, K>
    where
        K: Parsable + Into<Abstract>,
    {
        meta: &'y Meta<K>,
        layer: &'y Layer,
    }

    impl<'y, K> MetaLayerAccess<'y, K>
    where
        K: Parsable + Into<Abstract>,
    {
        fn new(meta: &'y Meta<K>, layer: &'y Layer) -> MetaLayerAccess<'y, K> {
            Self { meta, layer }
        }

        pub fn get_type(&'y self) -> Abstract {
            self.meta.to_abstract()
        }

        pub fn meta(&'y self) -> &'y Meta<K> {
            self.meta
        }

        pub fn specific(&'y self) -> &'y Specific {
            self.meta.specific()
        }

        pub fn layer(&'y self) -> &'y Layer {
            self.layer
        }
    }

    #[derive(Clone)]
    pub(crate) struct Layer {
        specific: Specific,
        classes: HashMap<Class, ClassPointRef>,
        data: HashMap<Data, SchemaPointRef>,
    }

    pub type ClassPointRef = Ref<Point, Class>;
    pub type SchemaPointRef = Ref<Point, Data>;
    pub type ParselPointRef<G: Parsable> = Ref<Point, G>;
    pub type ExactPointRef = Ref<Point, Full>;

    #[derive(Clone, Eq, PartialEq, Hash)]
    pub struct Ref<I, K>
    where
        I: Clone + Eq + PartialEq + Hash,
        K: Clone + Eq + PartialEq + Hash,
    {
        id: I,
        r#type: K,
    }

    impl<Scope, Abstract, Specific> Generic<Scope, Abstract, Specific>
    where
        Scope: Parsable + Default,
        Abstract: Delimited,
        Specific: Parsable,
    {
        pub fn of(r#abstract: Abstract, specific: Specific) -> Self {
            Self::new(Scope::default(), r#abstract, specific)
        }
    }

    impl<Scope, Abstract, Specific> Generic<Scope, Abstract, Specific>
    where
        Scope: Parsable + Default,
        Abstract: Delimited,
        Specific: Parsable,
    {
        pub fn new(scope: Scope, r#abstract: Abstract, specific: Specific) -> Self {
            Self {
                scope,
                r#abstract,
                specific,
            }
        }

        pub fn plus_scope(self, scope: Scope) -> Self {
            Self::new(scope, self.r#abstract, self.specific)
        }

        pub fn plus_specific(self, specific: Specific) -> Self {
            Self::new(self.scope, self.r#abstract, specific)
        }

        pub fn r#abstract(&self) -> &Abstract {
            &self.r#abstract
        }
        pub fn specific(&self) -> &Specific {
            &self.specific
        }
    }
}

#[cfg(test)]
pub mod test2 {
    use crate::parse::util::new_span;
    use crate::types::private::Parsable;
    use crate::types::scope::Scope;
    use crate::types::specific::Specific;
    use crate::types::Full;

    #[test]
    pub fn test_full() {
        use super::{Delimited, Parsable};
        use crate::types::{err, Abstract, AbstractDisc, Data, Full, Generic};
        let input = new_span("<Root@contrib:package:1.0.0>");
    }
}
