use std::collections::HashMap;
use std::marker::PhantomData;
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::{de, Deserialize, Deserializer};
use std::fmt;
use std::hash::Hash;
use serde_yaml::Value;
use crate::hyperspace::foundation::{DependencyKind, Kind};

pub type DependencyConfig = Config<DependencyKind>;

struct Config<K> where K: Kind{
    kind: K,
    config: Value
}

impl <K> Config<K> where K: Kind{
    fn new(kind: K, config: Value) -> Self {
        Self {
            kind,
            config
        }
    }

    pub fn kind() -> impl Kind {

    }
}


impl<'de,K> Deserialize<'de> for Config<K> where K: Kind{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field { Kind, Config };

        impl  Field  {
            fn kind() -> impl Kind {

            }
        }


        // This part could also be generated independently by:
        //
        //    #[derive(Deserialize)]
        //    #[serde(field_identifier, rename_all = "lowercase")]
        //    enum Field { Secs, Nanos }
        impl<'de,K> Deserialize<'de> for Field{
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {


                        formatter.write_str(format!("`{}` or `config`", phantom))
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            Kind::name() => Ok(Field::Kind),
                            "config" => Ok(Field::Config),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct ConfigVisitor;

        impl<'de> Visitor<'de> for ConfigVisitor {
            type Value = Config<K>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(K::)
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Config<K>, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let secs = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let nanos = seq.next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(Config::new(secs, nanos))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut secs = None;
                let mut nanos = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Secs => {
                            if secs.is_some() {
                                return Err(de::Error::duplicate_field("secs"));
                            }
                            secs = Some(map.next_value()?);
                        }
                        Field::Nanos => {
                            if nanos.is_some() {
                                return Err(de::Error::duplicate_field("nanos"));
                            }
                            nanos = Some(map.next_value()?);
                        }
                    }
                }
                let secs = secs.ok_or_else(|| de::Error::missing_field("secs"))?;
                let nanos = nanos.ok_or_else(|| de::Error::missing_field("nanos"))?;
                Ok(Config::new(secs, nanos))
            }
        }

        const FIELDS: &'static [&'static str] = &["secs", "nanos"];
        deserializer.deserialize_struct("Duration", FIELDS, ConfigVisitor)
    }
}