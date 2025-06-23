use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::EnumDiscriminants;
use indexmap::IndexMap;
use std::ops::{Deref, DerefMut, Index};
use std::fmt::{write, Display, Formatter};
use derive_builder::Builder;
use crate::parse::util::Span;
use crate::types::{err, ClassPointRef, Type, SchemaPointRef, TypeLocation, Absolute};
use crate::types::class::{Class, ClassDef};
use crate::types::data::{Data, DataDef};
use crate::types::archetype::Archetype;
use crate::types::scope::{Scope, Segment};
use crate::types::specific::SpecificLoc;



pub type Defs = HashMap<SliceLoc,Type>;

#[derive()]
pub(crate) struct Meta
{
    r#type: Type,
    /// types support inheritance and their
    /// multiple type definition layers that are composited.
    /// [Layer]s define inheritance in regular order.  The last
    /// layer is the [Type]  of this [Meta] composite.
    defs: IndexMap<SpecificSliceLoc, Layer>,
}

impl Meta
{
    pub fn new(r#type: Type, layers: IndexMap<SpecificLoc, Layer>) -> Result<Meta, err::TypeErr> {
        if layers.is_empty() {
            Err(err::TypeErr::empty_meta(r#type.into()))
        } else {
            Ok(Meta {
                r#type,
                defs: Default::default(),
            })
        }
    }

    pub fn to_type(&self) -> Type {
        self.r#type.clone().into()
    }

    pub fn describe(&self) -> String {
        todo!()
        //            format!("Meta definitions for type '{}'", Self::name(())
    }

    pub fn r#type(&self) -> &Type {
        &self.r#type
    }

    fn first(&self) -> &Layer {
        /// it's safe to unwrap because [Meta::new] will not accept empty defs
        self.defs.first().map(|(_, layer)| layer).unwrap()
    }

    fn layer_by_index(&self, index: usize) -> Result<&Layer, err::TypeErr> {
        self.defs
            .get_index(index)
            .ok_or(err::TypeErr::meta_layer_index_out_of_bounds(
                &self.r#type.clone().into(),
                &index,
                self.defs.len(),
            ))
            .map(|(_, layer)| layer)
    }

    fn layer_by_specific(&self, loc: &SpecificSliceLoc) -> Result<&Layer, err::TypeErr> {
        self.defs
            .get(&loc)
            .ok_or(err::TypeErr::specific_not_found(
                loc.clone(),
                self.describe(),
            ))
    }

    /*
    pub fn specific(&self) -> &SpecificLoc {
        &self.first().specific
    }
    
     */

    pub fn by_index(
        &self,
        index: usize,
    ) -> Result<MetaLayerAccess, err::TypeErr> {
        Ok(MetaLayerAccess::new(self, self.layer_by_index(index)?))
    }

    pub fn by_specific(
        &self,
        specific: &SpecificSliceLoc,
    ) -> Result<MetaLayerAccess, err::TypeErr> {
        Ok(MetaLayerAccess::new(
            self,
            self.layer_by_specific(specific)?,
        ))
    }
}

pub(crate) struct MetaBuilder<T>
where
    T: Archetype,
{
    r#type: T,
    defs: IndexMap<SpecificLoc, Layer>,
}

impl<T> MetaBuilder<T>
where
    T: Archetype + Into<Type>,
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
    T: Archetype,
{
    type Target = IndexMap<SpecificLoc, Layer>;

    fn deref(&self) -> &Self::Target {
        &self.defs
    }
}

impl<T> DerefMut for MetaBuilder<T>
where
    T: Archetype + Into<Type>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.defs
    }
}

pub(crate) struct MetaLayerAccess<'y>
{
    meta: &'y Meta,
    layer: &'y Layer,
}

impl<'y> MetaLayerAccess<'y>
{
    fn new(meta: &'y Meta, layer: &'y Layer) -> MetaLayerAccess<'y> {
        Self { meta, layer }
    }

    pub fn get_type(&'y self) -> Type {
        self.meta.to_type()
    }

    pub fn meta(&'y self) -> &'y Meta<K> {
        self.meta
    }

    pub fn specific(&'y self) -> &'y SpecificLoc {
        self.meta.specific()
    }

    pub fn layer(&'y self) -> &'y Layer {
        self.layer
    }
}

#[derive(Clone,Builder)]
pub(crate) struct Layer {
    id: SpecificSliceLoc,
    types: HashMap<Type, Absolute>,
}


#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash,Builder)]
pub struct SpecificSliceLoc {
   specific: SpecificLoc, 
    
   /// the hierarchy of [SliceLoc]s in `reverse` order
   ancestors: Vec<SliceLoc>
}


impl Display for SpecificSliceLoc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,"{}",self.specific)?;
        /// iterate in `reverse` since `SpecificSliceLoc::ancestors` 
        for (index,slice) in self.ancestors.iter().rev().enumerate() {
            write!(f, "{}", slice)?;
            if index != self.ancestors.len() - 1 {
                write!(f, ":")?;
            }
        }
        Ok(())
    }
}



#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash,Builder)]
pub struct Package {
  pub specific: SpecificLoc,
  pub title: String,
  pub slices: Vec<SliceLoc>,
}


#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Hash,derive_builder::Builder)]
pub struct SliceLoc {
  loc: Segment,
  children: Vec<Box<SliceLoc>>,
}

impl Display for SliceLoc {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f,"{}",self.loc)?;
        for (index,slice) in self.children.iter().enumerate() {
            write!(f, "{}", slice)?;
            if index != self.children.len() - 1 {
                write!(f, ":")?;
            }
        }
        Ok(())
    }
}
