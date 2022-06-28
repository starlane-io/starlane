use crate::{SkewerCase, Resource, ResourceIdentifier, ResourceSelector, ResourceStub, FieldSelection, ResourcePath, ConfigSrc};
use std::str::FromStr;
use crate::error::Error;
use crate::data::{DataSet, BinSrc, Meta};
use serde::{Serialize,Deserialize};
use std::collections::HashMap;
use crate::parse::{parse_resource_property_value_selector, parse_resource_value_selector, parse_resource_property_assignment};
use crate::status::Status;
use std::convert::TryInto;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceValueSelector {
    pub resource: ResourcePath,
    pub property: ResourcePropertyValueSelector,
}

impl ResourceValueSelector {
    pub fn new( resource: ResourcePath, property: ResourcePropertyValueSelector ) -> Self {
        Self{
            resource,
            property
        }
    }
}

impl FromStr for ResourceValueSelector {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover, selector ) = parse_resource_value_selector(s)?;

        if !leftover.is_empty() {
            return Err(format!("could not parse ResourceValueSelector: '{}' trailing portion '{}'", s, leftover).into() );
        } else {
            return Ok(selector?);
        }
    }
}



#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum DataSetAspectSelector {
    All,
    Exact(String)
}

impl DataSetAspectSelector {
    pub fn filter( &self, set: DataSet<BinSrc> ) -> ResourceValue {
        match self {
            DataSetAspectSelector::Exact(aspect) => {
                let mut rtn = DataSet::new();
                if set.contains_key(aspect) {
                    rtn.insert( aspect.clone(), set.get(aspect).expect(format!("expected aspect: {}", aspect).as_str() ).clone());
                }
                ResourceValue::DataSet(rtn)
            }
            DataSetAspectSelector::All => {
                ResourceValue::DataSet(set)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePropertyOp<P> {
    pub resource: ResourceIdentifier,
    pub property: P
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, strum_macros::Display)]
pub enum ResourcePropertyValueSelector {
    Registry(ResourceRegistryPropertyValueSelector),
    Host(ResourceHostPropertyValueSelector),
}

impl ResourcePropertyValueSelector {

    pub fn is_registry(&self) -> bool {
        match self {
            ResourcePropertyValueSelector::Registry(_)=> true,
            _ => false
        }
    }

    pub fn state() -> Self {
        Self::Host(ResourceHostPropertyValueSelector::State {
            aspect: DataSetAspectSelector::All,
            field: FieldValueSelector::All
        })
    }

    pub fn state_aspect(aspect: &str) -> Self {
        Self::Host(ResourceHostPropertyValueSelector::State {
            aspect: DataSetAspectSelector::Exact(aspect.to_string()),
            field: FieldValueSelector::All
        })
    }

    pub fn state_aspect_field(aspect: &str, field: &str) -> Self {
        Self::Host(ResourceHostPropertyValueSelector::State {
            aspect: DataSetAspectSelector::Exact(aspect.to_string()),
            field: FieldValueSelector::Meta(MetaFieldValueSelector::Exact(field.to_string()))
        })
    }

    pub fn filter( &self, resource: Resource ) -> ResourceValue {
        match self {
            Self::Host(ResourceHostPropertyValueSelector::State{ aspect, field }) => {
                field.filter( aspect.filter(resource.state) )
            }
            Self::Registry(ResourceRegistryPropertyValueSelector::Config)=> {
               ResourceValue::Config(resource.archetype.config)
            }
            Self::Registry(ResourceRegistryPropertyValueSelector::Status) => {
                ResourceValue::None
            }
        }
    }
}


impl TryInto<ResourceRegistryPropertyValueSelector> for ResourcePropertyValueSelector {
    type Error = Error;

    fn try_into(self) -> Result<ResourceRegistryPropertyValueSelector, Self::Error> {
        match self {
            Self::Registry(registry) => {
                Ok(registry)
            }
           what => {
                Err(format!("'{}' is not a Registry Resource Property",what.to_string()).into())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourceHostPropertyValueSelector {
    State{ aspect: DataSetAspectSelector, field: FieldValueSelector }
}

impl Into<ResourcePropertyValueSelector> for ResourceHostPropertyValueSelector {
    fn into(self) -> ResourcePropertyValueSelector {
        ResourcePropertyValueSelector::Host(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourceRegistryPropertyValueSelector {
    Status,
    Config
}

impl Into<ResourcePropertyValueSelector> for ResourceRegistryPropertyValueSelector {
    fn into(self) -> ResourcePropertyValueSelector {
        ResourcePropertyValueSelector::Registry(self)
    }
}

impl FromStr for ResourcePropertyValueSelector {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover,selector) = parse_resource_property_value_selector(s)?;
        if !leftover.is_empty() {
            Err(format!("could not parse entire ResourcePropertyValueSelector: {} because of remaining string: {}", s, leftover ).into())
        } else {
            Ok(selector?)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum FieldValueSelector {
    All,
    Meta(MetaFieldValueSelector)
}

impl FieldValueSelector {
    pub fn filter( &self, selection: ResourceValue ) -> ResourceValue {

        match self {
            Self::All => {
                if let ResourceValue::Meta(meta)  = selection {
                    ResourceValue::Meta(meta)
                } else {
                    selection
                }
            }
            Self::Meta(selector) => {
               if let ResourceValue::Meta(meta)  = selection {
                   selector.filter(meta)
               } else {
                   ResourceValue::None
               }
            }
        }
    }

}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum MetaFieldValueSelector {
    All,
    Exact(String)
}

impl MetaFieldValueSelector {
    pub fn filter( &self, meta: Meta ) -> ResourceValue {
        match self {
           MetaFieldValueSelector::Exact(field) => {
                if meta.contains_key(field) {
                   let value = meta.get(field).expect(format!("expecting field: {}",field).as_str() );
                   ResourceValue::String(value.clone())
                } else {
                    ResourceValue::None
                }
            }
            MetaFieldValueSelector::All => {
                ResourceValue::Meta(meta)
            }
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceValue {
    None,
    DataSet(DataSet<BinSrc>),
    BinSrc(BinSrc),
    String(String),
    Meta(Meta),
    Resource(Resource),
    Status(Status),
    Config(ConfigSrc)
}

impl ToString for ResourceValue {
    fn to_string(&self) -> String {
        match self {
            ResourceValue::None => {
                "".to_string()
            }
            ResourceValue::DataSet(data) => {
                let mut rtn = String::new();
                for (k,v) in data {
                    match v {
                        BinSrc::Memory(bin) => {
                            rtn.push_str( String::from_utf8(bin.to_vec()).unwrap_or("UTF ERROR!".to_string() ).as_str() )
                        }
                    }
                }
                rtn
            }
            ResourceValue::BinSrc(v) => {
                match v {
                    BinSrc::Memory(bin) => {
                        String::from_utf8(bin.to_vec()).unwrap_or("UTF ERROR!".to_string() )
                    }
                }
            }
            ResourceValue::String(string) => {
                string.clone()
            }
            ResourceValue::Meta(_) => {
                "Meta printing not supported yet.".to_string()
            }
            ResourceValue::Resource(_) => {
                "Resource string printing not supported yet.".to_string()
            }
            ResourceValue::Status(status) => {
                status.to_string()
            }
            ResourceValue::Config(config) => {
                config.to_string()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceValues<R> {
    pub resource: R,
    pub values: HashMap<ResourcePropertyValueSelector,ResourceValue>
}

impl <R> ResourceValues <R> {

  pub fn empty(resource: R ) -> Self {
      Self {
          resource,
          values: HashMap::new()
      }
  }

  pub fn new(resource: R, values: HashMap<ResourcePropertyValueSelector,ResourceValue>) -> Self {
      Self {
          resource,
          values
      }
  }

  pub fn with<T>(self, resource: T) -> ResourceValues<T> {
      ResourceValues{
          resource,
          values: self.values
       }
  }
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ResourceRegistryPropertyAssignment {
    pub resource: ResourceIdentifier,
    pub property: ResourceRegistryProperty
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePropertyAssignment {
    pub resource: ResourceIdentifier,
    pub property: ResourceProperty
}

impl ToString for ResourcePropertyAssignment {
    fn to_string(&self) -> String {
        return format!( "{}::{}", self.resource.to_string(), self.property.to_string() )
    }
}

impl TryInto<ResourceRegistryPropertyAssignment> for ResourcePropertyAssignment {
    type Error = Error;

    fn try_into(self) -> Result<ResourceRegistryPropertyAssignment, Self::Error> {
        Ok(ResourceRegistryPropertyAssignment {
            resource: self.resource,
            property: self.property.try_into()?
        })
    }
}

impl FromStr for ResourcePropertyAssignment {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (leftover,result) = parse_resource_property_assignment(s)?;
        if leftover.len() > 0 {
            Err(format!("could not parse part of particle property assignment: '{}' unprocessed portion: '{}'", s,leftover ).into())
        } else {
           result
        }
    }
}


#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum ResourceProperty {
  Registry(ResourceRegistryProperty)
}

#[derive(Debug,Clone, Serialize, Deserialize)]
pub enum ResourceRegistryProperty {
    Config(ConfigSrc)
}

impl ToString for ResourceProperty {
    fn to_string(&self) -> String {
        match self {
            ResourceProperty::Registry(ResourceRegistryProperty::Config(_)) => {
                return "config".to_string()
            }
        }
    }
}


impl ResourceProperty {
    pub fn is_registry_property(&self) -> bool {
      match self {
          ResourceProperty::Registry(_) => {
              true
          }
      }
    }
}

impl TryInto<ResourceRegistryProperty> for ResourceProperty {
    type Error = Error;

    fn try_into(self) -> Result<ResourceRegistryProperty, Self::Error> {
        match self {
            ResourceProperty::Registry(property) => {
                Ok(property)
            }
        }
    }
}


