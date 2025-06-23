use core::str::FromStr;
use std::collections::HashMap;
use std::ops::Deref;
use getset::Getters;
use crate::command::common::{PropertyMod, SetProperties};
use crate::err::SpaceErr;
use crate::kind::Kind;
use crate::parse::{SkewerCase, SnakeCase};
use crate::point::Point;
use serde::Deserialize;
use serde::Serialize;
use validator::ValidateEmail;

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

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct PropertiesConfig {
    pub properties: HashMap<String, PropertyDef>,
    pub kind: Kind,
}

impl Deref for PropertiesConfig {
    type Target = HashMap<String, PropertyDef>;

    fn deref(&self) -> &Self::Target {
        &self.properties
    }
}

impl PropertiesConfig {
    pub fn new(kind: Kind) -> PropertiesConfig {
        Self {
            properties: HashMap::new(),
            kind,
        }
    }

    pub fn builder() -> PropertiesConfigBuilder {
        PropertiesConfigBuilder {
            kind: None,
            properties: HashMap::new(),
        }
    }

    pub fn required(&self) -> Vec<String> {
        let mut rtn = vec![];
        for (key, def) in &self.properties {
            if def.required {
                rtn.push(key.clone());
            }
        }
        rtn
    }

    pub fn defaults(&self) -> Vec<String> {
        let mut rtn = vec![];
        for (key, def) in &self.properties {
            if def.default.is_some() {
                rtn.push(key.clone());
            }
        }
        rtn
    }

    pub fn check_create(&self, set: &SetProperties) -> Result<(), SpaceErr> {
        for req in self.required() {
            if !set.contains_key(&req) {
                return Err(format!(
                    "{} missing required property: '{}'",
                    self.kind.to_string(),
                    req
                )
                .into());
            }
        }

        for (key, propmod) in &set.map {
            let def = self.get(key).ok_or(format!(
                "{} illegal property: '{}'",
                self.kind.to_string(),
                key
            ))?;
            match propmod {
                PropertyMod::Set { key, value, lock } => {
                    if def.constant && def.default.as_ref().unwrap().clone() != value.clone() {
                        return Err(format!(
                            "{} property: '{}' is constant and cannot be set",
                            self.kind.to_string(),
                            key
                        )
                        .into());
                    }
                    match def.source {
                        PropertySource::CoreReadOnly => {
                            return Err(format!("{} property '{}' is flagged CoreReadOnly and cannot be set within the Mesh", self.kind.to_string(), key).into());
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

    pub fn check_read(&self, keys: &Vec<String>) -> Result<(), SpaceErr> {
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
    kind: Option<Kind>,
    properties: HashMap<String, PropertyDef>,
}

impl PropertiesConfigBuilder {
    pub fn new() -> Self {
        let mut rtn = Self {
            kind: None,
            properties: HashMap::new(),
        };
        /// unwraps are bad unless it's a `&'static str`
        rtn.add_point(SnakeCase::from_str("bind").unwrap(), false, true).unwrap();
        rtn
    }

    pub fn build(self) -> Result<PropertiesConfig, SpaceErr> {
        Ok(PropertiesConfig {
            kind: self.kind.ok_or(SpaceErr::server_error(
                "kind must be set before PropertiesConfig can be built",
            ))?,
            properties: self.properties,
        })
    }

    pub fn kind(&mut self, kind: Kind) {
        self.kind.replace(kind);
    }

    pub fn add(
        &mut self,
        name: SnakeCase,
        pattern: Box<dyn PropertyPattern>,
        required: bool,
        mutable: bool,
        source: PropertySource,
        default: Option<String>,
        constant: bool,
        permits: Vec<PropertyPermit>,
    ) -> Result<(), SpaceErr> {
        let def = PropertyDef::new(name.clone(), required, mutable, source, default, constant, permits)?;
        self.properties.insert(name.to_string(), def);
        Ok(())
    }

    pub fn add_string(&mut self, name: &str) -> Result<(), SpaceErr> {
        let def = PropertyDef::new(false, true, PropertySource::Shell, None, false, vec![])?;
        self.properties.insert(name.to_string(), def);
        Ok(())
    }

    pub fn add_point(&mut self, name: SnakeCase, required: bool, mutable: bool) -> Result<(), SpaceErr> {
        let prop_name = name.to_string();
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
