use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use rustls::pki_types::Der;
use serde::de::{DeserializeOwned, MapAccess, Visitor};
use serde_yaml::Value::Tagged;
/*
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AbstractMappings<'z,V> where V: Serialize+DeserializeOwned+'z{
   map: HashMap<String,V>,
   phantom: PhantomData<&'z V>
}

 */



#[derive(Debug, Default,Clone, Eq, PartialEq, Serialize)]
pub struct MyMap<V>(HashMap<String,V>);


impl <V> Deref for MyMap<V> {
    type Target = HashMap<String,V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl <V> DerefMut for MyMap<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        & mut self.0
    }
}

impl <V> MyMap<V> {
    fn new() -> MyMap<V> {
        MyMap(Default::default())
    }
}

struct MyMapVisitor<V> {
    marker: PhantomData<fn() -> MyMap<V>>
}

impl<V> MyMapVisitor<V> {
    fn new() -> Self {
        MyMapVisitor {
            marker: Default::default()
        }
    }
}


impl<'de, V> Visitor<'de> for MyMapVisitor<V>
where
    V: Deserialize<'de>,
{
    type Value = MyMap<V>;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a very special map")
    }


    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut map = MyMap::new();

        // While there are entries remaining in the input, add them
        // into our map.
        while let Some((key, value)) = access.next_entry()? {
            map.insert(key, value);
        }

        Ok(map)
    }
}




// This is the trait that informs Serde how to deserialize MyMap.
impl<'de,V> Deserialize<'de> for MyMap<V>
where
    V: Deserialize<'de>,
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







#[derive(Clone,Serialize,Deserialize)]
#[serde(untagged)]
pub enum Tag{
  Tag(String),
  Tuple(MyMap<String>)
}

impl Tag {
    fn tag(string: impl ToString) -> Self {
        Tag::Tag(string.to_string())
    }

    fn tuple(key: impl ToString,value: impl ToString) -> Self {
        let mut map = MyMap::new();
        map.insert(key.to_string(), value.to_string());
        map.insert(value.to_string(), key.to_string());

        Tag::Tuple(map)
    }

}







#[cfg(test)]
pub mod test {
    use chrono::serde::ts_microseconds::serialize;
    use convert_case::Casing;
    use serde::{Deserialize, Serialize, Serializer};
    use serde::ser::{SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant};
    use crate::hyperspace::foundation::util::{MyMap, Tag};

    #[test]
    pub fn tagz() {
        let mut list = vec![];
        list.push(Tag::tag("Registry"));
        list.push(Tag::tuple("database", "registry"));
        println!("{}", serde_yaml::to_string(&list).unwrap());
    }


    #[test]
    pub fn abstract_map() {
        let mut map: MyMap<String> = MyMap::new();
        map.insert("hello".to_string(), "doctor".to_string());
        map.insert("yesterday".to_string(), "tomorrow".to_string());

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


