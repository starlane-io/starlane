use crate::{SkewerCase, Resource, ResourceIdentifier, ResourceSelector, ResourceStub, FieldSelection, ResourcePath};
use std::str::FromStr;
use crate::error::Error;
use crate::data::{DataSet, BinSrc, Meta};
use serde::{Serialize,Deserialize};
use std::collections::HashMap;
use crate::parse::{parse_resource_property_value_selector, parse_resource_value_selector};
use crate::status::Status;

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

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ResourcePropertyValueSelector {
    None,
    State{ aspect: DataSetAspectSelector, field: FieldValueSelector },
    Status
}

impl ResourcePropertyValueSelector {

    pub fn state() -> Self {
        Self::State {
            aspect: DataSetAspectSelector::All,
            field: FieldValueSelector::All
        }
    }

    pub fn state_aspect(aspect: &str) -> Self {
        Self::State {
            aspect: DataSetAspectSelector::Exact(aspect.to_string()),
            field: FieldValueSelector::All
        }
    }

    pub fn state_aspect_field(aspect: &str, field: &str) -> Self {
        Self::State {
            aspect: DataSetAspectSelector::Exact(aspect.to_string()),
            field: FieldValueSelector::Meta(MetaFieldValueSelector::Exact(field.to_string()))
        }
    }

    pub fn filter( &self, resource: Resource ) -> ResourceValue {
        match self {
            ResourcePropertyValueSelector::State { aspect: selector, field } => {
                field.filter( selector.filter(resource.state) )
            }
            ResourcePropertyValueSelector::None => {
                ResourceValue::None
            }
            ResourcePropertyValueSelector::Status => {
                ResourceValue::None
            }
        }
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
    Status(Status)
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

