use crate::base::config::ConfigMap;
use crate::base::err::BaseErr;
use crate::base::foundation::kind::FoundationKind;
use bincode::Options;
use derive_name::Name;
use serde::de::{DeserializeOwned, MapAccess, Visitor};
use serde::ser::{Error, SerializeMap};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::SerializeAs;
use serde_yaml::{Mapping, Sequence, Value};
use std::collections::HashMap;
use std::fmt::{Display, Formatter, Write};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use crate::base::kind::IKind;

pub trait SubText {}

#[derive(Debug, Default, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Map(Mapping);

impl TryFrom<serde_yaml::Value> for Map {
    type Error = BaseErr;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Mapping(mapping) => Ok(Self(mapping)),
            other => {
                other.var_string();
                Err(BaseErr::serde_err(format!(
                    "expecting `Value::Mapping` found: 'Value::{}'",
                    other.var_string()
                )))
            }
        }
    }
}

impl TryFrom<&str> for Map {
    type Error = BaseErr;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        serde_yaml::from_str(value).map_err(Self::Error::serde_err)
    }
}

pub trait SerMap {
    fn to_map(self) -> Result<Map, BaseErr>; /* {
                                                       Err(FoundationErr::serde_err("this type does not support Serialization"))
                                                   }
                                                   */

    fn to_sequence(self) -> Result<Sequence, BaseErr>; /*{
                                                                 Err(FoundationErr::serde_err("this type does not support Serialization"))
                                                             }
                                                             */

    fn to_value(self) -> Result<serde_yaml::Value, BaseErr>; /* {
                                                                       Err(FoundationErr::serde_err("this type does not support Serialization"))
                                                                   }
                                                                   */
}

pub enum Warpy {
    Ser { phantom: PhantomData<()> },
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocalType {}

impl<T> SerializeAs<T> for LocalType {
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!()
    }
}

/*
pub fn to_config_map<K, C, F>(
    map: impl SerMap,
    factory: F,
) -> Result<ConfigMap<K, C>, FoundationErr>
where
    K: Eq + PartialEq + Hash + Clone + DeserializeOwned,
    C: DeserializeOwned + Clone,
    F: Fn((K, Arc<C>)) -> Result<C, FoundationErr> + Copy,
{
    let mut maps = HashMap::new();

    for item in map.to_sequence()? {
        let map = item.to_map()?;
        let kind: K  = map.kind()?;
        let item = factory(kind.clone(), Arc::new(map.clone()))?;
        maps.insert(kind, item);
    }

    Ok(maps)
}

 */

impl<T> SerMap for T
where
    T: Serialize,
{
    fn to_map(self) -> Result<Map, BaseErr> {
        serde_yaml::to_value(self)
            .map_err(BaseErr::serde_err)?
            .to_map()
    }

    fn to_sequence(self) -> Result<serde_yaml::Sequence, BaseErr> {
        serde_yaml::to_value(self)
            .map_err(BaseErr::serde_err)?
            .to_sequence()
    }

    fn to_value(self) -> Result<serde_yaml::Value, BaseErr> {
        serde_yaml::to_value(self).map_err(BaseErr::serde_err)
    }
}

pub trait VariantStr {
    fn var_string(&self) -> String;

    fn display(&self) -> impl Display;
}

impl VariantStr for serde_yaml::Value {
    fn var_string(&self) -> String {
        match self {
            Value::Null => "Null".to_string(),
            Value::Bool(_) => "Bool".to_string(),
            Value::Number(_) => "Number".to_string(),
            Value::String(_) => "String".to_string(),
            Value::Sequence(_) => "Sequence".to_string(),
            Value::Mapping(_) => "Mapping".to_string(),
            Value::Tagged(_) => "Tagged".to_string(),
        }
    }

    fn display(&self) -> impl Display {
        match self {
            Value::Null => "Null".to_string(),
            Value::Bool(v) => v.to_string(),
            Value::Number(v) => v.to_string(),
            Value::String(v) => v.to_string(),
            Value::Sequence(v) => v
                .clone()
                .into_iter()
                .map(|value| value.display().to_string())
                .collect::<Vec<String>>()
                .join(",")
                .to_string(),
            Value::Mapping(v) => v
                .clone()
                .into_iter()
                .map(|(key, value)| {
                    format!(
                        "{}: \"{}\"",
                        key.display().to_string(),
                        value.display().to_string()
                    )
                })
                .collect::<Vec<String>>()
                .join(",")
                .to_string(),
            Value::Tagged(v) => "TaggedValueNotImplemented".to_string(),
        }
    }
}

impl Map {
    pub fn to_value(self) -> Value {
        Value::Mapping(self.0)
    }

    pub fn from_field<M>(&self, field: &'static str) -> Result<M, BaseErr>
    where
        M: DeserializeOwned,
    {
        match self.get(field) {
            Some(value) => {
                Ok(serde_yaml::from_value(value.clone()).map_err(BaseErr::config_err)?)
            }
            None => Err(BaseErr::config_err(
                "missing required attribute 'kind'",
            )),
        }
    }

    pub fn from_field_opt<M>(&self, field: &'static str) -> Result<Option<M>, BaseErr>
    where
        M: DeserializeOwned,
    {
        match self.get(field) {
            None => Ok(None),
            Some(value) => Ok(Some(
                serde_yaml::from_value(value.clone()).map_err(BaseErr::config_err)?,
            )),
        }
    }

    pub fn kind<K>(&self) -> Result<K, BaseErr>
    where
        K: Eq + PartialEq + Hash + DeserializeOwned + Name,
    {
        let kind_as_value = self
            .0
            .get("kind")
            .ok_or_else(|| BaseErr::missing_kind_declaration(K::name()))?;
        let result = serde_yaml::from_value(kind_as_value.clone());
        match result {
            Ok(kind) => Ok(kind),
            Err(_) => {
                /// now we have to convert kind into a string
                let kind_str = serde_yaml::to_string(kind_as_value)
                    .map_err(|err| BaseErr::config_err(format!("{:?}", err)))?;
                Err(BaseErr::kind_not_found(K::name(), kind_str))
            }
        }
    }

    pub fn parse_list<D>(
        &self,
        field: &'static str,
        f: impl Fn(Map) -> Result<D, BaseErr>,
    ) -> Result<Vec<D>, BaseErr>
    where
        D: DeserializeOwned,
    {
        let items: Option<Vec<Map>> = self.from_field_opt(field)?;
        match items {
            None => Ok(vec![]),
            Some(items) => {
                let mut rtn = vec![];
                for item in items {
                    rtn.push(f(item)?);
                }
                Ok(rtn)
            }
        }
    }

    /// when you want to parse a list of configurations that are all the same
    pub fn parse_same<K, D>(&self, field: &'static str) -> Result<HashMap<K, D>, BaseErr>
    where
        D: DeserializeOwned,
        K: Eq + PartialEq + Hash + DeserializeOwned + derive_name::Name,
    {
        let items: Vec<Map> = self.from_field(field)?;
        let mut rtn = HashMap::new();
        for map in items {
            let kind = map.kind()?;
            let item: D =
                serde_yaml::from_value(map.to_value()).map_err(BaseErr::config_err)?;
            rtn.insert(kind, item);
        }
        Ok(rtn)
    }
}

impl Deref for Map {
    type Target = Mapping;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Map {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Map {
    fn new() -> Map {
        Map(Default::default())
    }
}

pub trait IntoSer {
    fn into_ser(&self) -> Box<dyn SerMap>;
}

pub trait CreateProxy {
    type Proxy;
    fn proxy(&self) -> Result<Self::Proxy, BaseErr>;
}


#[test]
pub fn kind_map() {
    //        let mut map: Map= Map::new();
    let mut map = HashMap::new();
    map.insert(
        "kind".to_string(),
        serde_yaml::to_value("DockerDesktop").unwrap(),
    );
    map.insert("hello".to_string(), serde_yaml::to_value("doctor").unwrap());
    map.insert(
        "yesterday".to_string(),
        serde_yaml::to_value("tomorow").unwrap(),
    );
    let yaml = serde_yaml::to_string(&map).unwrap();
    let map: Map = serde_yaml::from_str(&yaml).unwrap();

    let kind: FoundationKind = map.kind().unwrap();

    println!("kind: `{}`\n\n", kind);

    let out = serde_yaml::to_string(&map).unwrap();
    println!("{}", out);
}

#[test]
pub fn list() {
    let list = vec!["hello", "doctor", "yesterday", "tomorrow"];
    println!("{}", serde_yaml::to_string(&list).unwrap());
}

#[derive(Clone, Deserialize)]
pub struct Registry {
    pub database: String,
    pub seed: String,
}

struct ConfigMapVisitor<'de, K, C>
where
    K: Deserialize<'de> + Eq + PartialEq + Hash + Clone,
    C: Deserialize<'de> + Clone,
{
    config: PhantomData<fn() -> ConfigMap<K, C>>,
    lifetime: PhantomData<&'de ()>,
}

impl<'de, K, C> Default for ConfigMapVisitor<'de, K, C>
where
    K: Deserialize<'de> + Eq + PartialEq + Hash + Clone,
    C: Deserialize<'de> + Clone,
{
    fn default() -> Self {
        Self {
            config: Default::default(),
            lifetime: Default::default(),
        }
    }
}

impl<'de, K, C> serde::de::Visitor<'de> for ConfigMapVisitor<'de, K, C>
where
    K: Deserialize<'de> + Eq + PartialEq + Hash + Clone,
    C: Deserialize<'de> + Clone,
{
    type Value = ConfigMap<K, C>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a key/value mapping object")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = ConfigMap::new();

        while let Some((key, value)) = access.next_entry()? {
            map.insert(key, value);
        }
        Ok(map)
    }
}



#[cfg(test)]
pub mod test {
    use serde::{Deserialize, Serialize};
    use std::fmt::Debug;
    use std::sync::Arc;
    use derive_name::{Name, Named};
    use downcast_rs::{impl_downcast, Downcast, DowncastSync};
    #[test]
    pub fn test_serde_factory() -> anyhow::Result<()> {
        #[typetag::serde(tag = "kind")]
        trait SomeConfig: DowncastSync+Debug {
            fn say_my_name(&self) -> &'static str;
        }
        impl_downcast!(sync SomeConfig);



        #[derive(
            Eq,
            PartialEq,
            Hash,
            Deserialize,
            Serialize,
            Debug,
            strum_macros::EnumString,
            strum_macros::Display,
        )]
        enum Key {
            AConf,
            BConf,
        }

        #[derive(Clone, Debug, Default, Serialize, Deserialize,Name)]
        struct AConf {
            something: u32,
        }

        #[typetag::serde]
        impl SomeConfig for AConf {
            fn say_my_name(&self) -> &'static str {
                self.name()
            }
        }

        #[derive(Clone, Debug, Default, Serialize, Deserialize)]
        struct BConf {
            and_another_thing: String,
        }
        #[typetag::serde]
        impl SomeConfig for BConf {
            fn say_my_name(&self) -> &'static str {
                "BConf"
            }

        }

        let raw = r#"
- kind: AConf
  something: 34
- and_another_thing: "Hello World!"
  kind: BConf

        "#;

        let mut items: Vec<Arc<dyn SomeConfig>> = serde_yaml::from_str(raw)?;
        let item0  =items.remove(0);
        let item1  =items.remove(0);

        assert_eq!("AConf", item0.say_my_name());
        assert_eq!("BConf", item1.say_my_name());

        println!("0:\n{:?}", item0);
        println!("1:\n{:?}", item1);

        let conf0 = item0.clone().downcast_arc::<AConf>().unwrap();
        let conf1 = item1.clone().downcast_arc::<BConf>().unwrap();



        println!("AConf::something(&conf0): {}", conf0.something);
        println!("BConf::and_another_thing(&conf1): {}", conf1.and_another_thing);

        println!("item0 is still alive?: {}",item0.say_my_name());


        Ok(())
    }

    #[test]
    pub fn test_downgrade() -> anyhow::Result<()> {
        trait Trait {}
        impl Trait for String {}
        impl Trait for u8 {}

        impl dyn Trait {
            // SAFETY: I hope you know what you're doing
            unsafe fn downcast<T>(&self) -> &T {
                &*(self as *const dyn Trait as *const T)
            }
        }

            let a: &dyn Trait = &42_u8;
            let b: &dyn Trait = &String::from("hello");

            let _number: u8 = *unsafe { a.downcast::<u8>() };
            let _text: &str = unsafe { b.downcast::<String>() };

        Ok(())
    }

    #[test]
    pub fn analyze_vtable() {

        trait Trait {
            fn do_something(&self);
            fn do_something_else(&self);
        }

        impl Trait for String {
            fn do_something(&self) { println!("a string: {}", self); }
            fn do_something_else(&self) { println!("a string: {}", self); }
        }

        impl Trait for u128 {
            fn do_something(&self) { println!("a y=u128: {}", self); }
            fn do_something_else(&self) { println!("a u128: {}", self); }
        }


        fn analyse_fatp<T: ?Sized>(p: *const T, datasize: usize, vtsize: usize) {
            let addr = &p as *const *const T as *const usize;
            let second = (addr as usize + std::mem::size_of::<usize>()) as *const usize;
            let datap = unsafe { *addr } as *const usize;
            let vtp = unsafe { *second } as *const usize;
            let data = unsafe { std::slice::from_raw_parts(datap, datasize) };
            let vtable = unsafe { std::slice::from_raw_parts(vtp, vtsize) };
            let vtable = vtable
                .iter()
                .map(|val| format!("0x{:x}", val))
                .collect::<Vec<_>>();

            println!("Addr of fat pointer (1st word): {:p}", addr);
            println!("Addr of fat pointer (2nd word): {:p}", second);
            println!("Addr of data:                   {:p}", datap);
            println!("Addr of vtable:                 {:p}", vtp);
            println!("Data:   {:?}", data);
            println!("VTable: {:?}", vtable);
        }

        let obj: &dyn Trait = &String::from("hello");
        dbg!(String::do_something as *const ());
        dbg!(String::do_something_else as *const ());
        analyse_fatp(obj, std::mem::size_of::<String>() / std::mem::size_of::<usize>(), 5);

        let obj: &dyn Trait = &12_u128;
//        dbg!(u128::do_something as *const ());
//        dbg!(u128::do_something_else as *const ());
        analyse_fatp(obj, std::mem::size_of::<u128>() / std::mem::size_of::<usize>(), 5);

    }

}
