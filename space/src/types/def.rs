use crate::types::specific::SpecificLoc;
use crate::types::{err, Absolute, Type};
use derive_builder::Builder;
use getset::Getters;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::fmt::Display;
use crate::parse::{SkewerCase, SnakeCase};
use crate::particle::property::{PropertiesConfigBuilder, PropertyDef};
use crate::types::err::TypeErr;

/// [Defs] for an [Absolute]
#[derive(Clone,Getters,Builder)]
pub struct Defs
{
    r#specific: SpecificLoc,
    /// types support inheritance and their
    /// multiple type definition layers that are composited.
    /// [Layer]s define inheritance in regular order.  The last
    /// layer is the [Type]  of this [Defs] composite.
    #[getset(skip)]
    layers: IndexMap<Type,Vec<Layer>>,
}

impl Defs
{
    pub fn new(specific: SpecificLoc) -> Result<Defs, err::TypeErr> {

            Ok(Self {
                specific,
                layers: IndexMap::default(),
            })
    }
    
    pub fn add_layer(& mut self, r#type: Type, layer: Layer) {
        match self.layers.get_mut(&r#type) {
            None => {
                self.layers.insert(r#type, vec![layer]);
            }
            Some(layers) => layers.push(layer),
        }
    }
    
    pub fn create_layer_composite(&self) -> Result<CompositeBuilder,TypeErr> {
        let mut rtn = CompositeBuilder::of(self.specific.clone());
        
        for (r#type, layers) in &self.layers {
            for layer in layers {
                for change in &layer.changes {
                    let absolute = Absolute::new(Default::default(),r#type.clone(),self.specific.clone());

                    let mut ty_comp= match rtn.types.get_mut(&change.r#type) {
                        None => {
                            let ty_comp = TypeCompositeBuilder::of(absolute);
                            rtn.types.insert(change.r#type.clone(), ty_comp);
                            rtn.types.get_mut(&change.r#type).unwrap()
                        }
                        Some(ty_comp) => ty_comp
                    };
                    
                    match &change.action {
                        Action::Add(add) => {
                            match add {
                                Add::Property(prop) => { ty_comp.properties.push(prop.clone()); }
                            }
                        }
                        Action::Remove(remove) => {
                            match remove {
                                Remove::Type => {
                                    rtn.types.remove(&change.r#type);
                                },
                                Remove::Property(name) => {
                                    ty_comp.properties.remove(name);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(rtn)
    }



    pub fn describe(&self) -> String {
        todo!()
        //            format!("Meta definitions for type '{}'", Self::name(())
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



}




#[derive(Clone,Builder,Getters)]
pub struct Layer {
    specific: SpecificLoc,
    changes: Vec<Change>,
}

/// each [Layer] can modify the defs of it's inherited [Layer]...
/// including the ability to remove [PropertyDef] ... etc


#[derive(Clone)]
pub struct Change {
    r#type: Type,
    action: Action,
}

#[derive(Clone)]
pub enum Action{
    Add(Add),
    Remove(Remove),
}

/// no need for [Add::Type] since it will happen automatically when any element
/// of its composite is added.  
#[derive(Clone)]
pub enum Add {
    Property(PropertyDef),
}

#[derive(Clone)]
pub enum Remove{
    /// remove an entire [Type] from the composite
    Type,
    Property(SnakeCase),
}

pub struct TypeCompositeBuilder {
    absolute: Absolute,
    properties: PropertiesConfigBuilder
}

impl TypeCompositeBuilder {
    pub fn of( absolute: Absolute) -> Self {
        Self {
            properties: PropertiesConfigBuilder::new(absolute.clone()),
            absolute,
        }
    }
}

pub struct CompositeBuilder {
    specific: SpecificLoc,
    types: HashMap<Type, TypeCompositeBuilder>,
}

impl CompositeBuilder {
   pub fn of(specific: SpecificLoc) -> Self {
       Self {
           specific,
           types: Default::default()
       }
   } 
}
















