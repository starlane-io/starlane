use std::fmt::Debug;
use std::marker::PhantomData;
use serde::{Deserialize, Deserializer, Serialize};
use serde_yaml::Value;
use crate::hyperspace::foundation::kind::IKind;

#[derive(Debug, Clone,Serialize,Deserialize)]
struct Config<K,C> where K: IKind, C: Debug+Clone{
   kind: K,
   config: C,
}

pub type ProtoConfig<K: IKind> = Config<K,Value>;



#[cfg(test)]
pub mod test {
   #[test]
   pub fn test() {

   }
}