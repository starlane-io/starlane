use crate::kind::Specific;
use crate::types::specific::SpecificLoc;
use crate::types::{err, Absolute, Type};
use derive_builder::Builder;
use getset::Getters;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::fmt::Display;
use serde::{Deserialize, Serialize};

#[derive(Clone,Getters,Builder,Serialize,Deserialize)]
pub struct Meta
{
    r#absolute: Absolute,
    /// types support inheritance and their
    /// multiple type definition layers that are composited.
    /// [Layer]s define inheritance in regular order.  The last
    /// layer is the [Type]  of this [Meta] composite.
    #[getset(skip)]
    defs: IndexMap<SpecificLoc, Layer>,
}

impl Meta
{
    pub fn new(r#absolute: Absolute, layers: IndexMap<SpecificLoc, Layer>) -> Result<Meta, err::TypeErr> {
        if layers.is_empty() {
            Err(err::TypeErr::empty_meta(r#absolute.r#type))
        } else {
            Ok(Meta {
                r#absolute,
                defs: Default::default(),
            })
        }
    }

    pub fn to_type(&self) -> & Type {
        & self.r#absolute.r#type
    }

    pub fn describe(&self) -> String {
        todo!()
        //            format!("Meta definitions for type '{}'", Self::name(())
    }

    pub fn r#type(&self) -> &Type {
        &self.absolute.r#type
    }

    fn first(&self) -> &Layer {
        /// it's safe to unwrap because [Meta::new] will not accept empty defs
        self.defs.first().map(|(_, layer)| layer).unwrap()
    }

    fn layer_by_index(&self, index: usize) -> Result<&Layer, err::TypeErr> {
/*        self.defs
            .get_index(index)
            .ok_or(err::TypeErr::meta_layer_index_out_of_bounds(
                &self.r#absolute.clone().into(),
                &index,
                self.defs.len(),
            ))
            .map(|(_, layer)| layer)
            
 */
        todo!()
    }

    fn layer_by_specific(&self, loc: &SpecificLoc) -> Result<&Layer, err::TypeErr> {
        self.defs
            .get(loc)
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
        specific: &SpecificLoc,
    ) -> Result<MetaLayerAccess, err::TypeErr> {
        Ok(MetaLayerAccess::new(
            self,
            self.layer_by_specific(specific)?,
        ))
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

    pub fn get_type(&'y self) -> &'y Type {
        self.meta.to_type()
    }

    pub fn meta(&'y self) -> &'y Meta {
        self.meta
    }


    pub fn layer(&'y self) -> &'y Layer {
        self.layer
    }
}

#[derive(Clone,Builder,Getters,Serialize,Deserialize)]
pub struct Layer {
    specific: Specific,
    types: HashMap<Type, Meta>,
}












