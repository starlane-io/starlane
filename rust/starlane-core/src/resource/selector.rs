use std::str::FromStr;

use nom::error::VerboseError;
use nom::IResult;

use crate::error::Error;
use crate::resource::{ResourceType, Kind};
use std::collections::{HashMap, HashSet};
use crate::mesh::serde::id::{Address, Specific};
use serde::{Serialize,Deserialize};

type Res<T, U> = IResult<T, U, VerboseError<T>>;

pub struct MultiResourceSelector {
    pub rt: ResourceType,
}

impl Into<ResourceSelector> for MultiResourceSelector {
    fn into(self) -> ResourceSelector {
        let mut selector = ResourceSelector::new();
        selector.add_field(FieldSelection::Type(self.rt));
        selector
    }
}

/*
fn resource_type( input: &str ) -> Res<&str,Result<ResourceType,Error>> {
    context( "resource_type",
       delimited( tag("<"), alpha1, tag(">")  )
    )(input).map( |(next_input, mut res )|{
        (next_input,ResourceType::from_str(res))
    })
}

 */

impl FromStr for MultiResourceSelector {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kind = Kind::from_str(s)?;

        let resource_type = ResourceType::from_str(parts.resource_type.as_str())?;

        Ok(MultiResourceSelector { rt: resource_type })
    }
}




#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSelector {
    pub meta: MetaSelector,
    pub fields: HashSet<FieldSelection>,
}

impl ResourceSelector {
    pub fn children_selector(parent: Address) -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Parent(parent));
        selector
    }

    pub fn children_of_type_selector(parent: Address, child_type: ResourceType) -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Parent(parent));
        selector.add_field(FieldSelection::Type(child_type));
        selector
    }

    pub fn app_selector() -> Self {
        let mut selector = Self::new();
        selector.add_field(FieldSelection::Type(ResourceType::App));
        selector
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum ConfigSrc {
    None,
    Artifact(Address)
}

impl ToString for ConfigSrc {
    fn to_string(&self) -> String {
        match self {
            ConfigSrc::None => {
                "None".to_string()
            }
            ConfigSrc::Artifact(address) => {
                address.to_string()
            }
        }
    }
}

impl FromStr for ConfigSrc {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        println!("ConfigSrc:: PARSEING: {}",s);
        if "None" == s {
            Ok(Self::None)
        } else {
            let path= ResourcePath::from_str(s)?;
            Ok(Self::Artifact(path))
        }
    }
}


#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LabelConfig {
    pub name: String,
    pub index: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRegistryInfo {
    pub names: Names,
    pub labels: Labels,
}

impl ResourceRegistryInfo {
    pub fn new() -> Self {
        ResourceRegistryInfo {
            names: Names::new(),
            labels: Labels::new(),
        }
    }
}

pub type Labels = HashMap<String, String>;
pub type Names = Vec<String>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetaSelector {
    None,
    Name(String),
    Label(LabelSelector),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelSelector {
    pub labels: HashSet<LabelSelection>,
}

impl ResourceSelector {
    pub fn new() -> Self {
        let fields = HashSet::new();
        ResourceSelector {
            meta: MetaSelector::None,
            fields: fields,
        }
    }

    pub fn resource_types(&self) -> HashSet<ResourceType> {
        let mut rtn = HashSet::new();
        for field in &self.fields {
            if let FieldSelection::Type(resource_type) = field {
                rtn.insert(resource_type.clone());
            }
        }
        rtn
    }

    pub fn add(&mut self, field: FieldSelection) {
        self.fields.retain(|f| !f.is_matching_kind(&field));
        self.fields.insert(field);
    }

    pub fn is_empty(&self) -> bool {
        if !self.fields.is_empty() {
            return false;
        }

        match &self.meta {
            MetaSelector::None => {
                return true;
            }
            MetaSelector::Name(_) => {
                return false;
            }
            MetaSelector::Label(labels) => {
                return labels.labels.is_empty();
            }
        };
    }

    pub fn name(&mut self, name: String) -> Result<(), Error> {
        match &mut self.meta {
            MetaSelector::None => {
                self.meta = MetaSelector::Name(name.clone());
                Ok(())
            }
            MetaSelector::Name(_) => {
                self.meta = MetaSelector::Name(name.clone());
                Ok(())
            }
            MetaSelector::Label(_selector) => {
                Err("Selector is already set to a LABEL meta selector".into())
            }
        }
    }

    pub fn add_label(&mut self, label: LabelSelection) -> Result<(), Error> {
        match &mut self.meta {
            MetaSelector::None => {
                self.meta = MetaSelector::Label(LabelSelector {
                    labels: HashSet::new(),
                });
                self.add_label(label)
            }
            MetaSelector::Name(_) => Err("Selector is already set to a NAME meta selector".into()),
            MetaSelector::Label(selector) => {
                selector.labels.insert(label);
                Ok(())
            }
        }
    }

    pub fn add_field(&mut self, field: FieldSelection) {
        self.fields.insert(field);
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum LabelSelection {
    Exact(Label),
}

impl LabelSelection {
    pub fn exact(name: &str, value: &str) -> Self {
        LabelSelection::Exact(Label {
            name: name.to_string(),
            value: value.to_string(),
        })
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum FieldSelection {
    Type(ResourceType),
    Kind(Kind),
    Specific(Specific),
    Parent(Address),
}



impl ToString for FieldSelection {
    fn to_string(&self) -> String {
        match self {
            FieldSelection::Identifier(id) => id.to_string(),
            FieldSelection::Type(rt) => rt.to_string(),
            FieldSelection::Kind(kind) => kind.to_string(),
            FieldSelection::Specific(specific) => specific.to_string(),
            FieldSelection::Owner(owner) => owner.to_string(),
            FieldSelection::Parent(parent) => parent.to_string(),
        }
    }
}

impl FieldSelection {
    pub fn is_matching_kind(&self, field: &FieldSelection) -> bool {
        match self {
            FieldSelection::Identifier(_) => {
                if let FieldSelection::Identifier(_) = field {
                    return true;
                }
            }
            FieldSelection::Type(_) => {
                if let FieldSelection::Type(_) = field {
                    return true;
                }
            }
            FieldSelection::Kind(_) => {
                if let FieldSelection::Kind(_) = field {
                    return true;
                }
            }
            FieldSelection::Specific(_) => {
                if let FieldSelection::Specific(_) = field {
                    return true;
                }
            }
            FieldSelection::Owner(_) => {
                if let FieldSelection::Owner(_) = field {
                    return true;
                }
            }
            FieldSelection::Parent(_) => {
                if let FieldSelection::Parent(_) = field {
                    return true;
                }
            }
        };
        return false;
    }
}





#[cfg(test)]
mod test {
    use std::str::FromStr;

    use crate::error::Error;
    use crate::resource::selector::MultiResourceSelector;

    #[test]
    pub fn test() -> Result<(), Error> {
        MultiResourceSelector::from_str("<SubSpace>")?;

        Ok(())
    }
}






