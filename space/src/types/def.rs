use crate::kind::Specific;
use crate::types::specific::SpecificLoc;
use crate::types::{err, Absolute, Type};
use derive_builder::Builder;
use getset::Getters;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::fmt::Display;
use serde::{Deserialize, Serialize};
use crate::parse::SkewerCase;
use crate::particle::property::PropertyDef;

/// [Defs] for 
#[derive(Clone,Getters,Builder)]
pub struct Defs
{
    r#absolute: Absolute,
    /// types support inheritance and their
    /// multiple type definition layers that are composited.
    /// [Layer]s define inheritance in regular order.  The last
    /// layer is the [Type]  of this [Defs] composite.
    #[getset(skip)]
    layers: IndexMap<Absolute, Layer>,
}

impl Defs
{
    pub fn new(r#absolute: Absolute, layers: IndexMap<SpecificLoc, Layer>) -> Result<Defs, err::TypeErr> {
        if layers.is_empty() {
            Err(err::TypeErr::empty_meta(r#absolute.r#type))
        } else {
            Ok(Defs {
                r#absolute,
                layers: Default::default(),
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
        /// it's safe to unwrap because [Defs::new] will not accept empty defs
        self.layers.first().map(|(_, layer)| layer).unwrap()
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

    fn layer_by_absolute(&self, loc: &Absolute) -> Result<&Layer, err::TypeErr> {
        self.layers
            .get(loc)
            .ok_or(err::TypeErr::absolute_not_found(
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

    pub fn by_absolute(
        &self,
        absolute: &Absolute,
    ) -> Result<MetaLayerAccess, err::TypeErr> {
        Ok(MetaLayerAccess::new(
            self,
            self.layer_by_absolute(absolute)?,
        ))
    }
}




pub(crate) struct MetaLayerAccess<'y>
{
    meta: &'y Defs,
    layer: &'y Layer,
}

impl<'y> MetaLayerAccess<'y>
{
    fn new(meta: &'y Defs, layer: &'y Layer) -> MetaLayerAccess<'y> {
        Self { meta, layer }
    }

    pub fn get_type(&'y self) -> &'y Type {
        self.meta.to_type()
    }

    pub fn meta(&'y self) -> &'y Defs {
        self.meta
    }


    pub fn layer(&'y self) -> &'y Layer {
        self.layer
    }
}

#[derive(Clone,Builder,Getters)]
pub struct Layer {
    specific: SpecificLoc,
    properties: HashMap<SkewerCase,PropertyDef>,
}












