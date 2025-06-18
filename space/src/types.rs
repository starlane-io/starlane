use derive_name::Name;
use nom::bytes::complete::tag;
use nom::sequence::{delimited, tuple, Tuple};
use std::fmt::Display;
use std::str::FromStr;
use nom::branch::alt;
use nom::combinator::{into, not, value};
use nom::multi::many1;
use strum_macros::EnumDiscriminants;

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

pub type FullAbstract<Abstract: Parsable> = FullGeneric<Scope, Abstract, Specific>;

pub type Full = FullGeneric<Scope, Abstract, Specific>;
pub type FullSelector = FullGeneric<Pattern<Scope>, Pattern<Abstract>, SpecificSelector>;

impl<Scope, Abstract, Specific> Parsable for FullGeneric<Scope, Abstract, Specific>
where
    Scope: Parsable + Default,
    Abstract: Delimited,
    Specific: Parsable,
{
    fn parser<I>(input: I) -> Res<I, Self>
    where
        I: Span,
    {
        tuple((Scope::parser, Abstract::parser, tag("@"), Specific::parser))(input)
            .map(|(next, (scope, abs, _, specific))| (next, Self::new(scope, abs, specific)))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct FullGeneric<Scope, Abstract, Specific> {
    scope: Scope,
    r#abstract: Abstract,
    specific: Specific,
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
use crate::parse::util::Span;
use crate::parse::{delim_kind_lex, lex_block, unwrap_block, CamelCase, Res, SkewerCase};
use crate::point::Point;
use crate::selector::Pattern;
use crate::types::class::{Class, ClassDiscriminant};
use crate::types::private::{AbstractParsable, Delimited, Parsable};
use crate::types::scope::Scope;
use crate::types::specific::SpecificSelector;
pub use data::Data;
use specific::Specific;
use starlane_space::types::private::Variant;
use crate::parse::model::{BlockKind, NestedBlockKind};
use crate::types::parse::delim::delim;

pub(crate) mod private {
    use super::specific::Specific;
    use super::{err, Abstract, AbstractDisc, Data, Full, FullGeneric};
    use crate::parse::util::Span;
    use crate::parse::Res;
    use crate::point::Point;
    use crate::types::class::Class;
    use indexmap::IndexMap;
    use itertools::Itertools;
    use nom::Parser;
    use std::collections::HashMap;
    use std::fmt::Display;
    use std::hash::Hash;
    use std::ops::{Deref, DerefMut};
    use std::str::FromStr;
    use nom::bytes::complete::tag;
    use nom::combinator::into;
    use nom::sequence::delimited;
    use strum_macros::EnumDiscriminants;

    /// anything that can be parsed
    pub(crate) trait Parsable: Clone
    where
        Self: Sized,
    {
        fn parser<I>(input: I) -> Res<I, Self>
        where
            I: Span;
    }
    
    
    pub(crate) trait AbstractParsable: Clone
    where
        Self: Sized,
    {
        fn abstract_parser<I,F,O>(disc: AbstractDisc) -> impl FnMut(I) -> Res<I, O>
        where F: FnMut(I) -> Res<I,O>+Copy, I: Span;
    }


    pub trait Delimited: Parsable + Sized {
        fn delimiters() -> (&'static str, &'static str);
        
        fn delimited_parser<I,O>(f: impl FnMut(I) -> Res<I,O>) -> impl FnMut(I) -> Res<I, O>
        where I: Span {
            delimited( tag(Self::delimiters().0),f,tag(Self::delimiters().1))
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
        /// layer is the [FullGeneric] of this [Meta] composite.
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
        schema: HashMap<Data, SchemaPointRef>,
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

    impl<Scope, Abstract, Specific> FullGeneric<Scope, Abstract, Specific>
    where
        Scope: Default,
    {
        pub fn of(r#abstract: Abstract, specific: Specific) -> Self {
            Self::new(Scope::default(), r#abstract, specific)
        }
    }

    impl<Scope, Abstract, Specific> FullGeneric<Scope, Abstract, Specific> {
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
