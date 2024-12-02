use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fmt;
use std::fmt::{Formatter, Write};
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use derive_name::{Name, Named};
use futures::StreamExt;
use rustls::pki_types::Der;
use serde::__private::de::missing_field;
use serde::de::{DeserializeOwned, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde_yaml::{Mapping, Value};
use serde_yaml::Value::Tagged;
use crate::hyperspace::foundation::err::FoundationErr;
use crate::hyperspace::foundation::kind::{FoundationKind, IKind};
/*
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AbstractMappings<'z,V> where V: Serialize+DeserializeOwned+'z{
   map: HashMap<String,V>,
   phantom: PhantomData<&'z V>
}

 */

pub trait SubText{

}




#[derive(Debug, Default,Clone, Eq, PartialEq, Serialize,Deserialize)]
pub struct Map(Mapping);



impl Map {

    pub fn to_value(self) -> Value {
        Value::Mapping(self.0)
    }

    pub fn from_field<'z,M>( &self, field: &'static str  ) -> Result<M,FoundationErr> where M: Deserialize<'z> {
        match self.get(field) {
            Some(value) => {
                Ok(serde_yaml::from_value(value.clone()).map_err(FoundationErr::config_err)?)
            }
            None => Err(FoundationErr::config_err("missing required attribute 'kind'"))
        }
    }

    pub fn from_field_opt<'z,M>( &self, field: &'static str  ) -> Result<Option<M>,FoundationErr> where M: Deserialize<'z> {
        match self.get(field) {
            None => Ok(None),
            Some(value) => {
               Ok(Some(serde_yaml::from_value(value.clone()).map_err(FoundationErr::config_err)?))
            }
        }
    }


    pub fn kind<K>(&self) -> Result<K,FoundationErr> where K: IKind {
           let kind_as_value =self.0.get("kind").ok_or_else(|| FoundationErr::missing_kind_declaration(K::name()))?;
           let result = serde_yaml::from_value(kind_as_value.clone());
        match result {
            Ok(kind) => {
                Ok(kind)
            },
            Err(_) => {
                /// now we have to convert kind into a string
                let kind_str = serde_yaml::to_string(kind_as_value).map_err(|err|FoundationErr::config_err(format!("{:?}",err)))?;
                Err(FoundationErr::kind_not_found(K::name(),kind_str))
            }
        }
    }


    pub fn parse_list<D>(&self,field: &'static str, f: impl Fn(Map) -> Result<D,FoundationErr> ) -> Result<Vec<D>,FoundationErr> where D: DeserializeOwned{
        let items: Option<Vec<Map>>  = self.from_field_opt(field)?;
        match items {
            None => Ok(vec![]),
            Some(items) => {
                let mut rtn = vec![];
                for item in items{
                    rtn.push(f(item)?);
                }
                Ok(rtn)
            }
        }

    }

    pub fn parse_kinds<K,D>(&self,field: &'static str, f: impl Fn(Map) -> Result<D,FoundationErr> ) -> Result<HashMap<K,D>,FoundationErr> where D: DeserializeOwned, K: Eq+PartialEq+Hash+DeserializeOwned {
        let items: Option<Vec<Map>>  = self.from_field_opt(field)?;

        match items {
            None => {
                Ok(HashMap::new())
            }
            Some(items) => {
                let mut rtn = HashMap::new();
                for item in items {
                    rtn.insert(item.from_field("kind")?, f(item)?);
                }
                Ok(rtn)
            }
        }


    }

    pub fn parse_same<K,D>(&self,field: &'static str ) -> Result<HashMap<K,D>,FoundationErr> where D: DeserializeOwned, K: Eq+PartialEq+Hash+DeserializeOwned {
        let items: Vec<Map>  = self.from_field(field)?;
        let mut rtn = HashMap::new();
        for item in items {
            let item = serde_yaml::from_value(item.to_value())?;
            rtn.insert(item.from_field("kind")?, item);
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
        & mut self.0
    }
}

impl  Map {
    fn new() -> Map {
        Map(Default::default())
    }
}
/*

struct MyMapVisitor<K> where K: IKind   {
    marker: PhantomData<fn() -> Map<K>>
}

impl <K> MyMapVisitor<K> where K: IKind  {
    fn new() -> Self {
        MyMapVisitor {
            marker: Default::default()
        }
    }
}


impl<'de,K> Visitor<'de> for MyMapVisitor<K> where K: IKind
{
    type Value = Map<K>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a very special map")
    }


    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = Map::new();

        // While there are entries remaining in the input, add them
        // into our map.
        while let Some((key, value)) = access.next_entry()? {
            map.insert(key, value);
        }

        Ok(map)
    }
}




// This is the trait that informs Serde how to deserialize MyMap.
impl<'de,K> Deserialize<'de> for Map<K> where K:IKind
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(MyMapVisitor::new())
    }
}

/*
impl <'de,V> Deserialize<'de> for AbstractMappings<'de,V> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>
    {
        todo!()
    }
}

 */

 */

/*


impl <'z,V> Default for AbstractMappings<'z,V> where V: Serialize+Deserialize<'z>{
    fn default() -> Self {
        Self {
            ..Default::default()
        }
    }
}


impl <'z,V> Deref for AbstractMappings<'z,V> where V: Serialize+Deserialize<'z>{
    type Target = HashMap<&'static str,V>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl <'z,V> DerefMut for AbstractMappings<'z,V> where V: Serialize+Deserialize<'z>{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

 */



















    #[test]
    pub fn kind_map() {
//        let mut map: Map= Map::new();
        let mut map = HashMap::new();
        map.insert("kind".to_string(), serde_yaml::to_value("DockerDesktop").unwrap());
        map.insert("hello".to_string(), serde_yaml::to_value("doctor").unwrap());
        map.insert("yesterday".to_string(), serde_yaml::to_value("tomorow").unwrap());
        let yaml = serde_yaml::to_string(&map).unwrap();
        let map: Map =  serde_yaml::from_str(&yaml).unwrap();

        println!("kind: `{}`\n\n", map.kind().unwrap());

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
        pub seed: String
    }
/*


    pub struct StrVariant;
    impl Serialize for StrVariant {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer
        {
            serializer.serialize_str("hello")
            /*
            let mut serializer = serializer.serialize_str("Regsitry")?;
            let mut ser = serializer.serialize_struct_variant("Registry", 0, "Registry", 3)?;
            ser.serialize_field("database", "registry")?;
            ser.serialize_field("seed", "https://starlane.io")?;
            ser.serialize_field("can_scorch", "false")?;
            ser.end()

             */
        }
    }

    pub struct Normal;

    impl Serialize for Normal {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer
        {
            let mut ser = serializer.serialize_struct("Normal", 3)?;
            ser.serialize_field("database", "registry")?;
            ser.serialize_field("seed", "http://starlane.io")?;
            ser.serialize_field("can_scorch", "true")?;
            ser.end()
        }
    }

    #[derive(Clone,Serialize)]
    pub struct Blah {
        pub kind: String,
        pub database: String,
        pub seed: String,
        pub can_nuke: bool
    }

    #[test]
    pub fn new_way() {
        let b = Blah{
            kind: "Registry".to_string(),
            database: "registry".to_string(),
            seed: "https://starlane.io".to_string(),
            can_nuke: false,
        };

        let list = vec![b.clone(),b.clone(),b.clone()];

        println!("{}", serde_yaml::to_string(&list).unwrap());
    }

    #[test]
    pub fn structs() {
        println!("---STRUCT----\n");
        println!("{}", serde_yaml::to_string(&Normal).unwrap());
        println!("\n\n\n---STRUCT-VARIANT----\n");
        println!("{}", serde_yaml::to_string(&StrVariant).unwrap());
    }
}



/*
impl Serialize for Thingy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_key("Postgres")?;
    }
}

 */





fn blah() -> &'static str {
    "beware"
}
#[derive(Debug,Eq,PartialEq,Clone)]
struct Thingy {
    thingy: HashMap<String, String>,
}
impl Thingy {
    fn new(map: HashMap<String, String>) -> Thingy {
        Self {
            thingy: map
        }
    }
}


    impl serde::Serialize for Thingy {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut _serde_state = serde::Serializer::serialize_struct(serializer, "Thingy", false as usize + 1)?;
            serde::ser::SerializeStruct::serialize_field(&mut _serde_state, "Registry", &self.thingy)?;
            serde::ser::SerializeStruct::end(_serde_state)
        }
    }

impl<'de> serde::Deserialize<'de> for Thingy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[allow(non_camel_case_types)]
        #[doc(hidden)]
        enum Field { field0, ignore }
        #[doc(hidden)]
        struct FieldVisitor;
        impl<'de> serde::de::Visitor<'de> for FieldVisitor {
            type Value = Field;
            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result { Formatter::write_str(formatter, "field identifier") }
            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    0u64 => Ok(Field::field0),
                    _ => Ok(Field::ignore),
                }
            }
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "Registry" => Ok(Field::field0),
                    _ => { Ok(Field::ignore) }
                }
            }
            fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    b"Registry" => Ok(Field::field0),
                    _ => { Ok(Field::ignore) }
                }
            }
        }
        impl<'de> serde::Deserialize<'de> for Field {
            #[inline]
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            { serde::Deserializer::deserialize_identifier(deserializer, FieldVisitor) }
        }
        #[doc(hidden)]
        struct Visitor<'de> {
            marker: PhantomData<Thingy>,
            lifetime: PhantomData<&'de ()>,
        }
        impl<'de> serde::de::Visitor<'de> for Visitor<'de> {
            type Value = Thingy;
            fn expecting(&self, formatter: &mut Formatter) -> fmt::Result { Formatter::write_str(formatter, "struct Thingy") }
            #[inline]
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let field0 = match serde::de::SeqAccess::next_element::<HashMap<String, String>>(&mut seq)? {
                    Some(value) => value,
                    None => return Err(serde::de::Error::invalid_length(0usize, &"struct Thingy with 1 element")),
                };
                Ok(Thingy {
                    thingy: field0
                })
            }
            #[inline]
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut field0: Option<HashMap<String, String>> = None;
                while let Some(key) = serde::de::MapAccess::next_key::<Field>(&mut map)? {
                    match key {
                        Field::field0 => {
                            if Option::is_some(&field0) { return Err(<A::Error as serde::de::Error>::duplicate_field("Registry")); }
                            field0 = Some(serde::de::MapAccess::next_value::<HashMap<String, String>>(&mut map)?);
                        }
                        _ => { let _ = serde::de::MapAccess::next_value::<serde::de::IgnoredAny>(&mut map)?; }
                    }
                }
                let field0 = match field0 {
                    Some(field0) => field0,
                    None => missing_field("Registry")?,
                };
                Ok(Thingy {
                    thingy: field0
                })
            }
        }
        #[doc(hidden)]
        const FIELDS: &'static [&'static str] = &["Registry"];
        serde::Deserializer::deserialize_struct(deserializer, "Thingy", FIELDS, Visitor { marker: PhantomData::<Thingy>, lifetime: PhantomData })
    }
}


#[test]
fn thingy() {
    let mut map = HashMap::new();
    map.insert("hello".to_string(), "doctor".to_string());
    map.insert("yesterday".to_string(), "tomorrow".to_string());
   let thingy = Thingy::new(map);
    let list = vec![thingy.clone(),thingy.clone(),thingy.clone()];
    println!("{}", serde_yaml::to_string(&list).unwrap());
}



#[test]
fn des_thingy() {
    let mut map = HashMap::new();
    map.insert("hello".to_string(), "doctor".to_string());
    map.insert("yesterday".to_string(), "tomorrow".to_string());
    let thingy = Thingy::new(map);
    let list = vec![thingy.clone(),thingy.clone(),thingy.clone()];

    let yaml =
r#"
- Registry:
    yesterday: tomorrow
    hello: doctor
- Registry:
    yesterday: tomorrow
    hello: doctor
- Registry:
    yesterday: tomorrow
    hello: doctor
"#;

   let parsed  =serde_yaml::from_str::<Vec<Thingy>>(yaml).unwrap();

    assert_eq!(list,parsed);


let yaml =
        r#"
- Registry:
  yesterday: tomorrow
  hello: doctor
- Registry:
  yesterday: tomorrow
  hello: doctor
- Registry:
  yesterday: tomorrow
  hello: doctor
"#;


    let parsed2  =serde_yaml::from_str::<Vec<Thingy>>(yaml).unwrap();
    println!("{}", serde_yaml::to_string(&parsed2).unwrap());

}




/*
    impl Serialize for Registry {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer
        {
            let mut seq = serializer.serialize_seq(Some(1))?;
            seq.serialize_element("Registry")?;
            let serializer = seq.end()?;
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("database", "registry")?;
            map.serialize_entry("seed", "https://starlane.io")?;
            map.end()
        }
    }

         */

/*
    #[test]
    pub fn try_struct() {
        #[derive(Clone, Serialize, Deserialize)]
        pub enum Blah {
            T(String),
            R(Registry)
        }

        let registry = Registry {
            database: "registry".to_string(),
            seed: "along-seedthingy".to_string(),
        };
        let list = vec![registry];
        println!("{}", serde_yaml::to_string(&list).unwrap());
    }
}

 */


        /*
#[test]
pub fn list_of_maps()  {
    let mut list = vec![];
    for i in 0..5 {
        let mut map = MyMap::new();
        for x in 0..3 {
            let n = i*x;
            map.insert(format!("key-{}", n).to_string(), format!("value-{}", n).to_string());
        }
        list.push(map);
    }

    println!("{}",serde_yaml::to_string(&list).unwrap());


}

         */




 */