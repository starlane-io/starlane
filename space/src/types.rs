#![cfg(feature="types2")]

use strum_macros::EnumDiscriminants;
use thiserror::__private::AsDisplay;

mod class;
mod schema;

pub mod registry;
pub mod specific;
pub mod err;

/// meaning where does this Type definition come from
pub enum DefSrc {
    Builtin,
    Ext,
}

pub(crate) mod private {
    use std::borrow::Borrow;
    use std::marker::PhantomData;
    use std::str::FromStr;
    use tracing::Instrument;
    use crate::parse::CamelCase;
    use crate::kind::Specific;
    use crate::point::Point;
    use crate::types::{err, Type};
    use crate::types::class::Class;
    use super::TypeCategory;

    pub(crate) trait Kind: Clone+FromStr {

        type Type;

        fn category(&self) -> TypeCategory;

        fn type_kind(&self) -> super::TypeKind;

        fn plus_specific(self, specific: Specific) -> SpecificKind<Self> {
            SpecificKind::new(self,specific)
        }

        fn factory() -> impl Fn(SpecificKind<Self>) -> Type;

    }

    pub(crate) trait Typical: Into<Type> { }





    /// a `Variant` is a unique `Type` in a within a `Category`
    /// `Data` & `Class` categories and their enum variants ... i.e. [Data::Raw],[ClassVariant::_Ext]
    /// are the actual `Type` `Variants`
    /// Variants are always CamelCase
    pub(crate) trait Variant: Kind + Clone + ToString + From<CamelCase> + Into<CamelCase> { }

    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    pub(crate) struct KindVariantDef<T>
    where
        T: Kind + Into<super::TypeCategory>,
    {
        specific: Specific,
        kind: T,
    }

    /// [MetaDef] stores structured references to the [SpecificKind]'s definition elements.
    /// [Specifics] support hierarchical inheritance which is why [MetaDef] is composed of
    /// a vector of [LayerDef]s.  [MetaDef] composites the parental layers to provide a singular
    /// view of a [SpecificKind]'s [MetaDef] defs.
    ///
    #[derive(Clone)]
    pub(crate) struct MetaDef<K> where K: Kind {
        kind: K,
        layers: Vec<Layer>
    }

    impl <K> MetaDef<K> where K: Kind{
        pub fn new(kind: K, layers: Vec<Layer>) -> Result<Self,err::TypeErr> {
            if layers.is_empty() {
                Err(err::TypeErr::empty_meta(kind.type_kind()))
            } else {
                Ok(Self {
                    kind,
                    layers
                })
            }
        }

        pub fn specific(&self) -> & Specific {
            & self.layers.first().unwrap().specific
        }

        pub fn as_type(&self) -> K::Type {
            let specific = self.layers.first().unwrap().specific.clone();
            self.kind.clone().plus_specific(specific)
        }
    }


    pub(crate) struct Child<'y,T> where T: Kind{
        meta: &'y MetaDef<T>,
        layer: &'y Layer,
    }

    impl <'y,T> Child<'y,T> where T: Kind{
        pub fn new(meta: &'y MetaDef<T>, layer: &'y T ) -> Child<'y,T> {
            Self {
                meta,
                layer
            }
        }

        pub fn get_type(&'y self) -> SpecificKind<T> {
            self.meta.as_type()
        }

        pub fn meta(&'y self) -> &'y MetaDef<T>  {
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
        specific: Specific
    }


    pub struct Ref {
        point: Point,
    }






    #[derive(Clone, Debug, Eq, PartialEq, Hash, ,Serialize,Deserialize)]
    pub(crate) struct SpecificKind<K> where K: Kind{
        specific: Specific,
        kind: K,
    }

    impl <K> Typical for SpecificKind<K> where K: Kind{
    }


    impl <K> Into<Type> for SpecificKind<K> where K: Kind
    {
        fn into(self) -> Type {
            K::factory()(self)
        }
    }



    impl <K> SpecificKind<K> where K: Kind
    {
        pub fn new(kind: K, specific: Specific ) -> Self {
            Self {
                kind,
                specific
            }
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




pub use schema::SchemaKind;
use starlane_space::kind::Specific;
use crate::point::Point;
use crate::types::class::{Class, ClassKind};
use crate::types::private::Kind;
use crate::types::schema::Schema;




