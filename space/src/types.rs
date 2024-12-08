#![cfg(feature="types2")]

use strum_macros::EnumDiscriminants;
use thiserror::__private::AsDisplay;

mod class;
mod schema;

mod domian;

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
    use std::collections::HashMap;
    use std::fmt::Display;
    use std::marker::PhantomData;
    use std::ops::{Deref, DerefMut, Index};
    use std::str::FromStr;
    use indexmap::IndexMap;
    use itertools::Itertools;
    use rustls::pki_types::Der;
    use tracing::Instrument;
    use crate::parse::{some, CamelCase};
    use crate::kind::Specific;
    use crate::log::Level::Debug;
    use crate::point::Point;
    use crate::types::{err, SchemaKind, Type, TypeKind};
    use crate::types::class::{Class, ClassKind};
    use super::TypeCategory;

    pub(crate) trait Kind: Clone+Into<TypeKind>{

        type Type;

        fn category(&self) -> TypeCategory;

        fn plus_specific(self, specific: impl ToOwned<Owned=Specific>) -> Exact<Self> {
            Exact::new(self, specific)
        }

        fn factory() -> impl Fn(Exact<Self>) -> Type;

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


   /// check if Ref follows constarints4r

    #[derive(Clone)]
    pub struct Ref<K> where K: Kind  {
        kind: K,
        point: Point,
    }



    #[derive(Clone, Debug, Eq, PartialEq, Hash, ,Serialize,Deserialize)]
    pub(crate) struct Exact<K> where K: Kind{
        scope: DomainScope,
        kind: K,
        specific: Specific,
    }

    impl <K> Typical for Exact<K> where K: Kind{
    }


    impl <K> Into<Type> for Exact<K> where K: Kind
    {
        fn into(self) -> Type {
            K::factory()(self)
        }
    }



    impl <K> Exact<K> where K: Kind
    {
        pub fn new(kind: impl ToOwned<Owned=K>, specific: impl ToOwned<Owned=Specific> ) -> Self {
            let kind = kind.to_owned();
            let specific = specific.to_owned();
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




