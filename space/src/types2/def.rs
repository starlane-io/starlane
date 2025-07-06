use crate::parse::SnakeCase;
use crate::types::err::TypeErr;
use crate::types::property::{PropertiesConfig, PropertiesConfigBuilder, PropertyDef};
use crate::types::specific::SpecificLoc;
use crate::types::{err, Absolute, Type};
use getset::Getters;
use std::collections::HashMap;
use std::fmt::Display;

/// [Defs] for an [Absolute]
#[derive(Clone, Getters)]
pub struct Defs {
    specific: SpecificLoc,

    /// [Self::specific] must be in
    #[getset(skip)]
    layers: HashMap<SpecificLoc, Layer>,
}

impl Defs {
    pub fn new(specific: SpecificLoc) -> Defs {
        Self {
            specific,
            layers: Default::default(),
        }
    }

    pub fn push_layer(&mut self, r#type: Type, layer: Layer) {
        todo!()
        /*
        match self.layers.get_mut(&r#type) {
            None => {
                self.layers.insert(r#type, vec![layer]);
            }
            Some(layers) => layers.push(layer),
        }

         */
    }

    pub fn create_layer_composite(&self) -> Result<SpecificCompositeBuilder, TypeErr> {
        todo!()
        /*
                let mut rtn = SpecificCompositeBuilder::of(self.specific.clone());

                for (r#type, layer) in &self.layers {
                        for change in &layer.changes {
                            let absolute =
                                Absolute::new(Default::default(), r#type.clone(), self.specific.clone());

                            let mut ty_comp = match rtn.types.get_mut(&change.r#type) {
                                None => {
                                    let ty_comp = TypeCompositeBuilder::of(absolute);
                                    rtn.types.insert(change.r#type.clone(), ty_comp);
                                    rtn.types.get_mut(&change.r#type).unwrap()
                                }
                                Some(ty_comp) => ty_comp,
                            };

                            match &change.action {
                                Action::Add(add) => match add {
                                    Add::Property(prop) => {
                                        ty_comp.properties.push(prop.clone());
                                    }
                                },
                                Action::Remove(remove) => match remove {
                                    Remove::Type => {
                                        rtn.types.remove(&change.r#type);
                                    }
                                    Remove::Property(name) => {
        todo!()
        //                                ty_comp.properties.remove(name);
                                    }
                                },
                            }
                        }

                }

                Ok(rtn)
                */
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

#[derive(Clone, Getters)]
pub struct Layer {
    parent: Option<SpecificLoc>,
    specific: SpecificLoc,
    changes: Vec<Change>,
}

#[derive(Clone)]
pub struct LayerBuilder {
    parent: Option<Layer>,
    specific: SpecificLoc,
    changes: Vec<Change>,
}
impl LayerBuilder {
    pub fn new(specific: SpecificLoc) -> LayerBuilder {
        Self {
            specific,
            changes: Default::default(),
            parent: Default::default(),
        }
    }

    pub fn set_parent(&mut self, parent: Layer) {
        self.parent = Some(parent);
    }

    pub fn add_change(&mut self, change: Change) {
        self.changes.push(change);
    }

    pub fn build(self) -> Layer {
        let parent = self.parent.map(Box::new);
        /*
        Layer {
            parent,
            changes: self.changes,
            specific: self.specific
        }

         */
        todo!()
    }
}

/// each [Layer] can modify the defs of it's inherited [Layer]...
/// including the ability to remove [PropertyDef] ... etc

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Change {
    r#type: Type,
    action: Action,
}

impl Change {
    pub fn new(r#type: Type, action: impl Into<Action>) -> Self {
        let action = action.into();
        Self { r#type, action }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Add(Add),
    Remove(Remove),
}

/// no need for [Add::Type] since it will happen automatically when any element
/// of its composite is added.  
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Add {
    Property(PropertyDef),
}

impl Into<Action> for Add {
    fn into(self) -> Action {
        Action::Add(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Remove {
    /// remove an entire [Type] from the composite
    Type,
    Property(SnakeCase),
}

impl Into<Action> for Remove {
    fn into(self) -> Action {
        Action::Remove(self)
    }
}

#[derive(Clone, Getters)]
#[getset(get = "pub")]
pub struct TypeComposite {
    absolute: Absolute,
    properties: PropertiesConfig,
}

pub struct TypeCompositeBuilder {
    absolute: Absolute,
    properties: PropertiesConfigBuilder,
}

impl TypeCompositeBuilder {
    pub fn of(absolute: Absolute) -> Self {
        Self {
            properties: PropertiesConfigBuilder::new(absolute.clone()),
            absolute,
        }
    }

    pub fn build(self) -> TypeComposite {
        TypeComposite {
            absolute: self.absolute,
            properties: self.properties.build(),
        }
    }

    pub fn add_property(&mut self, prop: PropertyDef) {
        self.properties.push(prop);
    }

    pub fn remove_property(&mut self, key: &SnakeCase) {
        self.properties.remove(key);
    }
}

#[derive(Clone, Getters)]
pub struct SpecificComposite {
    pub specific: SpecificLoc,
    pub types: HashMap<Type, TypeComposite>,
}

pub struct SpecificCompositeBuilder {
    specific: SpecificLoc,
    types: HashMap<Type, TypeCompositeBuilder>,
}

impl SpecificCompositeBuilder {
    pub fn of(specific: SpecificLoc) -> Self {
        Self {
            specific,
            types: Default::default(),
        }
    }

    pub fn build(self) -> SpecificComposite {
        let types = self
            .types
            .into_iter()
            .map(|(ty, builder)| (ty, builder.build()))
            .collect();
        SpecificComposite {
            specific: self.specific,
            types,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::types::class::Class;
    use crate::types::def::{Add, Change, Defs, LayerBuilder, TypeCompositeBuilder};
    use crate::types::property::PropertyDef;
    use crate::types::specific::SpecificLoc;
    use crate::types::{Absolute, Type};
    use itertools::Itertools;

    #[test]
    pub fn type_builder() {
        let absolute = Absolute::mock_default();
        let less = PropertyDef::mock_less();
        let mut builder = TypeCompositeBuilder::of(absolute.clone());

        assert_eq!(absolute, builder.absolute);
        builder.add_property(less.clone());
        let comp = builder.build();
        assert_eq!(absolute, comp.absolute);
        assert_eq!(1, comp.properties.len());
        let property = comp.properties.get(less.name()).unwrap();
        assert_eq!(less, *property);
    }

    /*
    #[test]
    pub fn layer() {
        let mut defs= Defs::new(SpecificLoc::mock_default());
        let less= PropertyDef::mock_less();
        let fae = PropertyDef::mock_fae();
        let layer = LayerBuilder::new(SpecificLoc::mock_default());
        //defs.push_layer()
        todo!()
    }

     */

    /*
    #[test]
    pub fn multi_layer() {
        let specific = SpecificLoc::mock_default();
        let specific0 = SpecificLoc::mock_0();
        let specific1 = SpecificLoc::mock_1();
        let mut builder = Defs::new(specific);

        let abs1 = Absolute::mock_default();
        let abs2 = Absolute::mock_root();

        let less= PropertyDef::mock_less();
        let fae = PropertyDef::mock_fae();
        let modus = PropertyDef::mock_modus();

        let add_less = Change::new(Type::Class(Class::Root), Add::Property(less.clone()));
        let add_fae = Change::new(Type::Class(Class::User), Add::Property(fae.clone()));
        let add_modus = Change::new(Type::Class(Class::User), Add::Property(modus.clone()));

        builder.add_change(add_less.clone());
        builder.add_change(add_fae.clone());

        let layer = builder.clone().build();
        assert_eq!(specific, layer.specific);
        assert_eq!(2, layer.changes.len());
        let first = layer.changes.first().cloned();
        assert_eq!(Some(add_less), layer.changes.first().cloned());
    }

     */
}
