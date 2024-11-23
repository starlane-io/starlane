use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use rustls::pki_types::Der;
use serde::de::{DeserializeOwned, MapAccess, Visitor};
/*
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AbstractMappings<'z,V> where V: Serialize+DeserializeOwned+'z{
   map: HashMap<String,V>,
   phantom: PhantomData<&'z V>
}

 */

#[derive(Debug, Default,Clone, Eq, PartialEq, Serialize)]
pub struct MyMap<V> {
    map: HashMap<String,V>,
}

impl <V> Deref for MyMap<V> {
    type Target = HashMap<String,V>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl <V> DerefMut for MyMap<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        & mut self.map
    }
}

impl <V> MyMap<V> {
    fn new() -> MyMap<V> {
        Self {
        map: Default::default()
        }
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

#[cfg(test)]
pub mod test {
    use crate::hyperspace::foundation::util::MyMap;

    #[test]
pub fn abstract_map() {
        /*
    let mut map : AbstractMappings<String> = AbstractMappings::default();
    map.insert("hello","doctor".to_string());
    map.insert("yesterday","tomorrow".to_string());
    let string = serde_yaml::to_string(&map).unwrap();

    println!("{}",string);

         */
}


}