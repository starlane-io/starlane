use uuid::Uuid;

use crate::error::Error;
use crate::user::{AuthToken, User};
use serde::{Deserialize, Serialize, Serializer};
use std::marker::PhantomData;

pub type CryptKeyId = Uuid;
pub type HashId = Uuid;
pub type Hash = Vec<u8>;

pub struct UniqueHash
{
    pub id: HashId,
    pub hash: Hash
}

#[derive(Clone,Serialize,Deserialize)]
pub struct PublicKey
{
    pub id: CryptKeyId,
    pub data: Vec<u8>
}

#[derive(Clone,Serialize,Deserialize)]
pub struct PrivateKey
{
    pub id: CryptKeyId,
    pub data: Vec<u8>
}

pub struct EncryptionKeysFactory
{
}

impl EncryptionKeysFactory
{
    fn create(&self)->(PublicKey, PrivateKey)
    {
        let id = CryptKeyId::new_v4();
        (PublicKey {
            id: id.clone(),
            data: vec![]
        },
         PrivateKey {
             id: id.clone(),
             data: vec![]
        })
    }
}


#[derive(Clone,Serialize,Deserialize)]
pub struct Encrypted<D>
{
    pub key_id: CryptKeyId,
    pub data: Vec<u8>,
    phantom: PhantomData<D>,
}

impl <D> Encrypted<D> where D: Sync+Send+Serialize+Deserialize<'static>
{
    pub fn encrypt( data: &D, public_key: &PublicKey ) -> Self
    {
        let bytes = serde_json::to_string(&data).unwrap().into_bytes();
        Encrypted{
            key_id: public_key.id,
            data: bytes,
            phantom: PhantomData::default(),
        }
    }

    pub fn decrypt( &self, private_key: &PrivateKey ) -> D
    {
//        let mut str = String::from_utf8(self.data.clone() ).unwrap();
//        serde_json::from_str(str.as_str() ).unwrap()
        unimplemented!()
    }
}

#[derive(Clone,Serialize,Deserialize)]
pub struct HashEncrypted<D>
{
    pub key_id: CryptKeyId,
    pub hash_id: HashId,
    pub data: Vec<u8>,
    phantom: PhantomData<D>,
}

impl <D> HashEncrypted<D> where D: Sync+Send+Serialize+Deserialize<'static>
{
    pub fn encrypt( data: &D, hash: &UniqueHash, public_key: &PublicKey ) -> Self
    {
        let bytes = serde_json::to_string(&data).unwrap().into_bytes();
        HashEncrypted{
            hash_id: hash.id.clone(),
            key_id: public_key.id.clone(),
            data: bytes,
            phantom: PhantomData::default(),
        }
    }

    pub fn decrypt( &self, hash: &Vec<u8>, private_key: &PrivateKey ) -> D
    {
//        let mut str = String::from_utf8(self.data.clone() ).unwrap();
//        serde_json::from_str(str.as_str() ).unwrap()
        unimplemented!()
    }
}

pub struct JwtDecoder
{

}