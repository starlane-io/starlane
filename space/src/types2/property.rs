use core::str::FromStr;
use std::collections::hash_map::Iter;
use std::collections::HashMap;
use std::ops::Deref;
use getset::Getters;
use crate::err::SpaceErr;
use crate::parse::{SkewerCase, SnakeCase};
use crate::point::Point;
use serde::Deserialize;
use serde::Serialize;
use thiserror::__private::AsDisplay;
use validator::ValidateEmail;
use crate::types::Absolute;

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq,Getters)]
#[get = "pub"]
pub struct PropertyDef {
    name: SnakeCase,
    required: bool,
    mutable: bool,
    source: PropertySource,
    default: Option<String>,
    constant: bool,
    permits: Vec<PropertyPermit>,
}

impl PropertyDef {
    pub fn new(
        name: SnakeCase,
        required: bool,
        mutable: bool,
        source: PropertySource,
        default: Option<String>,
        constant: bool,
        permits: Vec<PropertyPermit>,
    ) -> Result<Self, SpaceErr> {
        if constant {
            default
                .as_ref()
                .ok_or("if PropertyDef is a constant then 'default' value must be set")?;
        }

        Ok(Self {
            name,
            required,
            mutable,
            source,
            default,
            constant,
            permits,
        })
    }
}

#[cfg(test)]
impl PropertyDef {
    pub fn mock_less() -> Self {
        Self {
            
            name: SnakeCase::from_str("less").unwrap(),
            required: false,
            mutable: false,
            source: PropertySource::Shell,
            default: None,
            constant: false,
            permits: vec![],
        }
    }

    pub fn mock_fae() -> Self {
        Self {

            name: SnakeCase::from_str("fae").unwrap(),
            required: false,
            mutable: false,
            source: PropertySource::Shell,
            default: None,
            constant: false,
            permits: vec![],
        }
    }

    pub fn mock_modus() -> Self {
        Self {

            name: SnakeCase::from_str("modus").unwrap(),
            required: false,
            mutable: false,
            source: PropertySource::Shell,
            default: None,
            constant: false,
            permits: vec![],
        }
    }
}

pub trait PropertyPattern: Send + Sync + 'static {
    fn is_match(&self, value: &String) -> Result<(), SpaceErr>;
}

#[derive(Clone)]
pub struct AnythingPattern {}

impl PropertyPattern for AnythingPattern {
    fn is_match(&self, value: &String) -> Result<(), SpaceErr> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct PointPattern {}

impl PropertyPattern for PointPattern {
    fn is_match(&self, value: &String) -> Result<(), SpaceErr> {
        use std::str::FromStr;
        Point::from_str(value.as_str())?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct U64Pattern {}

impl PropertyPattern for U64Pattern {
    fn is_match(&self, value: &String) -> Result<(), SpaceErr> {
        use std::str::FromStr;
        match u64::from_str(value.as_str()) {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string().into()),
        }
    }
}

#[derive(Clone)]
pub struct BoolPattern {}

impl PropertyPattern for BoolPattern {
    fn is_match(&self, value: &String) -> Result<(), SpaceErr> {
        use std::str::FromStr;
        match bool::from_str(value.as_str()) {
            Ok(_) => Ok(()),
            Err(err) => Err(err.to_string().into()),
        }
    }
}

#[derive(Clone)]
pub struct UsernamePattern {}

impl PropertyPattern for UsernamePattern {
    fn is_match(&self, value: &String) -> Result<(), SpaceErr> {
        SkewerCase::from_str(value.as_str())?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct EmailPattern {}

impl PropertyPattern for EmailPattern {
    fn is_match(&self, email: &String) -> Result<(), SpaceErr> {
        if !email.validate_email() {
            Err(format!("not a valid email: '{}'", email).into())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum PropertySource {
    Shell,
    Core,
    CoreReadOnly,
    CoreSecret,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq,Getters)]
#[get = "pub"]
pub struct PropertiesConfig {
    absolute: Absolute,
    properties: HashMap<SnakeCase, PropertyDef>,
}

impl Deref for PropertiesConfig {
    type Target = HashMap<SnakeCase, PropertyDef>;

    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

impl PropertiesConfig {

    pub fn builder(absolute: Absolute) -> PropertiesConfigBuilder {
        PropertiesConfigBuilder {
            absolute,
            properties: HashMap::new(),
        }
    }

    pub fn required(&self) -> Vec<SnakeCase> {
        let mut rtn = vec![];
        for (key, def) in &self.properties {
            if def.required {
                rtn.push(key.clone());
            }
        }
        rtn
    }

    pub fn defaults(&self) -> Vec<SnakeCase> {
        let mut rtn = vec![];
        for (key, def) in &self.properties {
            if def.default.is_some() {
                rtn.push(key.clone());
            }
        }
        rtn
    }
    

    pub fn check_create(&self, set: &SetProperties) -> Result<(), SpaceErr> {
        for req in &self.required() {
            if !set.contains_key(req) {
                return Err(format!(
                    "{} missing required property: '{}'",
                    self.absolute.to_string(),
                    req
                )
                .into());
            }
        }

        for (key, propmod) in &set.map {
            let def = self.get(key).ok_or(format!(
                "{} illegal property: '{}'",
                self.absolute.to_string(),
                key
            ))?;
            match propmod {
                PropertyMod::Set { key, value, lock } => {
                    if def.constant && def.default.as_ref().unwrap().clone() != value.clone() {
                        return Err(format!(
                            "{} property: '{}' is constant and cannot be set",
                            self.absolute.to_string(),
                            key
                        )
                        .into());
                    }
                    match def.source {
                        PropertySource::CoreReadOnly => {
                            return Err(format!("{} property '{}' is flagged CoreReadOnly and cannot be set within the Mesh", self.absolute.to_string(), key).into());
                        }
                        _ => {}
                    }
                }
                PropertyMod::UnSet(_) => {
                    return Err(format!("cannot unset: '{}' during particle create", key).into());
                }
            }
        }
        Ok(())
    }

    pub fn check_update(&self, set: &SetProperties) -> Result<(), SpaceErr> {
        for (key, propmod) in &set.map {
            let def = self
                .get(key)
                .ok_or(format!("illegal property: '{}'", key))?;
            match propmod {
                PropertyMod::Set { key, value, lock } => {
                    if def.constant {
                        return Err(
                            format!("property: '{}' is constant and cannot be set", key).into()
                        );
                    }
                    match def.source {
                        PropertySource::CoreReadOnly => {
                            return Err(format!("property '{}' is flagged CoreReadOnly and cannot be set within the Mesh", key).into());
                        }
                        _ => {}
                    }
                }
                PropertyMod::UnSet(_) => {
                    if !def.mutable {
                        return Err(format!("property '{}' is immutable and cannot be changed after particle creation", key).into());
                    }
                    if def.required {
                        return Err(
                            format!("property '{}' is required and cannot be unset", key).into(),
                        );
                    }
                }
            }
        }
        Ok(())
    }

    pub fn check_read(&self, keys: &Vec<SnakeCase>) -> Result<(), SpaceErr> {
        for key in keys {
            let def = self
                .get(key)
                .ok_or(format!("illegal property: '{}'", key))?;
            match def.source {
                PropertySource::CoreSecret => {
                    return Err(format!(
                        "property '{}' is flagged CoreSecret and cannot be read within the Mesh",
                        key
                    )
                    .into());
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn fill_create_defaults(&self, set: &SetProperties) -> Result<SetProperties, SpaceErr> {
        let mut rtn = set.clone();
        let defaults = self.defaults();
        for d in defaults {
            if !rtn.contains_key(&d) {
                let def = self
                    .get(&d)
                    .ok_or(format!("expected default property def: {}", &d))?;
                let value = def
                    .default
                    .as_ref()
                    .ok_or(format!("expected default property def: {}", &d))?
                    .clone();
                rtn.push(PropertyMod::Set {
                    key: d,
                    value,
                    lock: false,
                });
            }
        }
        Ok(rtn)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub enum PropertyPermit {
    Read,
    Write,
}

pub struct PropertiesConfigBuilder {
    absolute: Absolute,
    properties: HashMap<SnakeCase, PropertyDef>,
}

impl PropertiesConfigBuilder {
    pub fn new(absolute: Absolute) -> Self {
        let mut rtn = Self {
            absolute,
            properties: HashMap::new(),
        };
        /// disabled for now while properties are getting better sorted out
//        rtn.add_property(SnakeCase::from_str("bind").unwrap(), false, true).unwrap();
        rtn
    }

    pub fn build(self) -> PropertiesConfig {
        PropertiesConfig {
            absolute: self.absolute,
            properties: self.properties,
        }
    }

    pub fn add(
        &mut self,
        name: SnakeCase,
        _: Box<dyn PropertyPattern>,
        required: bool,
        mutable: bool,
        source: PropertySource,
        default: Option<String>,
        constant: bool,
        permits: Vec<PropertyPermit>,
    ) -> Result<(), SpaceErr> {
        self.push(PropertyDef::new(name.clone(), required, mutable, source, default, constant, permits)?);
        Ok(())
    }
    
    pub fn push( &mut self, def: PropertyDef){
        self.properties.insert(def.name.clone(), def);
    }
    
    
    pub fn remove(&mut self, name: & SnakeCase)  {
        self.properties.remove(name);
    }

    pub fn add_string(&mut self, name: SnakeCase) -> Result<(), SpaceErr> {
        let def = PropertyDef::new(name.clone(),false, true, PropertySource::Shell, None, false, vec![])?;
        self.properties.insert(name, def);
        Ok(())
    }

    pub fn add_property(&mut self, name: SnakeCase, required: bool, mutable: bool) -> Result<(), SpaceErr> {
        let prop_name = name.clone();
        let def = PropertyDef::new(
            name,
            required,
            mutable,
            PropertySource::Shell,
            None,
            false,
            vec![],
        )?;
        self.properties.insert(prop_name, def);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum PropertyMod {
    Set {
        key: SnakeCase,
        value: String,
        lock: bool,
    },
    UnSet(SnakeCase),
}

impl PropertyMod {
    pub fn set_or<E>(&self, err: E) -> Result<String, E> {
        match self {
            Self::Set { key, value, lock } => Ok(value.clone()),
            Self::UnSet(_) => Err(err),
        }
    }

    pub fn opt(&self) -> Option<String> {
        match self {
            Self::Set { key, value, lock } => Some(value.clone()),
            Self::UnSet(_) => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct SetProperties {
    pub map: HashMap<SnakeCase, PropertyMod>,
}

impl Default for SetProperties {
    fn default() -> Self {
        Self {
            map: Default::default(),
        }
    }
}

impl SetProperties {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn append(&mut self, properties: SetProperties) {
        for (_, property) in properties.map.into_iter() {
            self.push(property);
        }
    }

    pub fn push(&mut self, property: PropertyMod) {
        match &property {
            PropertyMod::Set { key, value, lock } => {
                self.map.insert(key.clone(), property);
            }
            PropertyMod::UnSet(key) => {
                self.map.insert(key.clone(), property);
            }
        }
    }

    pub fn iter(&self) -> Iter<'_, SnakeCase, PropertyMod> {
        self.map.iter()
    }
}

impl Deref for SetProperties {
    type Target = HashMap<SnakeCase, PropertyMod>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

pub type PropertyName = SnakeCase;